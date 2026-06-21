mod block_values;
mod classification;
mod condition_jumps;
mod if_values;
mod value_syntax;

use vela_common::Span;
use vela_hir::binding::LocalBindingKind;
use vela_syntax::ast::{
    BinaryOp, Block, ElseBranch, Expr, ExprKind, IfExpr, MatchExpr, Pattern, Stmt, StmtKind,
    SyntaxExpressionKind, SyntaxStatementKind,
};

use crate::{Constant, InstructionOffset, Register, UnlinkedInstructionKind};

use super::assignments::{AssignmentTargetSyntax, AssignmentValuePayloads, AssignmentValueSyntax};
use super::body_payloads::{
    CompilerBodyPayload, CompilerExpressionPayload, CompilerIfPayload, CompilerMatchArmPayload,
    CompilerStatementPayload,
};
use super::patterns::PatternBindingFacts;
use super::script_types::{ScriptTypeFact, type_hint_script_type};
use super::value_types::{
    RuntimeTypeFact, StaticExprType, TypeContractContext, check_expected_type, type_hint_value_type,
};
use super::{CompileError, CompileErrorKind, CompileResult, Compiler, frame_slot_kind};
use classification::{
    cst_range_iterable, expression_statement_kind_matches, i64_pattern_facts,
    is_map_or_set_type_hint, iterable_item_shape, legacy_range_iterable, legacy_statement_kind,
    merge_type_hint_and_value_fact, statement_kind_matches, value_expression_kind_matches,
};
use value_syntax::ValueSyntaxPayloads;

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

