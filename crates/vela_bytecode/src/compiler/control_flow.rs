use vela_common::Span;
use vela_hir::binding::LocalBindingKind;
use vela_hir::type_hint::HirTypeHint;
use vela_syntax::ast::{
    BinaryOp, Block, ElseBranch, Expr, ExprKind, IfExpr, MatchExpr, Pattern, Stmt, StmtKind,
};

use crate::{Constant, InstructionKind, InstructionOffset, Register};

use super::script_types::{ScriptTypeFact, type_hint_script_type};
use super::value_flow::{BlockValue, block_value};
use super::value_types::type_hint_value_type;
use super::{CompileError, CompileErrorKind, CompileResult, Compiler, frame_slot_kind};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LoopContext {
    continue_target: usize,
    break_jumps: Vec<usize>,
    continue_jumps: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LoopIterable {
    Generic {
        iterator: Register,
    },
    Range {
        cursor: Register,
        end: Register,
        done: Register,
        inclusive: bool,
    },
}

impl LoopContext {
    pub(super) fn new(continue_target: usize) -> Self {
        Self {
            continue_target,
            break_jumps: Vec::new(),
            continue_jumps: Vec::new(),
        }
    }

    pub(super) fn continue_target(&self) -> usize {
        self.continue_target
    }

    pub(super) fn break_jumps(&self) -> &[usize] {
        &self.break_jumps
    }

    pub(super) fn continue_jumps(&self) -> &[usize] {
        &self.continue_jumps
    }

    pub(super) fn push_break(&mut self, offset: usize) {
        self.break_jumps.push(offset);
    }

    pub(super) fn push_continue(&mut self, offset: usize) {
        self.continue_jumps.push(offset);
    }
}

impl Compiler<'_> {
    pub(super) fn compile_statements(&mut self, statements: &[Stmt]) -> CompileResult<bool> {
        for stmt in statements {
            if self.compile_statement(stmt)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn compile_statement(&mut self, stmt: &Stmt) -> CompileResult<bool> {
        match &stmt.kind {
            StmtKind::Let {
                name,
                type_hint,
                value,
            } => {
                let hinted_script_fact = type_hint.as_ref().and_then(|hint| {
                    let known_type_names = self.facts.known_type_names();
                    type_hint_script_type(&HirTypeHint::from_syntax(hint), known_type_names.iter())
                        .map(ScriptTypeFact::new)
                });
                let value_script_fact = value
                    .as_ref()
                    .and_then(|value| self.script_fact_for_expr(value));
                let script_fact =
                    merge_type_hint_and_value_fact(hinted_script_fact, value_script_fact);
                let hinted_value_type = type_hint
                    .as_ref()
                    .and_then(|hint| type_hint_value_type(&HirTypeHint::from_syntax(hint)));
                let value_type = value
                    .as_ref()
                    .and_then(|value| self.value_type_for_expr(value));
                let value_type = hinted_value_type.or(value_type);
                let (register, returned) = if let Some(value) = value {
                    self.compile_let_initializer(value)?
                } else {
                    (self.emit_constant(Constant::Null)?, false)
                };
                self.locals.insert(name.clone(), register);
                if let Some(local) =
                    self.bindings
                        .local_named_at(name, LocalBindingKind::Let, stmt.span)
                {
                    self.hir_locals.insert(local, register);
                    self.record_frame_slot(
                        name.clone(),
                        register,
                        frame_slot_kind(LocalBindingKind::Let),
                        Some(local),
                        Some(stmt.span),
                    );
                    self.script_types.set_local_fact(local, name, script_fact);
                    self.value_types.set_local(local, name, value_type);
                } else {
                    self.record_frame_slot(
                        name.clone(),
                        register,
                        frame_slot_kind(LocalBindingKind::Let),
                        None,
                        Some(stmt.span),
                    );
                    self.script_types.set_name_fact(name, script_fact);
                    self.value_types.set_name(name, value_type);
                }
                Ok(returned)
            }
            StmtKind::Return(value) => {
                let register = if let Some(value) = value {
                    self.compile_expr(value)?
                } else {
                    self.emit_constant(Constant::Null)?
                };
                self.emit(InstructionKind::Return { src: register });
                Ok(true)
            }
            StmtKind::Expr(expr) => {
                if let ExprKind::If(if_expr) = &expr.kind {
                    return self.compile_if(if_expr);
                }
                if let ExprKind::Match(match_expr) = &expr.kind {
                    return self.compile_match(match_expr);
                }
                if let ExprKind::Assign { .. } = &expr.kind {
                    self.compile_assignment(expr)?;
                    return Ok(false);
                }
                self.compile_expr(expr)?;
                Ok(false)
            }
            StmtKind::Block(block) => self.compile_statements(&block.statements),
            StmtKind::For {
                index_pattern,
                pattern,
                iterable,
                body,
            } => self.compile_for(stmt.span, index_pattern.as_ref(), pattern, iterable, body),
            StmtKind::Break => self.compile_break(),
            StmtKind::Continue => self.compile_continue(),
        }
    }

    fn compile_let_initializer(&mut self, value: &Expr) -> CompileResult<(Register, bool)> {
        match &value.kind {
            ExprKind::Block(block) => {
                let dst = self.alloc_register()?;
                let returned = self.compile_block_value_to(block, dst)?;
                Ok((dst, returned))
            }
            ExprKind::If(if_expr) => {
                let dst = self.alloc_register()?;
                let returned = self.compile_if_value_to(if_expr, dst)?;
                Ok((dst, returned))
            }
            ExprKind::Match(match_expr) => {
                let dst = self.alloc_register()?;
                let returned = self.compile_match_value_to(match_expr, dst)?;
                Ok((dst, returned))
            }
            _ => self.compile_expr(value).map(|register| (register, false)),
        }
    }

    fn compile_for(
        &mut self,
        stmt_span: Span,
        index_pattern: Option<&Pattern>,
        pattern: &Pattern,
        iterable: &Expr,
        body: &Block,
    ) -> CompileResult<bool> {
        let range_iterable = match &iterable.kind {
            ExprKind::Binary {
                op: BinaryOp::Range,
                left,
                right,
            } => Some((left.as_ref(), right.as_ref(), false)),
            ExprKind::Binary {
                op: BinaryOp::RangeInclusive,
                left,
                right,
            } => Some((left.as_ref(), right.as_ref(), true)),
            _ => None,
        };
        let loop_iterable = if let Some((start, end, inclusive)) = range_iterable {
            let cursor = self.compile_expr(start)?;
            let end = self.compile_expr(end)?;
            let done = self.alloc_register()?;
            self.emit_bool_constant_to(done, false);
            LoopIterable::Range {
                cursor,
                end,
                done,
                inclusive,
            }
        } else {
            let iterable = self.compile_expr(iterable)?;
            let iterator = self.alloc_register()?;
            self.emit(InstructionKind::IterInit {
                dst: iterator,
                iterable,
            });
            LoopIterable::Generic { iterator }
        };

        let item_register = self.alloc_register()?;
        let loop_index = if index_pattern.is_some() {
            let counter = self.alloc_register()?;
            self.emit_constant_to(counter, Constant::Int(0));
            Some((counter, self.emit_constant(Constant::Int(1))?))
        } else {
            None
        };
        let index_register = if index_pattern.is_some() {
            Some(self.alloc_register()?)
        } else {
            None
        };
        let previous_locals = self.locals.clone();
        let previous_hir_locals = self.hir_locals.clone();
        let previous_script_types = self.script_types.clone();
        let previous_value_types = self.value_types.clone();

        let loop_start = self.current_offset();
        let done_jump = match loop_iterable {
            LoopIterable::Generic { iterator } => self.emit_iter_next(iterator, item_register),
            LoopIterable::Range {
                cursor,
                end,
                done,
                inclusive,
            } => self.emit_range_next(cursor, end, done, inclusive, item_register),
        };
        if let (Some((counter, one)), Some(index_register)) = (loop_index, index_register) {
            self.emit(InstructionKind::Move {
                dst: index_register,
                src: counter,
            });
            self.emit(InstructionKind::Add {
                dst: counter,
                lhs: counter,
                rhs: one,
            });
        }
        let mut mismatch_jumps = Vec::new();
        if let (Some(index_pattern), Some(index_register)) = (index_pattern, index_register) {
            mismatch_jumps.extend(self.compile_match_pattern(index_register, index_pattern)?);
            self.bind_pattern_locals(
                index_register,
                index_pattern,
                stmt_span,
                None,
                LocalBindingKind::For,
            )?;
        }
        mismatch_jumps.extend(self.compile_match_pattern(item_register, pattern)?);
        self.bind_pattern_locals(
            item_register,
            pattern,
            stmt_span,
            None,
            LocalBindingKind::For,
        )?;
        self.loop_stack.push(LoopContext::new(loop_start));
        let body_returned = self.compile_statements(&body.statements)?;
        let loop_context = self
            .loop_stack
            .pop()
            .expect("loop context pushed before compiling for body");
        if !body_returned {
            self.emit(InstructionKind::Jump {
                target: InstructionOffset(loop_start),
            });
        }
        let loop_end = self.current_offset();
        self.patch_jump(done_jump, loop_end)?;
        for jump in mismatch_jumps {
            self.patch_jump(jump, loop_start)?;
        }
        for jump in loop_context.break_jumps() {
            self.patch_jump(*jump, loop_end)?;
        }
        for jump in loop_context.continue_jumps() {
            self.patch_jump(*jump, loop_context.continue_target())?;
        }

        self.locals = previous_locals;
        self.hir_locals = previous_hir_locals;
        self.script_types = previous_script_types;
        self.value_types = previous_value_types;

        Ok(false)
    }

    fn compile_break(&mut self) -> CompileResult<bool> {
        if self.loop_stack.is_empty() {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "break outside loop",
            )));
        }
        let jump = self.emit_jump();
        self.loop_stack
            .last_mut()
            .expect("loop stack checked above")
            .push_break(jump);
        Ok(true)
    }

    fn compile_continue(&mut self) -> CompileResult<bool> {
        if self.loop_stack.is_empty() {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "continue outside loop",
            )));
        }
        let jump = self.emit_jump();
        self.loop_stack
            .last_mut()
            .expect("loop stack checked above")
            .push_continue(jump);
        Ok(true)
    }

    pub(super) fn compile_block_value_to(
        &mut self,
        block: &Block,
        dst: Register,
    ) -> CompileResult<bool> {
        match block_value(block) {
            BlockValue::Empty => {
                self.emit_constant_to(dst, Constant::Null);
                Ok(false)
            }
            BlockValue::TailExpr { prefix, expr } => {
                for stmt in prefix {
                    if self.compile_statement(stmt)? {
                        return Ok(true);
                    }
                }
                let value = self.compile_expr(expr)?;
                self.emit(InstructionKind::Move { dst, src: value });
                Ok(false)
            }
            BlockValue::Statements(statements) => {
                let returned = self.compile_statements(statements)?;
                if !returned {
                    self.emit_constant_to(dst, Constant::Null);
                }
                Ok(returned)
            }
        }
    }

    fn compile_if(&mut self, if_expr: &IfExpr) -> CompileResult<bool> {
        let condition = self.compile_expr(&if_expr.condition)?;
        let jump_to_else = self.emit_jump_if_false(condition);

        let then_returned = self.compile_statements(&if_expr.then_branch.statements)?;
        let jump_to_end = if then_returned {
            None
        } else {
            Some(self.emit_jump())
        };

        self.patch_jump(jump_to_else, self.current_offset())?;

        let else_returned = match &if_expr.else_branch {
            Some(ElseBranch::Block(block)) => self.compile_statements(&block.statements)?,
            Some(ElseBranch::If(if_expr)) => self.compile_if(if_expr)?,
            None => false,
        };

        if let Some(jump_to_end) = jump_to_end {
            self.patch_jump(jump_to_end, self.current_offset())?;
        }

        Ok(then_returned && else_returned)
    }

    pub(super) fn compile_if_value_to(
        &mut self,
        if_expr: &IfExpr,
        dst: Register,
    ) -> CompileResult<bool> {
        let condition = self.compile_expr(&if_expr.condition)?;
        let jump_to_else = self.emit_jump_if_false(condition);

        let then_returned = self.compile_block_value_to(&if_expr.then_branch, dst)?;
        let jump_to_end = if then_returned {
            None
        } else {
            Some(self.emit_jump())
        };

        self.patch_jump(jump_to_else, self.current_offset())?;

        let else_returned = match &if_expr.else_branch {
            Some(ElseBranch::Block(block)) => self.compile_block_value_to(block, dst)?,
            Some(ElseBranch::If(if_expr)) => self.compile_if_value_to(if_expr, dst)?,
            None => {
                self.emit_constant_to(dst, Constant::Null);
                false
            }
        };

        if let Some(jump_to_end) = jump_to_end {
            self.patch_jump(jump_to_end, self.current_offset())?;
        }

        Ok(then_returned && else_returned)
    }

    fn compile_match(&mut self, match_expr: &MatchExpr) -> CompileResult<bool> {
        let scrutinee_fact = self.script_fact_for_expr(&match_expr.scrutinee);
        let scrutinee = self.compile_expr(&match_expr.scrutinee)?;
        let mut end_jumps = Vec::new();
        let mut all_arms_return = !match_expr.arms.is_empty();

        for arm in &match_expr.arms {
            let mut next_arm_jumps = self.compile_match_pattern(scrutinee, &arm.pattern)?;
            let previous_locals = self.locals.clone();
            let previous_hir_locals = self.hir_locals.clone();
            let previous_script_types = self.script_types.clone();
            let previous_value_types = self.value_types.clone();
            self.bind_pattern_locals(
                scrutinee,
                &arm.pattern,
                arm.body.span,
                scrutinee_fact.clone(),
                LocalBindingKind::Pattern,
            )?;
            if let Some(jump) = self.compile_match_guard(arm.guard.as_ref())? {
                next_arm_jumps.push(jump);
            }
            let arm_returned = match &arm.body.kind {
                ExprKind::Block(block) => self.compile_statements(&block.statements)?,
                _ => {
                    self.compile_expr(&arm.body)?;
                    false
                }
            };
            self.locals = previous_locals;
            self.hir_locals = previous_hir_locals;
            self.script_types = previous_script_types;
            self.value_types = previous_value_types;
            all_arms_return &= arm_returned;
            if !arm_returned {
                end_jumps.push(self.emit_jump());
            }
            if next_arm_jumps.is_empty() {
                break;
            }
            for jump in next_arm_jumps {
                self.patch_jump(jump, self.current_offset())?;
            }
        }

        for jump in end_jumps {
            self.patch_jump(jump, self.current_offset())?;
        }

        Ok(all_arms_return)
    }

    pub(super) fn compile_match_value_to(
        &mut self,
        match_expr: &MatchExpr,
        dst: Register,
    ) -> CompileResult<bool> {
        let scrutinee_fact = self.script_fact_for_expr(&match_expr.scrutinee);
        let scrutinee = self.compile_expr(&match_expr.scrutinee)?;
        let mut end_jumps = Vec::new();
        let mut all_arms_return = !match_expr.arms.is_empty();
        let mut has_catch_all = false;

        for arm in &match_expr.arms {
            let mut next_arm_jumps = self.compile_match_pattern(scrutinee, &arm.pattern)?;
            let previous_locals = self.locals.clone();
            let previous_hir_locals = self.hir_locals.clone();
            let previous_script_types = self.script_types.clone();
            let previous_value_types = self.value_types.clone();
            self.bind_pattern_locals(
                scrutinee,
                &arm.pattern,
                arm.body.span,
                scrutinee_fact.clone(),
                LocalBindingKind::Pattern,
            )?;
            if let Some(jump) = self.compile_match_guard(arm.guard.as_ref())? {
                next_arm_jumps.push(jump);
            }
            let arm_returned = self.compile_match_arm_value_to(&arm.body, dst)?;
            self.locals = previous_locals;
            self.hir_locals = previous_hir_locals;
            self.script_types = previous_script_types;
            self.value_types = previous_value_types;
            all_arms_return &= arm_returned;
            if !arm_returned {
                end_jumps.push(self.emit_jump());
            }
            if next_arm_jumps.is_empty() {
                has_catch_all = true;
                break;
            }
            for jump in next_arm_jumps {
                self.patch_jump(jump, self.current_offset())?;
            }
        }

        if !has_catch_all {
            self.emit_constant_to(dst, Constant::Null);
            all_arms_return = false;
        }

        for jump in end_jumps {
            self.patch_jump(jump, self.current_offset())?;
        }

        Ok(all_arms_return)
    }

    fn compile_match_guard(&mut self, guard: Option<&Expr>) -> CompileResult<Option<usize>> {
        let Some(guard) = guard else {
            return Ok(None);
        };
        let condition = self.compile_expr(guard)?;
        Ok(Some(self.emit_jump_if_false(condition)))
    }

    fn compile_match_arm_value_to(&mut self, body: &Expr, dst: Register) -> CompileResult<bool> {
        match &body.kind {
            ExprKind::Block(block) => self.compile_block_value_to(block, dst),
            _ => {
                let value = self.compile_expr(body)?;
                self.emit(InstructionKind::Move { dst, src: value });
                Ok(false)
            }
        }
    }
}

fn merge_type_hint_and_value_fact(
    hinted: Option<ScriptTypeFact>,
    value: Option<ScriptTypeFact>,
) -> Option<ScriptTypeFact> {
    match (hinted, value) {
        (Some(hinted), Some(value)) if hinted.type_name == value.type_name => {
            Some(ScriptTypeFact {
                type_name: hinted.type_name,
                enum_variant: value.enum_variant,
            })
        }
        (Some(hinted), _) => Some(hinted),
        (None, value) => value,
    }
}
