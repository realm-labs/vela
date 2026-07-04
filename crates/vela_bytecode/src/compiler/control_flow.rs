mod block_statement_values;
mod block_values;
mod classification;
mod condition_jumps;
mod if_values;
mod literal_statement_values;
mod loops;
mod matches;
mod path_values;
mod range_statement_values;
mod statements;
mod syntax_statement_values;
mod value_syntax;

use vela_common::{PrimitiveTag, Span};
use vela_hir::binding::LocalBindingKind;
use vela_syntax::ast::{
    Block, ElseBranch, Expr, ExprKind, IfExpr, Stmt, StmtKind, SyntaxExpressionKind,
    SyntaxStatementKind,
};

use crate::{Constant, InstructionOffset, Register, UnlinkedInstructionKind};

#[cfg(test)]
use super::assignments::{AssignmentTargetSyntax, AssignmentValuePayloads, AssignmentValueSyntax};
use super::body_payloads::{
    CompilerBodyPayload, CompilerExpressionPayload, CompilerIfPayload, CompilerPatternPayload,
    CompilerStatementPayload,
};
use super::patterns::PatternBindingFacts;
use super::script_types::{ScriptTypeFact, type_hint_script_type};
use super::value_types::{
    RuntimeTypeFact, StaticExprType, TypeContractContext, check_expected_type, type_hint_value_type,
};
use super::{CompileError, CompileErrorKind, CompileResult, Compiler, frame_slot_kind};
use classification::{
    control_flow_expression_requires_matching_syntax, i64_pattern_facts, is_map_or_set_type_hint,
    iterable_item_shape, merge_type_hint_and_value_fact, range_iterable_for_payload,
    statement_kind_for_stmt, value_expression_requires_matching_syntax,
};
pub(super) use loops::LoopContext;
use loops::{ForStatementParts, LoopIterable};
use value_syntax::ValueSyntaxPayloads;

impl Compiler<'_, '_> {
    #[cfg(test)]
    pub(in crate::compiler) fn compile_let_initializer_value_payload_for_test(
        &mut self,
        value: &Expr,
        expression: Option<&CompilerExpressionPayload<'_>>,
    ) -> CompileResult<(Register, bool)> {
        self.compile_let_initializer(
            value,
            None,
            TypeContractContext::TypedLet {
                name: "value".to_owned(),
            },
            ValueSyntaxPayloads::new(
                expression.and_then(CompilerExpressionPayload::kind),
                expression,
                None,
                None,
                None,
                false,
            ),
        )
    }

    #[cfg(test)]
    pub(in crate::compiler) fn compile_let_initializer_kind_without_expression_payload_for_test(
        &mut self,
        value: &Expr,
        kind: SyntaxExpressionKind,
    ) -> CompileResult<(Register, bool)> {
        self.compile_let_initializer(
            value,
            None,
            TypeContractContext::TypedLet {
                name: "value".to_owned(),
            },
            ValueSyntaxPayloads::new(Some(kind), None, None, None, None, false),
        )
    }

    #[cfg(test)]
    pub(in crate::compiler) fn compile_return_value_payload_for_test(
        &mut self,
        value: &Expr,
        expression: Option<&CompilerExpressionPayload<'_>>,
    ) -> CompileResult<(Register, bool)> {
        self.compile_return_expr(
            value,
            None,
            TypeContractContext::Return,
            ValueSyntaxPayloads::new(
                expression.and_then(CompilerExpressionPayload::kind),
                expression,
                None,
                None,
                None,
                false,
            ),
        )
    }

    #[cfg(test)]
    pub(in crate::compiler) fn compile_return_kind_without_expression_payload_for_test(
        &mut self,
        value: &Expr,
        kind: SyntaxExpressionKind,
    ) -> CompileResult<(Register, bool)> {
        self.compile_return_expr(
            value,
            None,
            TypeContractContext::Return,
            ValueSyntaxPayloads::new(Some(kind), None, None, None, None, false),
        )
    }

