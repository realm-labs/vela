use vela_common::{PrimitiveTag, Span};
use vela_hir::binding::LocalBindingKind;
use vela_hir::type_hint::HirTypeHint;
use vela_syntax::ast::{
    BinaryOp, Block, ElseBranch, Expr, ExprKind, IfExpr, Literal, MatchExpr, Pattern, Stmt,
    StmtKind, SyntaxExpressionKind, SyntaxStatementKind,
};

use crate::{Constant, InstructionOffset, Register, UnlinkedInstructionKind};

use super::body_payloads::CompilerStatementPayload;
use super::const_eval::compile_literal_constant_for_type;
use super::operators::i64_compare_op;
use super::patterns::PatternBindingFacts;
use super::record_shapes::ValueShape;
use super::script_types::{ScriptTypeFact, type_hint_script_type};
use super::value_flow::{BlockValue, block_value};
use super::value_types::{
    RuntimeTypeFact, StaticExprType, TypeContractContext, check_expected_type, type_hint_value_type,
};
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

impl Compiler<'_, '_> {
    pub(super) fn compile_statement_payloads(
        &mut self,
        statements: &[CompilerStatementPayload<'_>],
    ) -> CompileResult<bool> {
        for stmt in statements {
            if self.compile_statement_payload(stmt)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn compile_statement_payload(
        &mut self,
        stmt: &CompilerStatementPayload<'_>,
    ) -> CompileResult<bool> {
        let Some(kind) = stmt.statement_kind() else {
            return self.compile_statement(stmt.fallback());
        };
        if !statement_kind_matches(kind, stmt.fallback()) {
            self.compile_statement(stmt.fallback())
        } else if kind == SyntaxStatementKind::Let {
            self.compile_let_statement(stmt.fallback(), stmt.let_initializer_kind())
        } else if kind == SyntaxStatementKind::Return {
            self.compile_return_statement(stmt.fallback(), stmt.return_value_kind())
        } else if kind == SyntaxStatementKind::For {
            self.compile_for_statement(stmt.fallback(), stmt.for_iterable_binary_operator())
        } else if kind == SyntaxStatementKind::If {
            self.compile_if_statement(stmt.fallback(), stmt.if_condition_binary_operator())
        } else if kind == SyntaxStatementKind::Expr {
            self.compile_expr_statement_payload(stmt)
        } else {
            self.compile_statement_as(kind, stmt.fallback())
        }
    }

    fn compile_expr_statement_payload(
        &mut self,
        stmt: &CompilerStatementPayload<'_>,
    ) -> CompileResult<bool> {
        let StmtKind::Expr(expr) = &stmt.fallback().kind else {
            return self.compile_statement(stmt.fallback());
        };
        let Some(kind) = stmt.expression_kind() else {
            return self.compile_expr_statement(expr);
        };
        if !expression_statement_kind_matches(kind, expr) {
            return self.compile_expr_statement(expr);
        }
        if kind == SyntaxExpressionKind::Assign {
            self.compile_assignment(expr)?;
            Ok(false)
        } else {
            self.compile_expr(expr)?;
            Ok(false)
        }
    }

    pub(super) fn compile_statements(&mut self, statements: &[Stmt]) -> CompileResult<bool> {
        for stmt in statements {
            if self.compile_statement(stmt)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(super) fn compile_statement(&mut self, stmt: &Stmt) -> CompileResult<bool> {
        self.compile_statement_as(legacy_statement_kind(stmt), stmt)
    }

    fn compile_statement_as(
        &mut self,
        kind: SyntaxStatementKind,
        stmt: &Stmt,
    ) -> CompileResult<bool> {
        match kind {
            SyntaxStatementKind::Let => self.compile_let_statement(stmt, None),
            SyntaxStatementKind::Return => self.compile_return_statement(stmt, None),
            SyntaxStatementKind::Break => {
                let StmtKind::Break = &stmt.kind else {
                    return self.compile_statement(stmt);
                };
                self.compile_break()
            }
            SyntaxStatementKind::Continue => {
                let StmtKind::Continue = &stmt.kind else {
                    return self.compile_statement(stmt);
                };
                self.compile_continue()
            }
            SyntaxStatementKind::For => self.compile_for_statement(stmt, None),
            SyntaxStatementKind::If => self.compile_if_statement(stmt, None),
            SyntaxStatementKind::Match => {
                let StmtKind::Expr(expr) = &stmt.kind else {
                    return self.compile_statement(stmt);
                };
                let ExprKind::Match(match_expr) = &expr.kind else {
                    return self.compile_statement(stmt);
                };
                self.compile_match(match_expr)
            }
            SyntaxStatementKind::Block => {
                let StmtKind::Block(block) = &stmt.kind else {
                    return self.compile_statement(stmt);
                };
                self.compile_statements(&block.statements)
            }
            SyntaxStatementKind::Expr => {
                let StmtKind::Expr(expr) = &stmt.kind else {
                    return self.compile_statement(stmt);
                };
                self.compile_expr_statement(expr)
            }
        }
    }

    fn compile_let_statement(
        &mut self,
        stmt: &Stmt,
        initializer_kind: Option<SyntaxExpressionKind>,
    ) -> CompileResult<bool> {
        let StmtKind::Let {
            name,
            type_hint: _,
            value,
        } = &stmt.kind
        else {
            return self.compile_statement(stmt);
        };
        let local_binding = self
            .bindings
            .local_named_at(name, LocalBindingKind::Let, stmt.span)
            .and_then(|local| {
                self.bindings
                    .local(local)
                    .map(|binding| (local, binding.type_hint.clone()))
            });
        let hir_type_hint = local_binding.as_ref().and_then(|(_, hint)| hint.as_ref());
        let hinted_script_fact = hir_type_hint.and_then(|hint| {
            let known_type_names = self.facts.known_type_names();
            type_hint_script_type(hint, known_type_names.iter()).map(ScriptTypeFact::new)
        });
        let value_script_fact = value
            .as_ref()
            .and_then(|value| self.script_fact_for_expr(value));
        let script_hint_proven = hinted_script_fact
            .as_ref()
            .zip(value_script_fact.as_ref())
            .is_some_and(|(hint, value)| hint == value);
        let script_fact = merge_type_hint_and_value_fact(hinted_script_fact, value_script_fact);
        let hinted_value_type = hir_type_hint.and_then(type_hint_value_type);
        let value_type = value
            .as_ref()
            .and_then(|value| self.value_type_for_expr(value));
        let value_type = hinted_value_type.clone().or(value_type);
        let value_shape = value
            .as_ref()
            .and_then(|value| self.value_shape_for_expr(value));
        let (register, returned) = if let Some(value) = value {
            self.compile_let_initializer(
                value,
                hinted_value_type.clone(),
                TypeContractContext::TypedLet { name: name.clone() },
                initializer_kind,
            )?
        } else {
            (self.emit_constant(Constant::Null)?, false)
        };
        if let (Some(value), Some(hint), None) =
            (value.as_ref(), hir_type_hint, hinted_value_type.as_ref())
            && is_map_or_set_type_hint(hint)
            && !script_hint_proven
            && let Some(guard) =
                super::type_guard_for_hint(hint, crate::GuardLocation::Local, name, &self.facts)
        {
            self.emit_spanned(
                UnlinkedInstructionKind::GuardType {
                    src: register,
                    guard,
                },
                value.span,
            );
        }
        self.locals.insert(name.clone(), register);
        if let Some((local, _)) = local_binding {
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
            self.value_shapes.set_local(local, name, value_shape);
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
            self.value_shapes.set_name(name, value_shape);
        }
        Ok(returned)
    }

    fn compile_return_statement(
        &mut self,
        stmt: &Stmt,
        value_kind: Option<SyntaxExpressionKind>,
    ) -> CompileResult<bool> {
        let StmtKind::Return(value) = &stmt.kind else {
            return self.compile_statement(stmt);
        };
        let (register, returned) =
            self.compile_return_value(stmt.span, value.as_ref(), value_kind)?;
        if !returned {
            self.emit(UnlinkedInstructionKind::Return { src: register });
        }
        Ok(true)
    }

    fn compile_for_statement(
        &mut self,
        stmt: &Stmt,
        iterable_operator: Option<BinaryOp>,
    ) -> CompileResult<bool> {
        let StmtKind::For {
            index_pattern,
            pattern,
            iterable,
            body,
        } = &stmt.kind
        else {
            return self.compile_statement(stmt);
        };
        self.compile_for(
            stmt.span,
            index_pattern.as_ref(),
            pattern,
            iterable,
            body,
            iterable_operator,
        )
    }

    fn compile_if_statement(
        &mut self,
        stmt: &Stmt,
        condition_operator: Option<BinaryOp>,
    ) -> CompileResult<bool> {
        let StmtKind::Expr(expr) = &stmt.kind else {
            return self.compile_statement(stmt);
        };
        let ExprKind::If(if_expr) = &expr.kind else {
            return self.compile_statement(stmt);
        };
        self.compile_if(if_expr, condition_operator)
    }

    fn compile_expr_statement(&mut self, expr: &Expr) -> CompileResult<bool> {
        if let ExprKind::If(if_expr) = &expr.kind {
            return self.compile_if(if_expr, None);
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

    fn compile_let_initializer(
        &mut self,
        value: &Expr,
        expected: Option<super::value_types::RuntimeTypeFact>,
        context: TypeContractContext,
        syntax_kind: Option<SyntaxExpressionKind>,
    ) -> CompileResult<(Register, bool)> {
        if let Some(kind) = syntax_kind
            && value_expression_kind_matches(kind, value)
        {
            return self.compile_let_initializer_with_syntax_kind(value, expected, context, kind);
        }
        self.compile_let_initializer_legacy(value, expected, context)
    }

    fn compile_let_initializer_with_syntax_kind(
        &mut self,
        value: &Expr,
        expected: Option<super::value_types::RuntimeTypeFact>,
        context: TypeContractContext,
        kind: SyntaxExpressionKind,
    ) -> CompileResult<(Register, bool)> {
        match kind {
            SyntaxExpressionKind::Block => {
                if let Some(expected) = expected {
                    self.expected_type_for_expr(value, expected, context)?;
                }
                let ExprKind::Block(block) = &value.kind else {
                    unreachable!("validated CST block initializer kind");
                };
                let dst = self.alloc_register()?;
                let returned = self.compile_block_value_to(block, dst)?;
                Ok((dst, returned))
            }
            SyntaxExpressionKind::If => {
                if let Some(expected) = expected {
                    self.expected_type_for_expr(value, expected, context)?;
                }
                let ExprKind::If(if_expr) = &value.kind else {
                    unreachable!("validated CST if initializer kind");
                };
                let dst = self.alloc_register()?;
                let returned = self.compile_if_value_to(if_expr, dst)?;
                Ok((dst, returned))
            }
            SyntaxExpressionKind::Match => {
                if let Some(expected) = expected {
                    self.expected_type_for_expr(value, expected, context)?;
                }
                let ExprKind::Match(match_expr) = &value.kind else {
                    unreachable!("validated CST match initializer kind");
                };
                let dst = self.alloc_register()?;
                let returned = self.compile_match_value_to(match_expr, dst)?;
                Ok((dst, returned))
            }
            _ => self.compile_let_initializer_legacy(value, expected, context),
        }
    }

    fn compile_let_initializer_legacy(
        &mut self,
        value: &Expr,
        expected: Option<super::value_types::RuntimeTypeFact>,
        context: TypeContractContext,
    ) -> CompileResult<(Register, bool)> {
        match &value.kind {
            ExprKind::Block(block) => {
                if let Some(expected) = expected {
                    self.expected_type_for_expr(value, expected, context)?;
                }
                let dst = self.alloc_register()?;
                let returned = self.compile_block_value_to(block, dst)?;
                Ok((dst, returned))
            }
            ExprKind::If(if_expr) => {
                if let Some(expected) = expected {
                    self.expected_type_for_expr(value, expected, context)?;
                }
                let dst = self.alloc_register()?;
                let returned = self.compile_if_value_to(if_expr, dst)?;
                Ok((dst, returned))
            }
            ExprKind::Match(match_expr) => {
                if let Some(expected) = expected {
                    self.expected_type_for_expr(value, expected, context)?;
                }
                let dst = self.alloc_register()?;
                let returned = self.compile_match_value_to(match_expr, dst)?;
                Ok((dst, returned))
            }
            _ => match expected {
                Some(expected) => self
                    .compile_expr_with_expected_type(value, expected, context)
                    .map(|register| (register, false)),
                None => self.compile_expr(value).map(|register| (register, false)),
            },
        }
    }

    fn compile_return_value(
        &mut self,
        span: Span,
        value: Option<&Expr>,
        syntax_kind: Option<SyntaxExpressionKind>,
    ) -> CompileResult<(Register, bool)> {
        match (value, self.return_type.clone()) {
            (Some(value), Some(expected)) => self.compile_return_expr(
                value,
                Some(expected),
                TypeContractContext::Return,
                syntax_kind,
            ),
            (Some(value), None) => {
                self.compile_return_expr(value, None, TypeContractContext::Return, syntax_kind)
            }
            (None, Some(expected)) => {
                check_expected_type(
                    StaticExprType::Exact(RuntimeTypeFact::primitive(
                        vela_common::PrimitiveTag::Null,
                    )),
                    expected,
                    span,
                    TypeContractContext::Return,
                )?;
                self.emit_constant(Constant::Null)
                    .map(|register| (register, false))
            }
            (None, None) => self
                .emit_constant(Constant::Null)
                .map(|register| (register, false)),
        }
    }

    fn compile_return_expr(
        &mut self,
        value: &Expr,
        expected: Option<super::value_types::RuntimeTypeFact>,
        context: TypeContractContext,
        syntax_kind: Option<SyntaxExpressionKind>,
    ) -> CompileResult<(Register, bool)> {
        if let Some(kind) = syntax_kind
            && value_expression_kind_matches(kind, value)
        {
            return self.compile_return_expr_with_syntax_kind(value, expected, context, kind);
        }
        self.compile_return_expr_legacy(value, expected, context)
    }

    fn compile_return_expr_with_syntax_kind(
        &mut self,
        value: &Expr,
        expected: Option<super::value_types::RuntimeTypeFact>,
        context: TypeContractContext,
        kind: SyntaxExpressionKind,
    ) -> CompileResult<(Register, bool)> {
        match kind {
            SyntaxExpressionKind::Block => {
                if let Some(expected) = expected {
                    self.expected_type_for_expr(value, expected, context)?;
                }
                let ExprKind::Block(block) = &value.kind else {
                    unreachable!("validated CST block return value kind");
                };
                let dst = self.alloc_register()?;
                let returned = self.compile_block_value_to(block, dst)?;
                Ok((dst, returned))
            }
            SyntaxExpressionKind::If => {
                if let Some(expected) = expected {
                    self.expected_type_for_expr(value, expected, context)?;
                }
                let ExprKind::If(if_expr) = &value.kind else {
                    unreachable!("validated CST if return value kind");
                };
                let dst = self.alloc_register()?;
                let returned = self.compile_if_value_to(if_expr, dst)?;
                Ok((dst, returned))
            }
            SyntaxExpressionKind::Match => {
                if let Some(expected) = expected {
                    self.expected_type_for_expr(value, expected, context)?;
                }
                let ExprKind::Match(match_expr) = &value.kind else {
                    unreachable!("validated CST match return value kind");
                };
                let dst = self.alloc_register()?;
                let returned = self.compile_match_value_to(match_expr, dst)?;
                Ok((dst, returned))
            }
            _ => self.compile_return_expr_legacy(value, expected, context),
        }
    }

    fn compile_return_expr_legacy(
        &mut self,
        value: &Expr,
        expected: Option<super::value_types::RuntimeTypeFact>,
        context: TypeContractContext,
    ) -> CompileResult<(Register, bool)> {
        match expected {
            Some(expected) => self
                .compile_expr_with_expected_type(value, expected, context)
                .map(|register| (register, false)),
            None => self.compile_expr(value).map(|register| (register, false)),
        }
    }

    fn compile_for(
        &mut self,
        stmt_span: Span,
        index_pattern: Option<&Pattern>,
        pattern: &Pattern,
        iterable: &Expr,
        body: &Block,
        iterable_operator: Option<BinaryOp>,
    ) -> CompileResult<bool> {
        let range_iterable = iterable_operator
            .and_then(|operator| cst_range_iterable(operator, iterable))
            .or_else(|| legacy_range_iterable(iterable));
        let item_facts = if range_iterable.is_some() {
            i64_pattern_facts()
        } else {
            PatternBindingFacts::value_shape(
                self.value_shape_for_expr(iterable)
                    .and_then(iterable_item_shape),
            )
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
            let iterable_register = self.compile_expr(iterable)?;
            let iterator = self.alloc_register()?;
            self.emit_spanned(
                UnlinkedInstructionKind::IterInit {
                    dst: iterator,
                    iterable: iterable_register,
                },
                iterable.span,
            );
            LoopIterable::Generic { iterator }
        };

        let item_register = self.alloc_register()?;
        let loop_index = if index_pattern.is_some() {
            let counter = self.alloc_register()?;
            self.emit_constant_to(counter, Constant::Scalar(vela_common::ScalarValue::I64(0)));
            Some((
                counter,
                self.emit_constant(Constant::Scalar(vela_common::ScalarValue::I64(1)))?,
            ))
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
        let previous_value_shapes = self.value_shapes.clone();

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
            self.emit(UnlinkedInstructionKind::Move {
                dst: index_register,
                src: counter,
            });
            self.emit(UnlinkedInstructionKind::Add {
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
                i64_pattern_facts(),
                LocalBindingKind::For,
            )?;
        }
        mismatch_jumps.extend(self.compile_match_pattern(item_register, pattern)?);
        self.bind_pattern_locals(
            item_register,
            pattern,
            stmt_span,
            item_facts,
            LocalBindingKind::For,
        )?;
        self.loop_stack.push(LoopContext::new(loop_start));
        let body_returned = self.compile_statements(&body.statements)?;
        let loop_context = self
            .loop_stack
            .pop()
            .expect("loop context pushed before compiling for body");
        if !body_returned {
            self.emit(UnlinkedInstructionKind::Jump {
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
        self.value_shapes = previous_value_shapes;

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
                self.emit(UnlinkedInstructionKind::Move { dst, src: value });
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

    fn compile_if(
        &mut self,
        if_expr: &IfExpr,
        condition_operator: Option<BinaryOp>,
    ) -> CompileResult<bool> {
        let jump_to_else =
            self.emit_condition_jump_if_false(&if_expr.condition, condition_operator)?;

        let then_returned = self.compile_statements(&if_expr.then_branch.statements)?;
        let jump_to_end = if then_returned {
            None
        } else {
            Some(self.emit_jump())
        };

        self.patch_jump(jump_to_else, self.current_offset())?;

        let else_returned = match &if_expr.else_branch {
            Some(ElseBranch::Block(block)) => self.compile_statements(&block.statements)?,
            Some(ElseBranch::If(if_expr)) => self.compile_if(if_expr, None)?,
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
        let jump_to_else = self.emit_condition_jump_if_false(&if_expr.condition, None)?;

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

    fn emit_condition_jump_if_false(
        &mut self,
        condition: &Expr,
        condition_operator: Option<BinaryOp>,
    ) -> CompileResult<usize> {
        if let Some(jump) =
            self.try_emit_i64_immediate_jump_if_false(condition, condition_operator)?
        {
            return Ok(jump);
        }
        let condition = self.compile_expr(condition)?;
        Ok(self.emit_jump_if_false(condition))
    }

    fn try_emit_i64_immediate_jump_if_false(
        &mut self,
        condition: &Expr,
        condition_operator: Option<BinaryOp>,
    ) -> CompileResult<Option<usize>> {
        let ExprKind::Binary { left, right, .. } = &condition.kind else {
            return Ok(None);
        };
        let Some(op) =
            condition_operator_for_fallback(condition_operator, condition).and_then(i64_compare_op)
        else {
            return Ok(None);
        };
        if self.value_type_for_expr(left)
            != Some(RuntimeTypeFact::Primitive(vela_common::PrimitiveTag::I64))
        {
            return Ok(None);
        }
        let Some(imm) = self.i64_literal_value(right)? else {
            return Ok(None);
        };
        let lhs = self.compile_expr(left)?;
        let offset = self.current_offset();
        self.emit(UnlinkedInstructionKind::I64CmpImmJumpIfFalse {
            op,
            lhs,
            imm,
            target: InstructionOffset(usize::MAX),
        });
        Ok(Some(offset))
    }

    fn i64_literal_value(&self, expr: &Expr) -> CompileResult<Option<i64>> {
        let ExprKind::Literal(Literal::Integer(value)) = &expr.kind else {
            return Ok(None);
        };
        let Some(Constant::Scalar(vela_common::ScalarValue::I64(value))) =
            compile_literal_constant_for_type(
                &Literal::Integer(value.clone()),
                vela_common::PrimitiveTag::I64,
            )?
        else {
            return Ok(None);
        };
        Ok(Some(value))
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
            let previous_value_shapes = self.value_shapes.clone();
            self.bind_pattern_locals(
                scrutinee,
                &arm.pattern,
                arm.body.span,
                PatternBindingFacts::new(scrutinee_fact.clone()),
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
            self.value_shapes = previous_value_shapes;
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
            let previous_value_shapes = self.value_shapes.clone();
            self.bind_pattern_locals(
                scrutinee,
                &arm.pattern,
                arm.body.span,
                PatternBindingFacts::new(scrutinee_fact.clone()),
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
            self.value_shapes = previous_value_shapes;
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
                self.emit(UnlinkedInstructionKind::Move { dst, src: value });
                Ok(false)
            }
        }
    }
}

fn iterable_item_shape(shape: ValueShape) -> Option<ValueShape> {
    match shape {
        ValueShape::Array(element) | ValueShape::Set(element) => Some(*element),
        ValueShape::Map { key, value } => Some(ValueShape::map_entry(*key, *value)),
        _ => None,
    }
}

fn i64_pattern_facts() -> PatternBindingFacts {
    PatternBindingFacts::value(Some(RuntimeTypeFact::primitive(PrimitiveTag::I64)))
}

fn legacy_statement_kind(stmt: &Stmt) -> SyntaxStatementKind {
    match &stmt.kind {
        StmtKind::Let { .. } => SyntaxStatementKind::Let,
        StmtKind::Return(_) => SyntaxStatementKind::Return,
        StmtKind::Break => SyntaxStatementKind::Break,
        StmtKind::Continue => SyntaxStatementKind::Continue,
        StmtKind::For { .. } => SyntaxStatementKind::For,
        StmtKind::Block(_) => SyntaxStatementKind::Block,
        StmtKind::Expr(expr) => match &expr.kind {
            ExprKind::If(_) => SyntaxStatementKind::If,
            ExprKind::Match(_) => SyntaxStatementKind::Match,
            _ => SyntaxStatementKind::Expr,
        },
    }
}

fn statement_kind_matches(kind: SyntaxStatementKind, stmt: &Stmt) -> bool {
    kind == legacy_statement_kind(stmt)
}

fn expression_statement_kind_matches(kind: SyntaxExpressionKind, expr: &Expr) -> bool {
    matches!(kind, SyntaxExpressionKind::Assign) == matches!(expr.kind, ExprKind::Assign { .. })
}

fn value_expression_kind_matches(kind: SyntaxExpressionKind, expr: &Expr) -> bool {
    match kind {
        SyntaxExpressionKind::Block => matches!(expr.kind, ExprKind::Block(_)),
        SyntaxExpressionKind::If => matches!(expr.kind, ExprKind::If(_)),
        SyntaxExpressionKind::Match => matches!(expr.kind, ExprKind::Match(_)),
        _ => !matches!(
            expr.kind,
            ExprKind::Block(_) | ExprKind::If(_) | ExprKind::Match(_)
        ),
    }
}

fn cst_range_iterable(operator: BinaryOp, expr: &Expr) -> Option<(&Expr, &Expr, bool)> {
    let ExprKind::Binary { op, left, right } = &expr.kind else {
        return None;
    };
    match (operator, *op) {
        (BinaryOp::Range, BinaryOp::Range) => Some((left.as_ref(), right.as_ref(), false)),
        (BinaryOp::RangeInclusive, BinaryOp::RangeInclusive) => {
            Some((left.as_ref(), right.as_ref(), true))
        }
        _ => None,
    }
}

fn legacy_range_iterable(expr: &Expr) -> Option<(&Expr, &Expr, bool)> {
    match &expr.kind {
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
    }
}

fn condition_operator_for_fallback(
    syntax_operator: Option<BinaryOp>,
    expr: &Expr,
) -> Option<BinaryOp> {
    syntax_operator
        .and_then(|operator| cst_condition_operator(operator, expr))
        .or_else(|| legacy_condition_operator(expr))
}

fn cst_condition_operator(operator: BinaryOp, expr: &Expr) -> Option<BinaryOp> {
    let ExprKind::Binary { op, .. } = &expr.kind else {
        return None;
    };
    (operator == *op).then_some(operator)
}

fn legacy_condition_operator(expr: &Expr) -> Option<BinaryOp> {
    let ExprKind::Binary { op, .. } = &expr.kind else {
        return None;
    };
    Some(*op)
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

fn is_map_or_set_type_hint(hint: &HirTypeHint) -> bool {
    matches!(hint.path.as_slice(), [name] if matches!(name.as_str(), "Map" | "Set"))
}
