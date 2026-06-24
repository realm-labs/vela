use vela_common::Span;
use vela_syntax::ast::{Expr, ExprKind, SyntaxExpressionKind};

use super::body_payloads::CompilerExpressionPayload;

pub(super) fn expression_payload_kind_matches(kind: SyntaxExpressionKind, expr: &Expr) -> bool {
    match kind {
        SyntaxExpressionKind::Paren => true,
        SyntaxExpressionKind::Block => matches!(expr.kind, ExprKind::Block(_)),
        SyntaxExpressionKind::If => matches!(expr.kind, ExprKind::If(_)),
        SyntaxExpressionKind::Match => matches!(expr.kind, ExprKind::Match(_)),
        SyntaxExpressionKind::Path => matches!(expr.kind, ExprKind::Path(_) | ExprKind::SelfValue),
        SyntaxExpressionKind::Literal => {
            matches!(
                expr.kind,
                ExprKind::Literal(_) | ExprKind::InterpolatedString(_)
            )
        }
        SyntaxExpressionKind::Array => matches!(expr.kind, ExprKind::Array(_)),
        SyntaxExpressionKind::Map => matches!(expr.kind, ExprKind::Map(_)),
        SyntaxExpressionKind::Record => matches!(expr.kind, ExprKind::Record { .. }),
        SyntaxExpressionKind::Binary => matches!(expr.kind, ExprKind::Binary { .. }),
        SyntaxExpressionKind::Call => matches!(expr.kind, ExprKind::Call { .. }),
        SyntaxExpressionKind::Unary => matches!(expr.kind, ExprKind::Unary { .. }),
        SyntaxExpressionKind::Try => matches!(expr.kind, ExprKind::Try(_)),
        SyntaxExpressionKind::Field => matches!(expr.kind, ExprKind::Field { .. }),
        SyntaxExpressionKind::Index => matches!(expr.kind, ExprKind::Index { .. }),
        SyntaxExpressionKind::Lambda => matches!(expr.kind, ExprKind::Lambda { .. }),
        _ => !matches!(
            expr.kind,
            ExprKind::Block(_)
                | ExprKind::If(_)
                | ExprKind::Match(_)
                | ExprKind::Path(_)
                | ExprKind::Literal(_)
                | ExprKind::InterpolatedString(_)
                | ExprKind::Array(_)
                | ExprKind::Map(_)
                | ExprKind::Record { .. }
                | ExprKind::Binary { .. }
                | ExprKind::Call { .. }
                | ExprKind::Unary { .. }
                | ExprKind::Try(_)
                | ExprKind::Field { .. }
                | ExprKind::Index { .. }
                | ExprKind::Lambda { .. }
        ),
    }
}

pub(super) fn expression_payload_is_aligned(
    payload: &CompilerExpressionPayload<'_>,
    expr: &Expr,
) -> bool {
    payload
        .kind()
        .is_none_or(|kind| expression_payload_kind_matches(kind, expr))
        && expression_payload_overlaps_span(payload, expr.span)
}

fn expression_payload_overlaps_span(payload: &CompilerExpressionPayload<'_>, span: Span) -> bool {
    payload
        .syntax_span()
        .is_some_and(|syntax_span| spans_overlap(syntax_span, span))
}

fn spans_overlap(left: Span, right: Span) -> bool {
    left.start < right.end && right.start < left.end
}
