use vela_syntax::ast::{Expr, ExprKind, SyntaxExpressionKind};

pub(super) fn expression_payload_kind_matches(kind: SyntaxExpressionKind, expr: &Expr) -> bool {
    match kind {
        SyntaxExpressionKind::Block => matches!(expr.kind, ExprKind::Block(_)),
        SyntaxExpressionKind::If => matches!(expr.kind, ExprKind::If(_)),
        SyntaxExpressionKind::Match => matches!(expr.kind, ExprKind::Match(_)),
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
