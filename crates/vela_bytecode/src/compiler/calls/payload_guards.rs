use vela_common::Span;
use vela_syntax::ast::SyntaxExpressionKind;

use crate::compiler::body_payloads::{CompilerArgumentPayload, CompilerExpressionPayload};
use crate::compiler::{CompileError, CompileErrorKind, CompileResult};

pub(super) fn callback_lambda_payload_is_authoritative(
    arg_payload: Option<&CompilerExpressionPayload<'_>>,
    arg_value_span: Span,
    arg_value_kind: Option<SyntaxExpressionKind>,
) -> bool {
    let Some(payload) = arg_payload else {
        return true;
    };
    match payload.stored_syntax_kind() {
        Some(SyntaxExpressionKind::Lambda) => {
            arg_value_kind == Some(SyntaxExpressionKind::Lambda)
                && payload_span_overlaps(payload, arg_value_span)
        }
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
    callee_span: Span,
    callee_kind: Option<SyntaxExpressionKind>,
    callee_path_is_self: Option<bool>,
    callee_payload: Option<&CompilerExpressionPayload<'_>>,
) -> CompileResult<()> {
    let Some(payload) = callee_payload else {
        return Ok(());
    };
    if payload_matches_expression_facts(payload, callee_span, callee_kind, callee_path_is_self) {
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
    arg_count: usize,
    arg_payloads: Option<&[CompilerArgumentPayload]>,
) -> CompileResult<()> {
    if callee_payload.is_some() && arg_payloads.is_none() {
        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "mismatched CST call arguments",
        )));
    }
    if let Some(arg_payloads) = arg_payloads
        && arg_payloads.len() > arg_count
    {
        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "mismatched CST call arguments",
        )));
    }
    Ok(())
}

pub(super) fn payload_matches_expression_facts(
    payload: &CompilerExpressionPayload<'_>,
    span: Span,
    kind: Option<SyntaxExpressionKind>,
    path_is_self: Option<bool>,
) -> bool {
    payload_span_overlaps(payload, span)
        && callee_payload_kind_matches(payload.stored_syntax_kind(), kind)
        && path_is_self.is_none_or(|is_self| payload.syntax_is_self() == is_self)
}

fn callee_payload_kind_matches(
    payload_kind: Option<SyntaxExpressionKind>,
    callee_kind: Option<SyntaxExpressionKind>,
) -> bool {
    payload_kind == callee_kind || payload_kind == Some(SyntaxExpressionKind::Paren)
}

fn payload_span_overlaps(payload: &CompilerExpressionPayload<'_>, span: Span) -> bool {
    payload
        .syntax_span()
        .is_some_and(|payload_span| payload_span.start < span.end && span.start < payload_span.end)
}
