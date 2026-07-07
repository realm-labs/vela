mod block_statement_values;
mod block_values;
mod classification;
#[cfg(test)]
mod condition_jumps;
#[cfg(test)]
mod if_values;
mod literal_statement_values;
mod loops;
#[cfg(test)]
mod matches;
mod null_values;
mod path_values;
mod range_statement_values;
mod statements;
mod syntax_call_args;
mod syntax_constructors;
mod syntax_host_indexes;
mod syntax_if_values;
mod syntax_match_values;
mod syntax_record_values;
mod syntax_statement_values;
#[cfg(test)]
mod value_syntax;

use vela_common::PrimitiveTag;
#[cfg(test)]
use vela_common::Span;
#[cfg(test)]
use vela_hir::binding::LocalBindingKind;
#[cfg(test)]
use vela_syntax::ast::SyntaxExpressionKind;
#[cfg(test)]
use vela_syntax::ast::{Expr, ExprKind};

#[cfg(test)]
use crate::Register;
#[cfg(test)]
use crate::{Constant, UnlinkedInstructionKind};

#[cfg(test)]
use super::assignments::{AssignmentTargetSyntax, AssignmentValueSyntax};
#[cfg(test)]
use super::body_payloads::CompilerExpressionPayload;
use super::body_payloads::CompilerStatementPayload;
use super::script_types::{ScriptTypeFact, type_hint_script_type};
use super::value_types::{
    RuntimeTypeFact, StaticExprType, TypeContractContext, check_expected_type, type_hint_value_type,
};
use super::{CompileError, CompileErrorKind, CompileResult, Compiler, frame_slot_kind};
#[cfg(test)]
use classification::{
    is_map_or_set_type_hint, merge_type_hint_and_value_fact,
    value_expression_requires_matching_syntax,
};
pub(super) use loops::LoopContext;
#[cfg(test)]
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
                expression.and_then(CompilerExpressionPayload::syntax_kind),
                expression,
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
            ValueSyntaxPayloads::new(Some(kind), None, None, None, false),
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
                expression.and_then(CompilerExpressionPayload::syntax_kind),
                expression,
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
            ValueSyntaxPayloads::new(Some(kind), None, None, None, false),
        )
    }

    #[cfg(test)]
    fn compile_value_payload_block_expr_to(
        &mut self,
        expression_payload: Option<&CompilerExpressionPayload<'_>>,
        dst: Register,
        missing_message: &'static str,
    ) -> CompileResult<Option<bool>> {
        let Some(expression_payload) = expression_payload else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                missing_message,
            )));
        };
        let Some(source) = expression_payload.source() else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                missing_message,
            )));
        };
        let Some(expression) = expression_payload.syntax_expression() else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                missing_message,
            )));
        };
        self.compile_syntax_block_expr_to(source, expression, dst)
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
        if let Some((source, expression)) = stmt.expression_statement_syntax_expression()
            && let Some(done) = self.compile_syntax_constant_expr_statement(source, &expression)?
        {
            return Ok(done);
        }
        if let Some((source, expression)) = stmt.expression_statement_syntax_expression()
            && let Some(done) = self.compile_syntax_path_expr_statement(source, &expression)?
        {
            return Ok(done);
        }
        if let Some((source, expression)) = stmt.expression_statement_syntax_expression()
            && let Some(done) = self.compile_syntax_range_expr_statement(source, &expression)?
        {
            return Ok(done);
        }
        if let Some((source, expression)) = stmt.expression_statement_syntax_expression()
            && let Some(done) = self.compile_syntax_block_expr_statement(source, &expression)?
        {
            return Ok(done);
        }
        if stmt.is_syntax_only()
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
                let target_expression = expression_payload.assignment_target_payload();
                self.compile_assignment_with_payloads(
                    expr,
                    AssignmentTargetSyntax::new(target_expression.as_ref()),
                    AssignmentValueSyntax::new(
                        value_expression
                            .as_ref()
                            .and_then(CompilerExpressionPayload::syntax_kind),
                        expression_payload.syntax_assignment_operator(),
                        value_expression.as_ref(),
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

    #[cfg(test)]
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

    #[cfg(test)]
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

    #[cfg(test)]
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
                let Some(returned) = self.compile_value_payload_block_expr_to(
                    syntax_payloads.expression,
                    dst,
                    "missing CST let initializer block body payload",
                )?
                else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST let initializer block body payload",
                    )));
                };
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
                let returned = self.compile_if_value_with_payloads(if_expr, dst, if_payload)?;
                Ok((dst, returned))
            }
            SyntaxExpressionKind::Match => {
                if let Some(expected) = expected {
                    self.check_value_payload_type(value, expected, context, syntax_payloads)?;
                }
                let dst = self.alloc_register()?;
                if let Some(expression_payload) = syntax_payloads.expression
                    && let Some(returned) =
                        self.compile_syntax_match_payload_value_to(expression_payload, dst)?
                {
                    return Ok((dst, returned));
                }
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
                    match_arms,
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

    #[cfg(test)]
    fn compile_let_initializer_without_payload(
        &mut self,
        value: &Expr,
        expected: Option<super::value_types::RuntimeTypeFact>,
        context: TypeContractContext,
    ) -> CompileResult<(Register, bool)> {
        match &value.kind {
            ExprKind::Block(_) | ExprKind::If(_) | ExprKind::Match(_) => Err(CompileError::new(
                CompileErrorKind::UnsupportedSyntax("missing CST let initializer payload"),
            )),
            _ => match expected {
                Some(expected) => self
                    .compile_expr_with_expected_type(value, expected, context)
                    .map(|register| (register, false)),
                None => self.compile_expr(value).map(|register| (register, false)),
            },
        }
    }

    #[cfg(test)]
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

    #[cfg(test)]
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

    #[cfg(test)]
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
                let Some(returned) = self.compile_value_payload_block_expr_to(
                    syntax_payloads.expression,
                    dst,
                    "missing CST return block body payload",
                )?
                else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST return block body payload",
                    )));
                };
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
                let returned = self.compile_if_value_with_payloads(if_expr, dst, if_payload)?;
                Ok((dst, returned))
            }
            SyntaxExpressionKind::Match => {
                if let Some(expected) = expected {
                    self.check_value_payload_type(value, expected, context, syntax_payloads)?;
                }
                let dst = self.alloc_register()?;
                if let Some(expression_payload) = syntax_payloads.expression
                    && let Some(returned) =
                        self.compile_syntax_match_payload_value_to(expression_payload, dst)?
                {
                    return Ok((dst, returned));
                }
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
                    match_arms,
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

    #[cfg(test)]
    fn compile_return_expr_without_payload(
        &mut self,
        value: &Expr,
        expected: Option<super::value_types::RuntimeTypeFact>,
        context: TypeContractContext,
    ) -> CompileResult<(Register, bool)> {
        if matches!(
            value.kind,
            ExprKind::Block(_) | ExprKind::If(_) | ExprKind::Match(_)
        ) {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "missing CST return value payload",
            )));
        }
        match expected {
            Some(expected) => self
                .compile_expr_with_expected_type(value, expected, context)
                .map(|register| (register, false)),
            None => self.compile_expr(value).map(|register| (register, false)),
        }
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