struct ForStatementParts<'ast> {
    stmt_span: Span,
    index_pattern: Option<&'ast Pattern>,
    pattern: &'ast Pattern,
    iterable: &'ast Expr,
    body: &'ast Block,
    iterable_payload: Option<CompilerExpressionPayload<'ast>>,
    iterable_operator: Option<BinaryOp>,
    body_payload: Option<CompilerBodyPayload<'ast>>,
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
            self.compile_let_statement(
                stmt.fallback(),
                stmt.let_initializer_kind(),
                stmt.let_initializer_block_body_payload(),
                stmt.let_initializer_if_payload(),
                stmt.let_initializer_match_arm_payloads(),
                stmt.let_initializer_expression_payload(),
            )
        } else if kind == SyntaxStatementKind::Return {
            self.compile_return_statement(
                stmt.fallback(),
                stmt.return_value_kind(),
                stmt.return_value_block_body_payload(),
                stmt.return_value_if_payload(),
                stmt.return_value_match_arm_payloads(),
                stmt.return_value_expression_payload(),
            )
        } else if kind == SyntaxStatementKind::For {
            self.compile_for_statement(
                stmt.fallback(),
                stmt.for_iterable_expression_payload(),
                stmt.for_iterable_binary_operator(),
                stmt.for_body_payload(),
            )
        } else if kind == SyntaxStatementKind::If {
            self.compile_if_statement(stmt.fallback(), stmt.if_payload())
        } else if kind == SyntaxStatementKind::Match {
            self.compile_match_statement_payload(stmt)
        } else if kind == SyntaxStatementKind::Block {
            self.compile_block_statement_payload(stmt)
        } else if kind == SyntaxStatementKind::Expr {
            self.compile_expr_statement_payload(stmt)
        } else {
            self.compile_statement_as(kind, stmt.fallback())
        }
    }

    fn compile_block_statement_payload(
        &mut self,
        stmt: &CompilerStatementPayload<'_>,
    ) -> CompileResult<bool> {
        let Some(body) = stmt.block_body_payload() else {
            return self.compile_statement_as(SyntaxStatementKind::Block, stmt.fallback());
        };
        let statements = body.statement_payloads();
        self.compile_statement_payloads(&statements)
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
            let value_body = stmt.assignment_value_block_body_payload();
            let value_if = stmt.assignment_value_if_payload();
            let value_match_arms = stmt.assignment_value_match_arm_payloads();
            let value_expression = stmt.assignment_value_expression_payload();
            let target_expression = stmt.assignment_target_expression_payload();
            let value_match_scrutinee = value_expression
                .as_ref()
                .and_then(CompilerExpressionPayload::match_scrutinee_payload);
            self.compile_assignment_with_payloads(
                expr,
                AssignmentTargetSyntax::new(target_expression.as_ref()),
                AssignmentValueSyntax::new(
                    stmt.assignment_value_kind(),
                    value_expression.as_ref(),
                    AssignmentValuePayloads::new(
                        value_body.as_ref(),
                        value_if.as_ref(),
                        value_match_scrutinee.as_ref(),
                        value_match_arms.as_deref(),
                    ),
                ),
            )?;
            Ok(false)
        } else if kind == SyntaxExpressionKind::Call {
            let ExprKind::Call { callee, args } = &expr.kind else {
                return self.compile_expr_statement(expr);
            };
            let callee_payload = stmt.call_callee_payload();
            let argument_payloads = stmt.call_argument_payloads();
            self.compile_call_expr_with_arg_payloads(
                expr,
                callee,
                args,
                callee_payload.as_ref(),
                argument_payloads.as_deref(),
            )?;
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
            SyntaxStatementKind::Let => {
                self.compile_let_statement(stmt, None, None, None, None, None)
            }
            SyntaxStatementKind::Return => {
                self.compile_return_statement(stmt, None, None, None, None, None)
            }
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
            SyntaxStatementKind::For => self.compile_for_statement(stmt, None, None, None),
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
        initializer_body: Option<CompilerBodyPayload<'_>>,
        initializer_if: Option<CompilerIfPayload<'_>>,
        initializer_match_arms: Option<Vec<CompilerMatchArmPayload<'_>>>,
        initializer_expression: Option<CompilerExpressionPayload<'_>>,
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
                ValueSyntaxPayloads::new(
                    initializer_expression.as_ref(),
                    initializer_body.as_ref(),
                    initializer_if.as_ref(),
                    initializer_match_arms.as_deref(),
                ),
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
        value_body: Option<CompilerBodyPayload<'_>>,
        value_if: Option<CompilerIfPayload<'_>>,
        value_match_arms: Option<Vec<CompilerMatchArmPayload<'_>>>,
        value_expression: Option<CompilerExpressionPayload<'_>>,
    ) -> CompileResult<bool> {
        let StmtKind::Return(value) = &stmt.kind else {
            return self.compile_statement(stmt);
        };
        let (register, returned) = self.compile_return_value(
            stmt.span,
            value.as_ref(),
            value_kind,
            ValueSyntaxPayloads::new(
                value_expression.as_ref(),
                value_body.as_ref(),
                value_if.as_ref(),
                value_match_arms.as_deref(),
            ),
        )?;
        if !returned {
            self.emit(UnlinkedInstructionKind::Return { src: register });
        }
        Ok(true)
    }

    fn compile_for_statement<'ast>(
        &mut self,
        stmt: &'ast Stmt,
        iterable_payload: Option<CompilerExpressionPayload<'ast>>,
        iterable_operator: Option<BinaryOp>,
        body_payload: Option<CompilerBodyPayload<'ast>>,
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
        self.compile_for(ForStatementParts {
            stmt_span: stmt.span,
            index_pattern: index_pattern.as_ref(),
            pattern,
            iterable,
            body,
            iterable_payload,
            iterable_operator,
            body_payload,
        })
    }

    fn compile_if_statement(
        &mut self,
        stmt: &Stmt,
        payload: Option<CompilerIfPayload<'_>>,
    ) -> CompileResult<bool> {
        let StmtKind::Expr(expr) = &stmt.kind else {
            return self.compile_statement(stmt);
        };
        let ExprKind::If(if_expr) = &expr.kind else {
            return self.compile_statement(stmt);
        };
        self.compile_if(if_expr, payload.as_ref())
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

    fn compile_match_statement_payload(
        &mut self,
        stmt: &CompilerStatementPayload<'_>,
    ) -> CompileResult<bool> {
        let StmtKind::Expr(expr) = &stmt.fallback().kind else {
            return self.compile_statement_as(SyntaxStatementKind::Match, stmt.fallback());
        };
        let ExprKind::Match(match_expr) = &expr.kind else {
            return self.compile_statement_as(SyntaxStatementKind::Match, stmt.fallback());
        };
        let scrutinee_payload = stmt.match_scrutinee_payload();
        let arm_payloads = stmt.match_arm_payloads();
        self.compile_match_with_payloads(
            match_expr,
            scrutinee_payload.as_ref(),
            arm_payloads.as_deref(),
        )
    }

    fn compile_let_initializer(
        &mut self,
        value: &Expr,
        expected: Option<super::value_types::RuntimeTypeFact>,
        context: TypeContractContext,
        syntax_kind: Option<SyntaxExpressionKind>,
        syntax_payloads: ValueSyntaxPayloads<'_, '_>,
    ) -> CompileResult<(Register, bool)> {
        if let Some(kind) = syntax_kind
            && value_expression_kind_matches(kind, value)
        {
            return self.compile_let_initializer_with_syntax_kind(
                value,
                expected,
                context,
                kind,
                syntax_payloads,
            );
        }
        self.compile_let_initializer_legacy(value, expected, context)
    }

    fn compile_let_initializer_with_syntax_kind(
        &mut self,
        value: &Expr,
        expected: Option<super::value_types::RuntimeTypeFact>,
        context: TypeContractContext,
        kind: SyntaxExpressionKind,
        syntax_payloads: ValueSyntaxPayloads<'_, '_>,
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
                let returned = if let Some(body_payload) = syntax_payloads.block_body {
                    self.compile_block_payload_value_to(body_payload, dst)?
                } else {
                    self.compile_block_value_to(block, dst)?
                };
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
                let returned =
                    self.compile_if_value_with_payloads(if_expr, dst, syntax_payloads.if_expr)?;
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
                let scrutinee_payload = syntax_payloads
                    .expression
                    .and_then(CompilerExpressionPayload::match_scrutinee_payload);
                let returned = self.compile_match_value_with_payloads(
                    match_expr,
                    dst,
                    scrutinee_payload.as_ref(),
                    syntax_payloads.match_arms,
                )?;
                Ok((dst, returned))
            }
            SyntaxExpressionKind::Array
            | SyntaxExpressionKind::Map
            | SyntaxExpressionKind::Record
            | SyntaxExpressionKind::Binary
            | SyntaxExpressionKind::Call
            | SyntaxExpressionKind::Unary
            | SyntaxExpressionKind::Try => self
                .compile_expr_with_optional_expected_type_and_payload(
                    value,
                    expected,
                    context,
                    syntax_payloads.expression,
                )
                .map(|register| (register, false)),
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
        syntax_payloads: ValueSyntaxPayloads<'_, '_>,
    ) -> CompileResult<(Register, bool)> {
        match (value, self.return_type.clone()) {
            (Some(value), Some(expected)) => self.compile_return_expr(
                value,
                Some(expected),
                TypeContractContext::Return,
                syntax_kind,
                syntax_payloads,
            ),
            (Some(value), None) => self.compile_return_expr(
                value,
                None,
                TypeContractContext::Return,
                syntax_kind,
                syntax_payloads,
            ),
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
        syntax_payloads: ValueSyntaxPayloads<'_, '_>,
    ) -> CompileResult<(Register, bool)> {
        if let Some(kind) = syntax_kind
            && value_expression_kind_matches(kind, value)
        {
            return self.compile_return_expr_with_syntax_kind(
                value,
                expected,
                context,
                kind,
                syntax_payloads,
            );
        }
        self.compile_return_expr_legacy(value, expected, context)
    }

    fn compile_return_expr_with_syntax_kind(
        &mut self,
        value: &Expr,
        expected: Option<super::value_types::RuntimeTypeFact>,
        context: TypeContractContext,
        kind: SyntaxExpressionKind,
        syntax_payloads: ValueSyntaxPayloads<'_, '_>,
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
                let returned = if let Some(body_payload) = syntax_payloads.block_body {
                    self.compile_block_payload_value_to(body_payload, dst)?
                } else {
                    self.compile_block_value_to(block, dst)?
                };
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
                let returned =
                    self.compile_if_value_with_payloads(if_expr, dst, syntax_payloads.if_expr)?;
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
                let scrutinee_payload = syntax_payloads
                    .expression
                    .and_then(CompilerExpressionPayload::match_scrutinee_payload);
                let returned = self.compile_match_value_with_payloads(
                    match_expr,
                    dst,
                    scrutinee_payload.as_ref(),
                    syntax_payloads.match_arms,
                )?;
                Ok((dst, returned))
            }
            SyntaxExpressionKind::Array
            | SyntaxExpressionKind::Map
            | SyntaxExpressionKind::Record
            | SyntaxExpressionKind::Binary
            | SyntaxExpressionKind::Call
            | SyntaxExpressionKind::Unary
            | SyntaxExpressionKind::Try => self
                .compile_expr_with_optional_expected_type_and_payload(
                    value,
                    expected,
                    context,
                    syntax_payloads.expression,
                )
                .map(|register| (register, false)),
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

    fn compile_for(&mut self, parts: ForStatementParts<'_>) -> CompileResult<bool> {
        let iterable_operand_payloads = parts
            .iterable_payload
            .as_ref()
            .and_then(CompilerExpressionPayload::binary_operand_payloads);
        let range_iterable = parts
            .iterable_operator
            .and_then(|operator| cst_range_iterable(operator, parts.iterable))
            .or_else(|| legacy_range_iterable(parts.iterable));
        let item_facts = if range_iterable.is_some() {
            i64_pattern_facts()
        } else {
            PatternBindingFacts::value_shape(
                self.value_shape_for_expr(parts.iterable)
                    .and_then(iterable_item_shape),
            )
        };
        let loop_iterable = if let Some((start, end, inclusive)) = range_iterable {
            let (start_payload, end_payload) = iterable_operand_payloads
                .as_ref()
                .map(|(start_payload, end_payload)| (Some(start_payload), Some(end_payload)))
                .unwrap_or((None, None));
            let cursor = self.compile_expr_with_payload(start, start_payload)?;
            let end = self.compile_expr_with_payload(end, end_payload)?;
            let done = self.alloc_register()?;
            self.emit_bool_constant_to(done, false);
            LoopIterable::Range {
                cursor,
                end,
                done,
                inclusive,
            }
        } else {
            let iterable_register =
                self.compile_expr_with_payload(parts.iterable, parts.iterable_payload.as_ref())?;
            let iterator = self.alloc_register()?;
            self.emit_spanned(
                UnlinkedInstructionKind::IterInit {
                    dst: iterator,
                    iterable: iterable_register,
                },
                parts.iterable.span,
            );
            LoopIterable::Generic { iterator }
        };

        let item_register = self.alloc_register()?;
        let loop_index = if parts.index_pattern.is_some() {
            let counter = self.alloc_register()?;
            self.emit_constant_to(counter, Constant::Scalar(vela_common::ScalarValue::I64(0)));
            Some((
                counter,
                self.emit_constant(Constant::Scalar(vela_common::ScalarValue::I64(1)))?,
            ))
        } else {
            None
        };
        let index_register = if parts.index_pattern.is_some() {
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
        if let (Some(index_pattern), Some(index_register)) = (parts.index_pattern, index_register) {
            mismatch_jumps.extend(self.compile_match_pattern(
                index_register,
                index_pattern,
                None,
            )?);
            self.bind_pattern_locals(
                index_register,
                index_pattern,
                None,
                parts.stmt_span,
                i64_pattern_facts(),
                LocalBindingKind::For,
            )?;
        }
        mismatch_jumps.extend(self.compile_match_pattern(item_register, parts.pattern, None)?);
        self.bind_pattern_locals(
            item_register,
            parts.pattern,
            None,
            parts.stmt_span,
            item_facts,
            LocalBindingKind::For,
        )?;
        self.loop_stack.push(LoopContext::new(loop_start));
        let body_returned = if let Some(body_payload) = parts.body_payload {
            let statements = body_payload.statement_payloads();
            self.compile_statement_payloads(&statements)?
        } else {
            self.compile_statements(&parts.body.statements)?
        };
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

    fn compile_if(
        &mut self,
        if_expr: &IfExpr,
        payload: Option<&CompilerIfPayload<'_>>,
    ) -> CompileResult<bool> {
        let jump_to_else = self.emit_condition_jump_if_false(
            &if_expr.condition,
            payload.and_then(CompilerIfPayload::condition_operator),
            payload.and_then(CompilerIfPayload::condition_payload),
        )?;

        let then_returned = self.compile_if_block(
            &if_expr.then_branch,
            payload.and_then(CompilerIfPayload::then_body),
        )?;
        let jump_to_end = if then_returned {
            None
        } else {
            Some(self.emit_jump())
        };

        self.patch_jump(jump_to_else, self.current_offset())?;

        let else_returned = match &if_expr.else_branch {
            Some(ElseBranch::Block(block)) => {
                self.compile_if_block(block, payload.and_then(CompilerIfPayload::else_body))?
            }
            Some(ElseBranch::If(if_expr)) => {
                self.compile_if(if_expr, payload.and_then(CompilerIfPayload::else_if))?
            }
            None => false,
        };

        if let Some(jump_to_end) = jump_to_end {
            self.patch_jump(jump_to_end, self.current_offset())?;
        }

        Ok(then_returned && else_returned)
    }

    fn compile_if_block(
        &mut self,
        block: &Block,
        payload: Option<&CompilerBodyPayload<'_>>,
    ) -> CompileResult<bool> {
        if let Some(payload) = payload {
            let statements = payload.statement_payloads();
            self.compile_statement_payloads(&statements)
        } else {
            self.compile_statements(&block.statements)
        }
    }

    fn compile_match(&mut self, match_expr: &MatchExpr) -> CompileResult<bool> {
        self.compile_match_with_payloads(match_expr, None, None)
    }

    fn compile_match_with_payloads(
        &mut self,
        match_expr: &MatchExpr,
        scrutinee_payload: Option<&CompilerExpressionPayload<'_>>,
        arm_payloads: Option<&[CompilerMatchArmPayload<'_>]>,
    ) -> CompileResult<bool> {
        let scrutinee_fact = self.script_fact_for_expr(&match_expr.scrutinee);
        let scrutinee = self.compile_expr_with_payload(&match_expr.scrutinee, scrutinee_payload)?;
        let mut end_jumps = Vec::new();
        let mut all_arms_return = !match_expr.arms.is_empty();

        for (index, arm) in match_expr.arms.iter().enumerate() {
            let arm_payload = arm_payloads.and_then(|payloads| payloads.get(index));
            let pattern_payload = arm_payload.map(CompilerMatchArmPayload::pattern_payload);
            let mut next_arm_jumps =
                self.compile_match_pattern(scrutinee, &arm.pattern, pattern_payload.as_ref())?;
            let previous_locals = self.locals.clone();
            let previous_hir_locals = self.hir_locals.clone();
            let previous_script_types = self.script_types.clone();
            let previous_value_types = self.value_types.clone();
            let previous_value_shapes = self.value_shapes.clone();
            self.bind_pattern_locals(
                scrutinee,
                &arm.pattern,
                pattern_payload.as_ref(),
                arm.body.span,
                PatternBindingFacts::new(scrutinee_fact.clone()),
                LocalBindingKind::Pattern,
            )?;
            if let Some(jump) = self.compile_match_guard(
                arm.guard.as_ref(),
                arm_payload.and_then(|payload| payload.guard_payload()),
            )? {
                next_arm_jumps.push(jump);
            }
            let arm_returned = self.compile_match_arm_statement(arm, arm_payload)?;
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

    fn compile_match_arm_statement(
        &mut self,
        arm: &vela_syntax::ast::MatchArm,
        payload: Option<&CompilerMatchArmPayload<'_>>,
    ) -> CompileResult<bool> {
        if let Some(body) = payload.and_then(CompilerMatchArmPayload::body_block_payload) {
            let statements = body.statement_payloads();
            return self.compile_statement_payloads(&statements);
        }
        match &arm.body.kind {
            ExprKind::Block(block) => self.compile_statements(&block.statements),
            _ => {
                let body_payload = payload.map(CompilerMatchArmPayload::body_expression_payload);
                self.compile_expr_with_payload(&arm.body, body_payload.as_ref())?;
                Ok(false)
            }
        }
    }

    pub(super) fn compile_match_value_to(
        &mut self,
        match_expr: &MatchExpr,
        dst: Register,
    ) -> CompileResult<bool> {
        self.compile_match_value_with_payloads(match_expr, dst, None, None)
    }

    pub(in crate::compiler) fn compile_match_value_with_payloads(
        &mut self,
        match_expr: &MatchExpr,
        dst: Register,
        scrutinee_payload: Option<&CompilerExpressionPayload<'_>>,
        arm_payloads: Option<&[CompilerMatchArmPayload<'_>]>,
    ) -> CompileResult<bool> {
        let scrutinee_fact = self.script_fact_for_expr(&match_expr.scrutinee);
        let scrutinee = self.compile_expr_with_payload(&match_expr.scrutinee, scrutinee_payload)?;
        let mut end_jumps = Vec::new();
        let mut all_arms_return = !match_expr.arms.is_empty();
        let mut has_catch_all = false;

        for (index, arm) in match_expr.arms.iter().enumerate() {
            let arm_payload = arm_payloads.and_then(|payloads| payloads.get(index));
            let pattern_payload = arm_payload.map(CompilerMatchArmPayload::pattern_payload);
            let mut next_arm_jumps =
                self.compile_match_pattern(scrutinee, &arm.pattern, pattern_payload.as_ref())?;
            let previous_locals = self.locals.clone();
            let previous_hir_locals = self.hir_locals.clone();
            let previous_script_types = self.script_types.clone();
            let previous_value_types = self.value_types.clone();
            let previous_value_shapes = self.value_shapes.clone();
            self.bind_pattern_locals(
                scrutinee,
                &arm.pattern,
                pattern_payload.as_ref(),
                arm.body.span,
                PatternBindingFacts::new(scrutinee_fact.clone()),
                LocalBindingKind::Pattern,
            )?;
            if let Some(jump) = self.compile_match_guard(
                arm.guard.as_ref(),
                arm_payload.and_then(|payload| payload.guard_payload()),
            )? {
                next_arm_jumps.push(jump);
            }
            let arm_returned = self.compile_match_arm_value_to(&arm.body, arm_payload, dst)?;
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

    fn compile_match_guard(
        &mut self,
        guard: Option<&Expr>,
        payload: Option<CompilerExpressionPayload<'_>>,
    ) -> CompileResult<Option<usize>> {
        let Some(guard) = guard else {
            return Ok(None);
        };
        let condition = self.compile_expr_with_payload(guard, payload.as_ref())?;
        Ok(Some(self.emit_jump_if_false(condition)))
    }

    fn compile_match_arm_value_to(
        &mut self,
        body: &Expr,
        payload: Option<&CompilerMatchArmPayload<'_>>,
        dst: Register,
    ) -> CompileResult<bool> {
        if let Some(body) = payload.and_then(CompilerMatchArmPayload::body_block_payload) {
            return self.compile_block_payload_value_to(&body, dst);
        }
        match &body.kind {
            ExprKind::Block(block) => self.compile_block_value_to(block, dst),
            _ => {
                let body_payload = payload.map(CompilerMatchArmPayload::body_expression_payload);
                let value = self.compile_expr_with_payload(body, body_payload.as_ref())?;
                self.emit(UnlinkedInstructionKind::Move { dst, src: value });
                Ok(false)
            }
        }
    }
}
