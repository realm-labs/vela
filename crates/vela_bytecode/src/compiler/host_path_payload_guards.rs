use vela_syntax::ast::{Expr, ExprKind, SyntaxExpressionKind};

use crate::compiler::body_payloads::CompilerExpressionPayload;

pub(super) fn payload_matches_host_path_expression(
    payload: &CompilerExpressionPayload<'_>,
    expr: &Expr,
) -> bool {
    let Some(payload_span) = payload.syntax_span() else {
        return false;
    };
    if payload_span.start >= expr.span.end || expr.span.start >= payload_span.end {
        return false;
    }
    let Some(payload_kind) = payload.stored_syntax_kind() else {
        return true;
    };
    let Some(expr_kind) = host_path_expression_kind(expr) else {
        return false;
    };
    (payload_kind == expr_kind || payload_kind == SyntaxExpressionKind::Paren)
        && (expr_kind != SyntaxExpressionKind::Path
            || payload.syntax_is_self() == matches!(expr.kind, ExprKind::SelfValue))
}

fn host_path_expression_kind(expr: &Expr) -> Option<SyntaxExpressionKind> {
    match &expr.kind {
        ExprKind::Literal(_) | ExprKind::InterpolatedString(_) => {
            Some(SyntaxExpressionKind::Literal)
        }
        ExprKind::Path(_) | ExprKind::SelfValue => Some(SyntaxExpressionKind::Path),
        ExprKind::Unary { .. } => Some(SyntaxExpressionKind::Unary),
        ExprKind::Binary { .. } => Some(SyntaxExpressionKind::Binary),
        ExprKind::Assign { .. } => Some(SyntaxExpressionKind::Assign),
        ExprKind::Field { .. } => Some(SyntaxExpressionKind::Field),
        ExprKind::Call { .. } => Some(SyntaxExpressionKind::Call),
        ExprKind::Index { .. } => Some(SyntaxExpressionKind::Index),
        ExprKind::Try(_) => Some(SyntaxExpressionKind::Try),
        ExprKind::Array(_) => Some(SyntaxExpressionKind::Array),
        ExprKind::Map(_) => Some(SyntaxExpressionKind::Map),
        ExprKind::Record { .. } => Some(SyntaxExpressionKind::Record),
        ExprKind::Lambda { .. } => Some(SyntaxExpressionKind::Lambda),
        ExprKind::Block(_) => Some(SyntaxExpressionKind::Block),
        ExprKind::If(_) => Some(SyntaxExpressionKind::If),
        ExprKind::Match(_) => Some(SyntaxExpressionKind::Match),
        ExprKind::Error => None,
    }
}
