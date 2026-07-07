use vela_common::Span;
use vela_syntax::ast::SyntaxExpressionKind;

use crate::compiler::body_payloads::{CompilerArgumentPayload, CompilerExpressionPayload};
#[cfg(not(test))]
use crate::compiler::expression_facts::payload_overlaps_span;
#[cfg(test)]
use crate::compiler::expression_facts::{ExpressionFacts, payload_matches_expression_facts};
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

#[cfg(test)]
pub(super) fn reject_mismatched_call_callee_payload(
    callee: ExpressionFacts,
    callee_payload: Option<&CompilerExpressionPayload<'_>>,
) -> CompileResult<()> {
    let Some(payload) = callee_payload else {
        return Ok(());
    };
    if payload_matches_expression_facts(payload, callee) {
        Ok(())
    } else {
        Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "mismatched CST call callee payload",
        )))
    }
}

#[cfg(not(test))]
pub(super) fn reject_mismatched_call_callee_payload(
    callee_span: Span,
    callee_payload: Option<&CompilerExpressionPayload<'_>>,
) -> CompileResult<()> {
    let Some(payload) = callee_payload else {
        return Ok(());
    };
    if payload_overlaps_span(payload, callee_span) {
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
        && arg_payloads.len() != arg_count
    {
        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "mismatched CST call arguments",
        )));
    }
    Ok(())
}

fn payload_span_overlaps(payload: &CompilerExpressionPayload<'_>, span: Span) -> bool {
    payload
        .syntax_span()
        .is_some_and(|payload_span| payload_span.start < span.end && span.start < payload_span.end)
}
