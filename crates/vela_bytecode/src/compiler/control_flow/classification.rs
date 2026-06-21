use vela_common::PrimitiveTag;
use vela_hir::type_hint::HirTypeHint;
use vela_syntax::ast::{
    BinaryOp, Expr, ExprKind, Stmt, StmtKind, SyntaxExpressionKind, SyntaxStatementKind,
};

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

pub(super) fn legacy_statement_kind(stmt: &Stmt) -> SyntaxStatementKind {
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

pub(super) fn statement_kind_matches(kind: SyntaxStatementKind, stmt: &Stmt) -> bool {
    kind == legacy_statement_kind(stmt)
}

pub(super) fn expression_statement_kind_matches(kind: SyntaxExpressionKind, expr: &Expr) -> bool {
    matches!(kind, SyntaxExpressionKind::Assign) == matches!(expr.kind, ExprKind::Assign { .. })
}

pub(super) fn value_expression_kind_matches(kind: SyntaxExpressionKind, expr: &Expr) -> bool {
    match kind {
        SyntaxExpressionKind::Block => matches!(expr.kind, ExprKind::Block(_)),
        SyntaxExpressionKind::If => matches!(expr.kind, ExprKind::If(_)),
        SyntaxExpressionKind::Match => matches!(expr.kind, ExprKind::Match(_)),
        _ => !matches!(
            expr.kind,
            ExprKind::Block(_) | ExprKind::If(_) | ExprKind::Match(_)
        ),
    }
}

pub(super) fn cst_range_iterable(operator: BinaryOp, expr: &Expr) -> Option<(&Expr, &Expr, bool)> {
    let ExprKind::Binary { op, left, right } = &expr.kind else {
        return None;
    };
    match (operator, *op) {
        (BinaryOp::Range, BinaryOp::Range) => Some((left.as_ref(), right.as_ref(), false)),
        (BinaryOp::RangeInclusive, BinaryOp::RangeInclusive) => {
            Some((left.as_ref(), right.as_ref(), true))
        }
        _ => None,
    }
}

pub(super) fn legacy_range_iterable(expr: &Expr) -> Option<(&Expr, &Expr, bool)> {
    match &expr.kind {
        ExprKind::Binary {
            op: BinaryOp::Range,
            left,
            right,
        } => Some((left.as_ref(), right.as_ref(), false)),
        ExprKind::Binary {
            op: BinaryOp::RangeInclusive,
            left,
            right,
        } => Some((left.as_ref(), right.as_ref(), true)),
        _ => None,
    }
}

pub(super) fn condition_operator_for_fallback(
    syntax_operator: Option<BinaryOp>,
    expr: &Expr,
) -> Option<BinaryOp> {
    syntax_operator
        .and_then(|operator| cst_condition_operator(operator, expr))
        .or_else(|| legacy_condition_operator(expr))
}

fn cst_condition_operator(operator: BinaryOp, expr: &Expr) -> Option<BinaryOp> {
    let ExprKind::Binary { op, .. } = &expr.kind else {
        return None;
    };
    (operator == *op).then_some(operator)
}

fn legacy_condition_operator(expr: &Expr) -> Option<BinaryOp> {
    let ExprKind::Binary { op, .. } = &expr.kind else {
        return None;
    };
    Some(*op)
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
