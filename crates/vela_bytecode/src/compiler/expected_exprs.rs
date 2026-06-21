use vela_common::Span;
use vela_syntax::ast::{Expr, ExprKind, Literal};

use crate::{
    GuardKind, GuardLocation, Register, UnlinkedGuardContext, UnlinkedInstructionKind,
    UnlinkedTypeGuard,
};

use super::body_payloads::CompilerExpressionPayload;
use super::const_eval::compile_literal_constant_for_type;
use super::value_types::{ExpectedTypeOutcome, RuntimeTypeFact, TypeContractContext};
use super::{CompileResult, Compiler};

impl Compiler<'_, '_> {
    pub(super) fn compile_expr_with_expected_type(
        &mut self,
        expr: &Expr,
        expected: RuntimeTypeFact,
        context: TypeContractContext,
    ) -> CompileResult<Register> {
        self.compile_expr_with_expected_type_and_payload(expr, expected, context, None)
    }

    pub(super) fn compile_expr_with_expected_type_and_payload(
        &mut self,
        expr: &Expr,
        expected: RuntimeTypeFact,
        context: TypeContractContext,
        payload: Option<&CompilerExpressionPayload<'_>>,
    ) -> CompileResult<Register> {
        let outcome =
            self.expected_type_for_expr_with_payload(expr, expected, context.clone(), payload)?;
        if let ExpectedTypeOutcome::Contextualized(RuntimeTypeFact::Primitive(tag)) = &outcome
            && let Some((literal, span)) = contextual_literal_payload(expr, payload)
            && let Some(constant) = compile_literal_constant_for_type(&literal, *tag)
                .map_err(|error| error.with_span(span))?
        {
            return self.emit_constant(constant);
        }
        let register = self.compile_expr_with_payload(expr, payload)?;
        if let ExpectedTypeOutcome::RequiresRuntimeGuard(expected) = &outcome
            && let Some((location, name)) = guard_location_and_name(context)
            && let Some(plan) = super::type_guard_plan_for_runtime_type(expected)
        {
            self.emit_spanned(
                UnlinkedInstructionKind::GuardType {
                    src: register,
                    guard: UnlinkedTypeGuard::new(
                        plan,
                        UnlinkedGuardContext::new(GuardKind::Contract, location, name),
                    ),
                },
                expr.span,
            );
        }
        Ok(register)
    }

    pub(super) fn compile_expr_with_optional_expected_type_and_payload(
        &mut self,
        expr: &Expr,
        expected: Option<RuntimeTypeFact>,
        context: TypeContractContext,
        payload: Option<&CompilerExpressionPayload<'_>>,
    ) -> CompileResult<Register> {
        match expected {
            Some(expected) => {
                self.compile_expr_with_expected_type_and_payload(expr, expected, context, payload)
            }
            None => self.compile_expr_with_payload(expr, payload),
        }
    }
}

fn contextual_literal_payload(
    expr: &Expr,
    payload: Option<&CompilerExpressionPayload<'_>>,
) -> Option<(Literal, Span)> {
    payload
        .and_then(|payload| {
            let literal = payload.syntax_literal()?;
            Some((literal, payload.syntax_span().unwrap_or(expr.span)))
        })
        .or_else(|| match &expr.kind {
            ExprKind::Literal(literal) => Some((literal.clone(), expr.span)),
            _ => None,
        })
}

fn guard_location_and_name(context: TypeContractContext) -> Option<(GuardLocation, String)> {
    match context {
        TypeContractContext::TypedLet { name } => Some((GuardLocation::Local, name)),
        TypeContractContext::Field { name } => Some((GuardLocation::Field, name)),
        TypeContractContext::NativeParameter { name, index, .. } => {
            Some((GuardLocation::Parameter { index }, name))
        }
        TypeContractContext::FunctionParameter { .. } | TypeContractContext::Return => None,
    }
}
