use vela_common::Span;
use vela_syntax::ast::{Expr, ExprKind, SyntaxExpressionKind};

use crate::compiler::body_payloads::CompilerExpressionPayload;

pub(super) fn expression_syntax_kind(expr: &Expr) -> Option<SyntaxExpressionKind> {
    Some(match expr.kind {
        ExprKind::Path(_) | ExprKind::SelfValue => SyntaxExpressionKind::Path,
        ExprKind::Field { .. } => SyntaxExpressionKind::Field,
        ExprKind::Index { .. } => SyntaxExpressionKind::Index,
        ExprKind::Assign { .. } => SyntaxExpressionKind::Assign,
        ExprKind::Call { .. } => SyntaxExpressionKind::Call,
        ExprKind::Unary { .. } => SyntaxExpressionKind::Unary,
        ExprKind::Binary { .. } => SyntaxExpressionKind::Binary,
        ExprKind::Try(_) => SyntaxExpressionKind::Try,
        ExprKind::Array(_) => SyntaxExpressionKind::Array,
        ExprKind::Map(_) => SyntaxExpressionKind::Map,
        ExprKind::Record { .. } => SyntaxExpressionKind::Record,
        ExprKind::Lambda { .. } => SyntaxExpressionKind::Lambda,
        ExprKind::Block(_) => SyntaxExpressionKind::Block,
        ExprKind::If(_) => SyntaxExpressionKind::If,
        ExprKind::Match(_) => SyntaxExpressionKind::Match,
        ExprKind::Literal(_) | ExprKind::InterpolatedString(_) => SyntaxExpressionKind::Literal,
        ExprKind::Error => return None,
    })
}

pub(super) fn expression_path_is_self(expr: &Expr) -> Option<bool> {
    match expr.kind {
        ExprKind::Path(_) => Some(false),
        ExprKind::SelfValue => Some(true),
        _ => None,
    }
}

pub(super) fn payload_kind_matches_expression(
    payload: &CompilerExpressionPayload<'_>,
    expr: &Expr,
) -> bool {
    payload_kind_matches_known_expression_facts(
        payload.stored_syntax_kind(),
        expression_syntax_kind(expr),
        expression_path_is_self(expr),
        payload.syntax_is_self(),
    )
}

pub(super) fn sourced_payload_kind_matches_expression(
    payload: &CompilerExpressionPayload<'_>,
    expr: &Expr,
) -> bool {
    payload_kind_matches_known_expression_facts(
        payload.syntax_kind(),
        expression_syntax_kind(expr),
        expression_path_is_self(expr),
        payload.syntax_is_self(),
    )
}

pub(super) fn payload_matches_expression_facts(
    payload: &CompilerExpressionPayload<'_>,
    span: Span,
    kind: Option<SyntaxExpressionKind>,
    path_is_self: Option<bool>,
) -> bool {
    payload
        .syntax_span()
        .is_some_and(|payload_span| payload_span.start < span.end && span.start < payload_span.end)
        && payload_kind_matches_expression_facts(
            payload.stored_syntax_kind(),
            kind,
            path_is_self,
            payload.syntax_is_self(),
        )
}

pub(super) fn payload_overlaps_expression_facts(
    payload: &CompilerExpressionPayload<'_>,
    span: Span,
    kind: Option<SyntaxExpressionKind>,
    path_is_self: Option<bool>,
    missing_kind_matches: bool,
) -> bool {
    payload
        .syntax_span()
        .is_some_and(|payload_span| payload_span.start < span.end && span.start < payload_span.end)
        && match payload.stored_syntax_kind() {
            Some(payload_kind) => payload_kind_matches_expression_facts(
                Some(payload_kind),
                kind,
                path_is_self,
                payload.syntax_is_self(),
            ),
            None => missing_kind_matches,
        }
}

fn payload_kind_matches_expression_facts(
    payload_kind: Option<SyntaxExpressionKind>,
    expr_kind: Option<SyntaxExpressionKind>,
    path_is_self: Option<bool>,
    payload_is_self: bool,
) -> bool {
    (payload_kind == expr_kind || payload_kind == Some(SyntaxExpressionKind::Paren))
        && path_is_self.is_none_or(|is_self| is_self == payload_is_self)
}

fn payload_kind_matches_known_expression_facts(
    payload_kind: Option<SyntaxExpressionKind>,
    expr_kind: Option<SyntaxExpressionKind>,
    path_is_self: Option<bool>,
    payload_is_self: bool,
) -> bool {
    if payload_kind == Some(SyntaxExpressionKind::Paren) {
        return true;
    }
    payload_kind.is_some()
        && payload_kind_matches_expression_facts(
            payload_kind,
            expr_kind,
            path_is_self,
            payload_is_self,
        )
}
