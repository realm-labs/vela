use vela_common::PrimitiveTag;
use vela_hir::type_hint::HirTypeHint;
use vela_syntax::ast::{BinaryOp, Expr, ExprKind, Stmt, StmtKind, SyntaxStatementKind};

use crate::compiler::body_payloads::CompilerExpressionPayload;
use crate::compiler::patterns::PatternBindingFacts;
use crate::compiler::record_shapes::ValueShape;
use crate::compiler::script_types::ScriptTypeFact;
use crate::compiler::value_types::RuntimeTypeFact;

pub(super) fn iterable_item_shape(shape: ValueShape) -> Option<ValueShape> {
    match shape {
        ValueShape::Array(element) | ValueShape::Set(element) => Some(*element),
        ValueShape::Map { key, value } => Some(ValueShape::map_entry(*key, *value)),
        _ => None,
    }
}

pub(super) fn i64_pattern_facts() -> PatternBindingFacts {
    PatternBindingFacts::value(Some(RuntimeTypeFact::primitive(PrimitiveTag::I64)))
}

pub(super) fn fallback_statement_kind(stmt: &Stmt) -> SyntaxStatementKind {
    match &stmt.kind {
        StmtKind::Let { .. } => SyntaxStatementKind::Let,
        StmtKind::Return(_) => SyntaxStatementKind::Return,
        StmtKind::Break => SyntaxStatementKind::Break,
        StmtKind::Continue => SyntaxStatementKind::Continue,
        StmtKind::For { .. } => SyntaxStatementKind::For,
        StmtKind::Block(_) => SyntaxStatementKind::Block,
        StmtKind::Expr(expr) => match &expr.kind {
            ExprKind::If(_) => SyntaxStatementKind::If,
            ExprKind::Match(_) => SyntaxStatementKind::Match,
            _ => SyntaxStatementKind::Expr,
        },
    }
}

pub(super) fn value_expression_requires_matching_syntax(expr: &Expr) -> bool {
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

pub(super) fn control_flow_expression_requires_matching_syntax(expr: &Expr) -> bool {
    matches!(
        expr.kind,
        ExprKind::Block(_)
            | ExprKind::If(_)
            | ExprKind::Match(_)
            | ExprKind::Array(_)
            | ExprKind::Map(_)
            | ExprKind::Record { .. }
            | ExprKind::Binary { .. }
            | ExprKind::Path(_)
            | ExprKind::SelfValue
    )
}

pub(super) fn range_iterable_for_payload<'ast>(
    payload: Option<&CompilerExpressionPayload<'_>>,
    expr: &'ast Expr,
) -> Option<(&'ast Expr, &'ast Expr, bool)> {
    let ExprKind::Binary { left, right, .. } = &expr.kind else {
        return None;
    };
    match (
        payload.and_then(CompilerExpressionPayload::syntax_binary_operator),
        payload.is_some(),
    ) {
        (Some(BinaryOp::Range), _) => Some((left.as_ref(), right.as_ref(), false)),
        (Some(BinaryOp::RangeInclusive), _) => Some((left.as_ref(), right.as_ref(), true)),
        _ => None,
    }
}

pub(super) fn condition_operator_for_payload(
    payload: Option<&CompilerExpressionPayload<'_>>,
    expr: &Expr,
) -> Option<BinaryOp> {
    if matches!(expr.kind, ExprKind::Binary { .. }) {
        match (
            payload.and_then(CompilerExpressionPayload::syntax_binary_operator),
            payload.is_some(),
        ) {
            (Some(op), _) => Some(op),
            (None, _) => None,
        }
    } else {
        None
    }
}

pub(super) fn merge_type_hint_and_value_fact(
    hinted: Option<ScriptTypeFact>,
    value: Option<ScriptTypeFact>,
) -> Option<ScriptTypeFact> {
    match (hinted, value) {
        (Some(hinted), Some(value)) if hinted.type_name == value.type_name => {
            Some(ScriptTypeFact {
                type_name: hinted.type_name,
                enum_variant: value.enum_variant,
            })
        }
        (Some(hinted), _) => Some(hinted),
        (None, value) => value,
    }
}

pub(super) fn is_map_or_set_type_hint(hint: &HirTypeHint) -> bool {
    matches!(hint.path.as_slice(), [name] if matches!(name.as_str(), "Map" | "Set"))
}
