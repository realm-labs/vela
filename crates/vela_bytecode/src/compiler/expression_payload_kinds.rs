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
    payload.fallback_kind_matches_stored_syntax_kind()
        && payload.fallback_kind_matches_expr(expr)
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

pub(super) fn expression_requires_matching_payload(expr: &Expr) -> bool {
    matches!(
        expr.kind,
        ExprKind::Block(_)
            | ExprKind::If(_)
            | ExprKind::Match(_)
            | ExprKind::Array(_)
            | ExprKind::Map(_)
            | ExprKind::Record { .. }
            | ExprKind::Path(_)
            | ExprKind::SelfValue
    )
}

pub(super) fn expression_rejects_missing_payload(expr: &Expr) -> bool {
    expression_requires_matching_payload(expr)
        || matches!(
            expr.kind,
            ExprKind::Assign { .. }
                | ExprKind::Binary { .. }
                | ExprKind::Call { .. }
                | ExprKind::Field { .. }
                | ExprKind::Index { .. }
                | ExprKind::InterpolatedString(_)
                | ExprKind::Lambda { .. }
                | ExprKind::Literal(_)
                | ExprKind::Path(_)
                | ExprKind::SelfValue
                | ExprKind::Try(_)
                | ExprKind::Unary { .. }
        )
}

fn expression_payload_overlaps_span(payload: &CompilerExpressionPayload<'_>, span: Span) -> bool {
    payload
        .syntax_span()
        .is_some_and(|syntax_span| spans_overlap(syntax_span, span))
}

fn spans_overlap(left: Span, right: Span) -> bool {
    left.start < right.end && right.start < left.end
}
