use vela_common::Span;
use vela_syntax::ast::{Expr, ExprKind, SyntaxExpressionKind};

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
    let Some(kind) = payload.stored_syntax_kind() else {
        return true;
    };
    if kind == SyntaxExpressionKind::Literal {
        return matches!(
            expr.kind,
            ExprKind::Literal(_) | ExprKind::InterpolatedString(_)
        ) && expression_payload_shape_matches_expr(payload, expr);
    }
    payload.fallback_expr_matches_stored_syntax_kind()
        && payload.fallback_expr_matches_expr(expr)
        && expression_payload_shape_matches_expr(payload, expr)
}

fn expression_payload_shape_matches_expr(
    payload: &CompilerExpressionPayload<'_>,
    expr: &Expr,
) -> bool {
    if payload.stored_syntax_kind() != Some(SyntaxExpressionKind::Path) {
        return true;
    }
    match &expr.kind {
        ExprKind::Path(_) => !payload.syntax_is_self(),
        ExprKind::SelfValue => payload.syntax_is_self(),
        _ => false,
    }
}

fn expression_payload_overlaps_span(payload: &CompilerExpressionPayload<'_>, span: Span) -> bool {
    payload
        .syntax_span()
        .is_some_and(|syntax_span| spans_overlap(syntax_span, span))
}

fn spans_overlap(left: Span, right: Span) -> bool {
    left.start < right.end && right.start < left.end
}
