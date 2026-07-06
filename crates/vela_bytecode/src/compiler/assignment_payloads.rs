use vela_syntax::ast::Expr;

use crate::compiler::body_payloads::CompilerExpressionPayload;
use crate::compiler::error::{CompileError, CompileErrorKind, CompileResult};

pub(in crate::compiler) fn validate_assignment_target_payload(
    target: &Expr,
    payload: Option<&CompilerExpressionPayload<'_>>,
) -> CompileResult<()> {
    if payload.is_some_and(|payload| {
        !payload.matches_paired_expr(target) || !payload.syntax_overlaps_span(target.span)
    }) {
        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "mismatched CST assignment target",
        )));
    }
    Ok(())
}

pub(in crate::compiler) fn validate_assignment_value_payload(
    value: &Expr,
    payload: Option<&CompilerExpressionPayload<'_>>,
) -> CompileResult<()> {
    if payload.is_some_and(|payload| {
        !payload.matches_paired_expr(value) || !payload.syntax_overlaps_span(value.span)
    }) {
        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "mismatched CST assignment value",
        )));
    }
    Ok(())
}
