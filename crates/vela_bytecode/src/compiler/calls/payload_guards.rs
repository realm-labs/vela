use vela_syntax::ast::{Expr, SyntaxExpressionKind};

use crate::compiler::body_payloads::{CompilerArgumentPayload, CompilerExpressionPayload};
use crate::compiler::{CompileError, CompileErrorKind, CompileResult};

pub(super) fn callback_lambda_payload_is_authoritative(
    arg_payload: Option<&CompilerExpressionPayload<'_>>,
    arg_value: &Expr,
) -> bool {
    let Some(payload) = arg_payload else {
        return true;
    };
    match payload.syntax_kind() {
        Some(SyntaxExpressionKind::Lambda) => payload.is_aligned_with_paired_expr(arg_value),
        Some(_) => false,
        None => true,
    }
}

pub(super) fn reject_missing_callback_lambda_body(
    arg_payload: Option<&CompilerExpressionPayload<'_>>,
    body_payload: Option<&CompilerExpressionPayload<'_>>,
) -> CompileResult<()> {
    if arg_payload.is_some() && body_payload.is_none() {
        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "missing CST lambda body",
        )));
    }
    Ok(())
}

pub(super) fn reject_mismatched_call_callee_payload(
    callee: &Expr,
    callee_payload: Option<&CompilerExpressionPayload<'_>>,
) -> CompileResult<()> {
    let Some(payload) = callee_payload else {
        return Ok(());
    };
    if payload.is_aligned_with_paired_expr(callee) {
        Ok(())
    } else {
        Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "mismatched CST call callee payload",
        )))
    }
}

pub(super) fn reject_missing_call_callee_payload(
    callee_payload: Option<&CompilerExpressionPayload<'_>>,
) -> CompileResult<()> {
    if callee_payload.is_some_and(|payload| payload.syntax_expression().is_none()) {
        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "missing CST call callee",
        )));
    }
    Ok(())
}

pub(super) fn reject_mismatched_call_argument_payloads(
    callee_payload: Option<&CompilerExpressionPayload<'_>>,
    arg_payloads: Option<&[CompilerArgumentPayload]>,
) -> CompileResult<()> {
    if callee_payload.is_some() && arg_payloads.is_none() {
        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "mismatched CST call arguments",
        )));
    }
    Ok(())
}
