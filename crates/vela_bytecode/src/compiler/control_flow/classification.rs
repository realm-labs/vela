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
        SyntaxExpressionKind::Array => matches!(expr.kind, ExprKind::Array(_)),
        SyntaxExpressionKind::Map => matches!(expr.kind, ExprKind::Map(_)),
        SyntaxExpressionKind::Record => matches!(expr.kind, ExprKind::Record { .. }),
        SyntaxExpressionKind::Binary => matches!(expr.kind, ExprKind::Binary { .. }),
        SyntaxExpressionKind::Call => matches!(expr.kind, ExprKind::Call { .. }),
        SyntaxExpressionKind::Unary => matches!(expr.kind, ExprKind::Unary { .. }),
        SyntaxExpressionKind::Try => matches!(expr.kind, ExprKind::Try(_)),
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
        ),
    }
}

pub(super) fn range_iterable_for_payload(
    syntax_operator: Option<BinaryOp>,
    expr: &Expr,
) -> Option<(&Expr, &Expr, bool)> {
    let ExprKind::Binary { left, right, .. } = &expr.kind else {
        return None;
    };
    match syntax_operator {
        Some(BinaryOp::Range) => Some((left.as_ref(), right.as_ref(), false)),
        Some(BinaryOp::RangeInclusive) => Some((left.as_ref(), right.as_ref(), true)),
        Some(_) => None,
        None => legacy_range_iterable(expr),
    }
}

fn legacy_range_iterable(expr: &Expr) -> Option<(&Expr, &Expr, bool)> {
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
    has_payload: bool,
    expr: &Expr,
) -> Option<BinaryOp> {
    if matches!(expr.kind, ExprKind::Binary { .. }) {
        match (syntax_operator, has_payload) {
            (Some(op), _) => Some(op),
            (None, false) => legacy_condition_operator(expr),
            (None, true) => None,
        }
    } else {
        None
    }
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