    fn compile_block_statement_payload(
        &mut self,
        stmt: &CompilerStatementPayload<'_>,
    ) -> CompileResult<bool> {
        let Some(body) = stmt.block_body_payload() else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "missing CST block statement body payload",
            )));
        };
        self.compile_body_payload_statements(&body)
    }

    fn compile_expr_statement_payload(
        &mut self,
        stmt: &CompilerStatementPayload<'_>,
    ) -> CompileResult<bool> {
        let Some(kind) = stmt.stored_expression_kind() else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "missing CST expression statement payload",
            )));
        };
        let syntax_payload_is_trusted = expression_statement_syntax_payload_is_trusted(stmt);
        if syntax_payload_is_trusted
            && let Some((source, expression)) = stmt.expression_statement_syntax_expression()
            && let Some(done) = self.compile_syntax_constant_expr_statement(source, &expression)?
        {
            return Ok(done);
        }
        if syntax_payload_is_trusted
            && let Some((source, expression)) = stmt.expression_statement_syntax_expression()
            && let Some(done) = self.compile_syntax_path_expr_statement(source, &expression)?
        {
            return Ok(done);
        }
        if syntax_payload_is_trusted
            && let Some((source, expression)) = stmt.expression_statement_syntax_expression()
            && let Some(done) = self.compile_syntax_range_expr_statement(source, &expression)?
        {
            return Ok(done);
        }
        if syntax_payload_is_trusted
            && let Some(body) = stmt.expression_statement_block_body_payload()
        {
            let dst = self.alloc_register()?;
            return self.compile_block_payload_value_to(&body, dst);
        }
        if syntax_payload_is_trusted
            && let Some((source, expression)) = stmt.expression_statement_syntax_expression()
            && let Some(done) = self.compile_syntax_value_expr_statement(source, &expression)?
        {
            return Ok(done);
        }
        #[cfg(test)]
        {
            let Some(expression_payload) = stmt.expression_payload() else {
                return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                    "missing CST expression statement payload",
                )));
            };
            let expr = expression_payload.fallback();
            return if kind == SyntaxExpressionKind::Assign {
                let value_expression = expression_payload.assignment_value_payload();
                let value_body = value_expression
                    .as_ref()
                    .and_then(CompilerExpressionPayload::block_body_payload);
                let value_if = value_expression
                    .as_ref()
                    .and_then(CompilerExpressionPayload::if_payload);
                let value_match_arms = value_expression
                    .as_ref()
                    .and_then(CompilerExpressionPayload::match_arm_payloads);
                let target_expression = expression_payload.assignment_target_payload();
                let value_match_scrutinee = value_expression
                    .as_ref()
                    .and_then(CompilerExpressionPayload::match_scrutinee_payload);
                self.compile_assignment_with_payloads(
                    expr,
                    AssignmentTargetSyntax::new(target_expression.as_ref()),
                    AssignmentValueSyntax::new(
                        value_expression
                            .as_ref()
                            .and_then(CompilerExpressionPayload::syntax_kind),
                        expression_payload.syntax_assignment_operator(),
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
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST expression statement payload",
                    )));
                };
                let callee_payload = expression_payload.call_callee_payload();
                let argument_payloads = expression_payload.call_argument_payloads();
                self.compile_call_expr_with_arg_payloads(
                    expr,
                    callee,
                    args,
                    callee_payload.as_ref(),
                    argument_payloads.as_deref(),
                )?;
                Ok(false)
            } else {
                self.compile_expr_with_payload(expr, Some(&expression_payload))?;
                Ok(false)
            };
        }
        #[cfg_attr(test, allow(unreachable_code))]
        {
            let _ = kind;
            Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "unsupported CST expression statement payload",
            )))
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
        self.compile_statement_as(statement_kind_for_stmt(stmt), stmt)
    }

    fn compile_statement_as(
        &mut self,
        kind: SyntaxStatementKind,
        stmt: &Stmt,
    ) -> CompileResult<bool> {
        match kind {
            SyntaxStatementKind::Let => self.compile_let_statement(
                stmt,
                ValueSyntaxPayloads::new(None, None, None, None, None, false),
            ),
            SyntaxStatementKind::Return => self.compile_return_statement(
                stmt,
                ValueSyntaxPayloads::new(None, None, None, None, None, false),
            ),
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
            SyntaxStatementKind::For => self.compile_for_statement(stmt, None, None, None, None),
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
        syntax_payloads: ValueSyntaxPayloads<'_, '_>,
    ) -> CompileResult<bool> {
        let StmtKind::Let {
            name,
            type_hint: _,
            value,
        } = &stmt.kind
        else {
            return self.compile_statement(stmt);
        };
        self.compile_let_binding(name.clone(), stmt.span, value.as_ref(), syntax_payloads)
    }

    pub(in crate::compiler::control_flow) fn compile_let_without_initializer(
        &mut self,
        name: String,
        span: Span,
    ) -> CompileResult<bool> {
        self.compile_let_binding(
            name,
            span,
            None,
            ValueSyntaxPayloads::new(None, None, None, None, None, true),
        )
    }

    fn compile_let_binding(
        &mut self,
        name: String,
        span: Span,
        value: Option<&Expr>,
        syntax_payloads: ValueSyntaxPayloads<'_, '_>,
    ) -> CompileResult<bool> {
        let value = if syntax_payloads.syntax_value_missing {
            None
        } else {
            value
        };
        let local_binding = self
            .bindings
            .local_named_at(&name, LocalBindingKind::Let, span)
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
        let value_script_fact = value.and_then(|value| {
            self.script_fact_for_expr_with_payload(value, syntax_payloads.expression)
        });
        let script_hint_proven = hinted_script_fact
            .as_ref()
            .zip(value_script_fact.as_ref())
            .is_some_and(|(hint, value)| hint == value);
        let script_fact = merge_type_hint_and_value_fact(hinted_script_fact, value_script_fact);
        let hinted_value_type = hir_type_hint.and_then(type_hint_value_type);
        let value_type = value.and_then(|value| {
            self.value_type_for_expr_with_payload(value, syntax_payloads.expression)
        });
        let value_type = hinted_value_type.clone().or(value_type);
        let value_shape = value.and_then(|value| {
            self.value_shape_for_expr_with_payload(value, syntax_payloads.expression)
        });
        let (register, returned) = if let Some(value) = value {
            self.compile_let_initializer(
                value,
                hinted_value_type.clone(),
                TypeContractContext::TypedLet { name: name.clone() },
                syntax_payloads,
            )?
        } else {
            (self.emit_constant(Constant::Null)?, false)
        };
        if let (Some(value), Some(hint), None) = (value, hir_type_hint, hinted_value_type.as_ref())
            && is_map_or_set_type_hint(hint)
            && !script_hint_proven
            && let Some(guard) = super::type_guard_for_hint(
                hint,
                crate::GuardLocation::Local,
                name.clone(),
                &self.facts,
            )
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
                Some(span),
            );
            self.script_types
                .set_local_fact(local, name.clone(), script_fact);
            self.value_types.set_local(local, name.clone(), value_type);
            self.value_shapes.set_local(local, name, value_shape);
        } else {
            self.record_frame_slot(
                name.clone(),
                register,
                frame_slot_kind(LocalBindingKind::Let),
                None,
                Some(span),
            );
            self.script_types.set_name_fact(name.clone(), script_fact);
            self.value_types.set_name(name.clone(), value_type);
            self.value_shapes.set_name(name, value_shape);
        }
        Ok(returned)
    }

    fn compile_return_statement(
        &mut self,
        stmt: &Stmt,
        syntax_payloads: ValueSyntaxPayloads<'_, '_>,
    ) -> CompileResult<bool> {
        let StmtKind::Return(value) = &stmt.kind else {
            return self.compile_statement(stmt);
        };
        let (register, returned) =
            self.compile_return_value(stmt.span, value.as_ref(), syntax_payloads)?;
        if !returned {
            self.emit(UnlinkedInstructionKind::Return { src: register });
        }
        Ok(true)
    }

    pub(in crate::compiler::control_flow) fn compile_empty_return(
        &mut self,
        span: Span,
    ) -> CompileResult<bool> {
        let (register, returned) = self.compile_return_value(
            span,
            None,
            ValueSyntaxPayloads::new(None, None, None, None, None, false),
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
        body_payload: Option<CompilerBodyPayload<'ast>>,
        index_pattern_payload: Option<CompilerPatternPayload>,
        pattern_payload: Option<CompilerPatternPayload>,
    ) -> CompileResult<bool> {
        let StmtKind::For {
            index_pattern,
            pattern,
            iterable,
            body: _,
        } = &stmt.kind
        else {
            return self.compile_statement(stmt);
        };
        self.compile_for(ForStatementParts {
            stmt_span: stmt.span,
            index_pattern: index_pattern.as_ref(),
            pattern,
            iterable,
            index_pattern_payload,
            pattern_payload,
            iterable_payload,
            body_payload,
        })
    }

    fn compile_if_statement(
        &mut self,
        stmt: &Stmt,
        payload: Option<&CompilerIfPayload<'_>>,
    ) -> CompileResult<bool> {
        let StmtKind::Expr(expr) = &stmt.kind else {
            return self.compile_statement(stmt);
        };
        let ExprKind::If(if_expr) = &expr.kind else {
            return self.compile_statement(stmt);
        };
        self.compile_if(if_expr, payload)
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

    #[cfg(test)]
    fn compile_match_statement_payload(
        &mut self,
        stmt: &CompilerStatementPayload<'_>,
    ) -> CompileResult<bool> {
        let Some(expression_payload) = stmt.expression_payload() else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "mismatched CST match statement payload",
            )));
        };
        let expr = expression_payload.fallback();
        let ExprKind::Match(match_expr) = &expr.kind else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "mismatched CST match statement payload",
            )));
        };
        let Some(scrutinee_payload) = expression_payload.match_scrutinee_payload() else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "mismatched CST match statement payload",
            )));
        };
        let arm_payloads = expression_payload.match_arm_payloads();
        self.compile_match_with_payloads(
            match_expr,
            Some(&scrutinee_payload),
            arm_payloads.as_deref(),
        )
    }

    fn compile_let_initializer(
        &mut self,
        value: &Expr,
        expected: Option<super::value_types::RuntimeTypeFact>,
        context: TypeContractContext,
        syntax_payloads: ValueSyntaxPayloads<'_, '_>,
    ) -> CompileResult<(Register, bool)> {
        if let Some(kind) = syntax_payloads.kind {
            if syntax_payloads.matches_value(value) {
                return self.compile_let_initializer_with_syntax_kind(
                    value,
                    expected,
                    context,
                    kind,
                    syntax_payloads,
                );
            }
            if value_expression_requires_matching_syntax(value) {
                return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                    "mismatched CST let initializer payload",
                )));
            }
        }
        if syntax_payloads.syntax_value_missing {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "missing CST let initializer payload",
            )));
        }
        if syntax_payloads.has_unclassified_expression_payload()
            || syntax_payloads.has_kind_without_expression_payload()
        {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "missing CST let initializer payload",
            )));
        }
        self.compile_let_initializer_without_payload(value, expected, context)
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
                    self.check_value_payload_type(value, expected, context, syntax_payloads)?;
                }
                let dst = self.alloc_register()?;
                let Some(body_payload) = syntax_payloads.block_body else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST let initializer block body payload",
                    )));
                };
                let returned = self.compile_block_payload_value_to(body_payload, dst)?;
                Ok((dst, returned))
            }
            SyntaxExpressionKind::If => {
                if let Some(expected) = expected {
                    self.check_value_payload_type(value, expected, context, syntax_payloads)?;
                }
                let dst = self.alloc_register()?;
                let ExprKind::If(if_expr) = &value.kind else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST let initializer if payload",
                    )));
                };
                let Some(if_payload) = syntax_payloads.if_expr else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST let initializer if payload",
                    )));
                };
                let returned =
                    self.compile_if_value_with_payloads(if_expr, dst, Some(if_payload))?;
                Ok((dst, returned))
            }
            SyntaxExpressionKind::Match => {
                if let Some(expected) = expected {
                    self.check_value_payload_type(value, expected, context, syntax_payloads)?;
                }
                let dst = self.alloc_register()?;
                let ExprKind::Match(match_expr) = &value.kind else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST let initializer match payload",
                    )));
                };
                let Some(scrutinee_payload) = syntax_payloads
                    .expression
                    .and_then(CompilerExpressionPayload::match_scrutinee_payload)
                else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST let initializer match payload",
                    )));
                };
                let Some(match_arms) = syntax_payloads.match_arms else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST let initializer match arm payloads",
                    )));
                };
                let returned = self.compile_match_value_with_payloads(
                    match_expr,
                    dst,
                    Some(&scrutinee_payload),
                    Some(match_arms),
                )?;
                Ok((dst, returned))
            }
            SyntaxExpressionKind::Array
            | SyntaxExpressionKind::Assign
            | SyntaxExpressionKind::Field
            | SyntaxExpressionKind::Index
            | SyntaxExpressionKind::Lambda
            | SyntaxExpressionKind::Literal
            | SyntaxExpressionKind::Map
            | SyntaxExpressionKind::Paren
            | SyntaxExpressionKind::Path
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
        }
    }

    fn compile_let_initializer_without_payload(
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
        syntax_payloads: ValueSyntaxPayloads<'_, '_>,
    ) -> CompileResult<(Register, bool)> {
        match (value, self.return_type.clone()) {
            (Some(value), Some(expected)) => self.compile_return_expr(
                value,
                Some(expected),
                TypeContractContext::Return,
                syntax_payloads,
            ),
            (Some(value), None) => {
                self.compile_return_expr(value, None, TypeContractContext::Return, syntax_payloads)
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
        syntax_payloads: ValueSyntaxPayloads<'_, '_>,
    ) -> CompileResult<(Register, bool)> {
        if let Some(kind) = syntax_payloads.kind {
            if syntax_payloads.matches_value(value) {
                return self.compile_return_expr_with_syntax_kind(
                    value,
                    expected,
                    context,
                    kind,
                    syntax_payloads,
                );
            }
            if value_expression_requires_matching_syntax(value) {
                return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                    "mismatched CST return value payload",
                )));
            }
        }
        if syntax_payloads.syntax_value_missing {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "missing CST return value payload",
            )));
        }
        if syntax_payloads.has_unclassified_expression_payload()
            || syntax_payloads.has_kind_without_expression_payload()
        {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "missing CST return value payload",
            )));
        }
        self.compile_return_expr_without_payload(value, expected, context)
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
                    self.check_value_payload_type(value, expected, context, syntax_payloads)?;
                }
                let dst = self.alloc_register()?;
                let Some(body_payload) = syntax_payloads.block_body else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST return block body payload",
                    )));
                };
                let returned = self.compile_block_payload_value_to(body_payload, dst)?;
                Ok((dst, returned))
            }
            SyntaxExpressionKind::If => {
                if let Some(expected) = expected {
                    self.check_value_payload_type(value, expected, context, syntax_payloads)?;
                }
                let dst = self.alloc_register()?;
                let ExprKind::If(if_expr) = &value.kind else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST return if payload",
                    )));
                };
                let Some(if_payload) = syntax_payloads.if_expr else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST return if payload",
                    )));
                };
                let returned =
                    self.compile_if_value_with_payloads(if_expr, dst, Some(if_payload))?;
                Ok((dst, returned))
            }
            SyntaxExpressionKind::Match => {
                if let Some(expected) = expected {
                    self.check_value_payload_type(value, expected, context, syntax_payloads)?;
                }
                let dst = self.alloc_register()?;
                let ExprKind::Match(match_expr) = &value.kind else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST return match payload",
                    )));
                };
                let Some(scrutinee_payload) = syntax_payloads
                    .expression
                    .and_then(CompilerExpressionPayload::match_scrutinee_payload)
                else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST return match payload",
                    )));
                };
                let Some(match_arms) = syntax_payloads.match_arms else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST return match arm payloads",
                    )));
                };
                let returned = self.compile_match_value_with_payloads(
                    match_expr,
                    dst,
                    Some(&scrutinee_payload),
                    Some(match_arms),
                )?;
                Ok((dst, returned))
            }
            SyntaxExpressionKind::Array
            | SyntaxExpressionKind::Assign
            | SyntaxExpressionKind::Field
            | SyntaxExpressionKind::Index
            | SyntaxExpressionKind::Lambda
            | SyntaxExpressionKind::Literal
            | SyntaxExpressionKind::Map
            | SyntaxExpressionKind::Paren
            | SyntaxExpressionKind::Path
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
        }
    }

    fn compile_return_expr_without_payload(
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
        if let Some(payload) = parts.iterable_payload.as_ref()
            && !payload.matches_paired_expr(parts.iterable)
            && control_flow_expression_requires_matching_syntax(parts.iterable)
        {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "mismatched CST for iterable payload",
            )));
        }
        let range_iterable =
            range_iterable_for_payload(parts.iterable_payload.as_ref(), parts.iterable);
        let iterable_operand_payloads =
            match (range_iterable.is_some(), parts.iterable_payload.as_ref()) {
                (true, Some(payload)) => {
                    let ExprKind::Binary { .. } = &parts.iterable.kind else {
                        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                            "missing CST range operand payload",
                        )));
                    };
                    Some(payload.binary_operand_payloads().ok_or_else(|| {
                        CompileError::new(CompileErrorKind::UnsupportedSyntax(
                            "missing CST range operand payload",
                        ))
                    })?)
                }
                _ => None,
            };
        let item_facts = if range_iterable.is_some() {
            i64_pattern_facts()
        } else {
            PatternBindingFacts::value_shape(
                self.value_shape_for_expr_with_payload(
                    parts.iterable,
                    parts.iterable_payload.as_ref(),
                )
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
                parts.index_pattern_payload.as_ref(),
            )?);
            self.bind_pattern_locals(
                index_register,
                index_pattern,
                parts.index_pattern_payload.as_ref(),
                parts.stmt_span,
                i64_pattern_facts(),
                LocalBindingKind::For,
            )?;
        }
        mismatch_jumps.extend(self.compile_match_pattern(
            item_register,
            parts.pattern,
            parts.pattern_payload.as_ref(),
        )?);
        self.bind_pattern_locals(
            item_register,
            parts.pattern,
            parts.pattern_payload.as_ref(),
            parts.stmt_span,
            item_facts,
            LocalBindingKind::For,
        )?;
        let Some(body_payload) = parts.body_payload else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "missing CST for statement body payload",
            )));
        };
        self.loop_stack.push(LoopContext::new(loop_start));
        let body_returned = self.compile_body_payload_statements(&body_payload)?;
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
        let condition_payload = required_if_statement_child_payload(
            payload,
            payload.and_then(CompilerIfPayload::condition_payload),
            "missing CST if condition payload",
        )?;
        let jump_to_else =
            self.emit_condition_jump_if_false(&if_expr.condition, condition_payload)?;

        let then_payload = required_if_statement_child_payload(
            payload,
            payload.and_then(CompilerIfPayload::then_body),
            "missing CST if then body payload",
        )?;
        let then_returned = self.compile_if_block(&if_expr.then_branch, then_payload)?;
        let jump_to_end = if then_returned {
            None
        } else {
            Some(self.emit_jump())
        };

        self.patch_jump(jump_to_else, self.current_offset())?;

        let else_returned = match &if_expr.else_branch {
            Some(ElseBranch::Block(block)) => self.compile_if_block(
                block,
                required_if_statement_child_payload(
                    payload,
                    payload.and_then(CompilerIfPayload::else_body),
                    "missing CST if else body payload",
                )?,
            )?,
            Some(ElseBranch::If(if_expr)) => self.compile_if(
                if_expr,
                required_if_statement_child_payload(
                    payload,
                    payload.and_then(CompilerIfPayload::else_if),
                    "missing CST else-if payload",
                )?,
            )?,
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
            self.compile_body_payload_statements(payload)
        } else {
            self.compile_statements(&block.statements)
        }
    }
}

fn expression_statement_syntax_payload_is_trusted(stmt: &CompilerStatementPayload<'_>) -> bool {
    #[cfg(test)]
    {
        stmt.is_syntax_only()
            || stmt
                .expression_payload()
                .is_some_and(|payload| payload.matches_paired_expr(payload.fallback()))
    }
    #[cfg(not(test))]
    {
        let _ = stmt;
        true
    }
}

fn static_type_runtime_fact(static_type: StaticExprType) -> Option<RuntimeTypeFact> {
    match static_type {
        StaticExprType::Exact(fact) => Some(fact),
        StaticExprType::UnsuffixedIntegerLiteral => {
            Some(RuntimeTypeFact::primitive(PrimitiveTag::I64))
        }
        StaticExprType::UnsuffixedFloatLiteral => {
            Some(RuntimeTypeFact::primitive(PrimitiveTag::F64))
        }
        StaticExprType::Dynamic => None,
    }
}

fn required_if_statement_child_payload<'payload, T>(
    parent: Option<&CompilerIfPayload<'_>>,
    child: Option<&'payload T>,
    message: &'static str,
) -> CompileResult<Option<&'payload T>> {
    if parent.is_some() && child.is_none() {
        Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            message,
        )))
    } else {
        Ok(child)
    }
}
