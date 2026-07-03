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
        && expression_matches_payload_fallback_expr(payload, expr)
        && expression_payload_shape_matches_expr(payload, expr)
}

fn expression_matches_payload_fallback_expr(
    payload: &CompilerExpressionPayload<'_>,
    expr: &Expr,
) -> bool {
    match fallback_expr_syntax_kind(expr) {
        Some(SyntaxExpressionKind::Paren) => true,
        Some(kind) => payload.fallback_expr_matches_syntax_kind(kind),
        None => payload.fallback_expr_is_error(),
    }
}

fn fallback_expr_syntax_kind(expr: &Expr) -> Option<SyntaxExpressionKind> {
    Some(match expr.kind {
        ExprKind::Literal(_) | ExprKind::InterpolatedString(_) => SyntaxExpressionKind::Literal,
        ExprKind::Path(_) | ExprKind::SelfValue => SyntaxExpressionKind::Path,
        ExprKind::Block(_) => SyntaxExpressionKind::Block,
        ExprKind::If(_) => SyntaxExpressionKind::If,
        ExprKind::Match(_) => SyntaxExpressionKind::Match,
        ExprKind::Assign { .. } => SyntaxExpressionKind::Assign,
        ExprKind::Unary { .. } => SyntaxExpressionKind::Unary,
        ExprKind::Try(_) => SyntaxExpressionKind::Try,
        ExprKind::Binary { .. } => SyntaxExpressionKind::Binary,
        ExprKind::Call { .. } => SyntaxExpressionKind::Call,
        ExprKind::Field { .. } => SyntaxExpressionKind::Field,
        ExprKind::Index { .. } => SyntaxExpressionKind::Index,
        ExprKind::Lambda { .. } => SyntaxExpressionKind::Lambda,
        ExprKind::Array(_) => SyntaxExpressionKind::Array,
        ExprKind::Map(_) => SyntaxExpressionKind::Map,
        ExprKind::Record { .. } => SyntaxExpressionKind::Record,
        ExprKind::Error => return None,
    })
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
