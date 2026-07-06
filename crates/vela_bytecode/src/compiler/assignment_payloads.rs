use vela_common::Span;
use vela_syntax::ast::SyntaxExpressionKind;

use crate::compiler::body_payloads::CompilerExpressionPayload;
use crate::compiler::error::{CompileError, CompileErrorKind, CompileResult};

pub(in crate::compiler) fn validate_assignment_target_payload(
    target_span: Span,
    target_kind: Option<SyntaxExpressionKind>,
    target_path_is_self: Option<bool>,
    payload: Option<&CompilerExpressionPayload<'_>>,
) -> CompileResult<()> {
    if payload.is_some_and(|payload| {
        !payload_span_overlaps(payload, target_span)
            || payload.stored_syntax_kind() != target_kind
            || target_path_is_self.is_some_and(|is_self| payload.syntax_is_self() != is_self)
    }) {
        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "mismatched CST assignment target",
        )));
    }
    Ok(())
}

pub(in crate::compiler) fn validate_assignment_value_payload(
    value_span: Span,
    payload: Option<&CompilerExpressionPayload<'_>>,
) -> CompileResult<()> {
    if payload.is_some_and(|payload| !payload_span_overlaps(payload, value_span)) {
        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "mismatched CST assignment value",
        )));
    }
    Ok(())
}

fn payload_span_overlaps(payload: &CompilerExpressionPayload<'_>, span: Span) -> bool {
    payload
        .syntax_span()
        .is_some_and(|payload_span| payload_span.start < span.end && span.start < payload_span.end)
}
