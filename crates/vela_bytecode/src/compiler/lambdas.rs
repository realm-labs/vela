use std::collections::{BTreeMap, HashMap};

use vela_hir::{BindingMap, BindingResolution, HirLocalId};
use vela_syntax::{
    Argument, Block, ElseBranch, Expr, ExprKind, IfExpr, MapEntry, MatchArm, MatchExpr,
    RecordField, Stmt, StmtKind,
};

use crate::Register;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LambdaCapture {
    pub local: HirLocalId,
    pub name: String,
    pub register: Register,
}

pub(crate) fn collect_lambda_captures(
    bindings: &BindingMap,
    available: &HashMap<HirLocalId, Register>,
    body: &Expr,
) -> Vec<LambdaCapture> {
    let mut captures = BTreeMap::new();
    collect_expr(bindings, available, body, &mut captures);
    captures.into_values().collect()
}

fn collect_expr(
    bindings: &BindingMap,
    available: &HashMap<HirLocalId, Register>,
    expr: &Expr,
    captures: &mut BTreeMap<HirLocalId, LambdaCapture>,
) {
    match &expr.kind {
        ExprKind::Path(path) => {
            let Some(BindingResolution::Local(local)) = bindings.resolution_at_span(expr.span)
            else {
                return;
            };
            let Some(register) = available.get(local).copied() else {
                return;
            };
            let Some(name) = path.first() else {
                return;
            };
            captures.entry(*local).or_insert_with(|| LambdaCapture {
                local: *local,
                name: name.clone(),
                register,
            });
        }
        ExprKind::Unary { expr, .. } | ExprKind::Try(expr) => {
            collect_expr(bindings, available, expr, captures);
        }
        ExprKind::Binary { left, right, .. } => {
            collect_expr(bindings, available, left, captures);
            collect_expr(bindings, available, right, captures);
        }
        ExprKind::Assign { target, value, .. } => {
            collect_expr(bindings, available, target, captures);
            collect_expr(bindings, available, value, captures);
        }
        ExprKind::Field { base, .. } => collect_expr(bindings, available, base, captures),
        ExprKind::Call { callee, args } => {
            collect_expr(bindings, available, callee, captures);
            for arg in args {
                collect_argument(bindings, available, arg, captures);
            }
        }
        ExprKind::Index { base, index } => {
            collect_expr(bindings, available, base, captures);
            collect_expr(bindings, available, index, captures);
        }
        ExprKind::Array(items) => {
            for item in items {
                collect_expr(bindings, available, item, captures);
            }
        }
        ExprKind::Map(entries) => {
            for entry in entries {
                collect_map_entry(bindings, available, entry, captures);
            }
        }
        ExprKind::Record { fields, .. } => {
            for field in fields {
                collect_record_field(bindings, available, field, captures);
            }
        }
        ExprKind::If(if_expr) => collect_if(bindings, available, if_expr, captures),
        ExprKind::Match(match_expr) => collect_match(bindings, available, match_expr, captures),
        ExprKind::Block(block) => collect_block(bindings, available, block, captures),
        ExprKind::Lambda { .. } => {}
        ExprKind::Literal(_) | ExprKind::SelfValue | ExprKind::Error => {}
    }
}

fn collect_argument(
    bindings: &BindingMap,
    available: &HashMap<HirLocalId, Register>,
    argument: &Argument,
    captures: &mut BTreeMap<HirLocalId, LambdaCapture>,
) {
    collect_expr(bindings, available, &argument.value, captures);
}

fn collect_map_entry(
    bindings: &BindingMap,
    available: &HashMap<HirLocalId, Register>,
    entry: &MapEntry,
    captures: &mut BTreeMap<HirLocalId, LambdaCapture>,
) {
    collect_expr(bindings, available, &entry.key, captures);
    collect_expr(bindings, available, &entry.value, captures);
}

fn collect_record_field(
    bindings: &BindingMap,
    available: &HashMap<HirLocalId, Register>,
    field: &RecordField,
    captures: &mut BTreeMap<HirLocalId, LambdaCapture>,
) {
    if let Some(value) = &field.value {
        collect_expr(bindings, available, value, captures);
    }
}

fn collect_if(
    bindings: &BindingMap,
    available: &HashMap<HirLocalId, Register>,
    if_expr: &IfExpr,
    captures: &mut BTreeMap<HirLocalId, LambdaCapture>,
) {
    collect_expr(bindings, available, &if_expr.condition, captures);
    collect_block(bindings, available, &if_expr.then_branch, captures);
    match &if_expr.else_branch {
        Some(ElseBranch::If(if_expr)) => collect_if(bindings, available, if_expr, captures),
        Some(ElseBranch::Block(block)) => collect_block(bindings, available, block, captures),
        None => {}
    }
}

fn collect_match(
    bindings: &BindingMap,
    available: &HashMap<HirLocalId, Register>,
    match_expr: &MatchExpr,
    captures: &mut BTreeMap<HirLocalId, LambdaCapture>,
) {
    collect_expr(bindings, available, &match_expr.scrutinee, captures);
    for arm in &match_expr.arms {
        collect_match_arm(bindings, available, arm, captures);
    }
}

fn collect_match_arm(
    bindings: &BindingMap,
    available: &HashMap<HirLocalId, Register>,
    arm: &MatchArm,
    captures: &mut BTreeMap<HirLocalId, LambdaCapture>,
) {
    if let Some(guard) = &arm.guard {
        collect_expr(bindings, available, guard, captures);
    }
    collect_expr(bindings, available, &arm.body, captures);
}

fn collect_block(
    bindings: &BindingMap,
    available: &HashMap<HirLocalId, Register>,
    block: &Block,
    captures: &mut BTreeMap<HirLocalId, LambdaCapture>,
) {
    for statement in &block.statements {
        collect_statement(bindings, available, statement, captures);
    }
}

fn collect_statement(
    bindings: &BindingMap,
    available: &HashMap<HirLocalId, Register>,
    statement: &Stmt,
    captures: &mut BTreeMap<HirLocalId, LambdaCapture>,
) {
    match &statement.kind {
        StmtKind::Let { value, .. } | StmtKind::Return(value) => {
            if let Some(value) = value {
                collect_expr(bindings, available, value, captures);
            }
        }
        StmtKind::For { iterable, body, .. } => {
            collect_expr(bindings, available, iterable, captures);
            collect_block(bindings, available, body, captures);
        }
        StmtKind::Expr(expr) => collect_expr(bindings, available, expr, captures),
        StmtKind::Block(block) => collect_block(bindings, available, block, captures),
        StmtKind::Break | StmtKind::Continue => {}
    }
}
