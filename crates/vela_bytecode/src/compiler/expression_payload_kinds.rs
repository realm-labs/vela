use vela_common::Span;
use vela_syntax::ast::Expr;

use super::body_payloads::CompilerExpressionPayload;

pub(super) fn expression_payload_is_aligned(
    payload: &CompilerExpressionPayload<'_>,
    expr: &Expr,
) -> bool {
    expression_payload_matches_expr(payload, expr)
        && expression_payload_overlaps_span(payload, expr.span)
}

pub(super) fn expression_payload_matches_expr(
    payload: &CompilerExpressionPayload<'_>,
    expr: &Expr,
) -> bool {
    payload.fallback_expr_matches_stored_syntax_expr(expr)
}

fn expression_payload_overlaps_span(payload: &CompilerExpressionPayload<'_>, span: Span) -> bool {
    payload
        .syntax_span()
        .is_some_and(|syntax_span| spans_overlap(syntax_span, span))
}

fn spans_overlap(left: Span, right: Span) -> bool {
    left.start < right.end && right.start < left.end
}
