use vela_syntax::ast::{
    Argument, Block, ElseBranch, Expr, ExprKind, IfExpr, ItemKind, MatchArm, RecordField,
    SourceFile, Stmt, StmtKind,
};

pub(super) fn for_each_explicit_record_field(
    parsed: &SourceFile,
    mut visit: impl FnMut(&[String], &RecordField),
) {
    for item in &parsed.items {
        match &item.kind {
            ItemKind::Const(item) => visit_expr_record_fields(&item.value, &mut visit),
            ItemKind::Function(item) => visit_block_record_fields(&item.body, &mut visit),
            ItemKind::Struct(item) => {
                for field in &item.fields {
                    if let Some(value) = &field.default_value {
                        visit_expr_record_fields(value, &mut visit);
                    }
                }
            }
            ItemKind::Trait(item) => {
                for method in &item.methods {
                    if let Some(body) = &method.default_body {
                        visit_block_record_fields(body, &mut visit);
                    }
                }
            }
            ItemKind::Impl(item) => {
                for method in &item.methods {
                    visit_block_record_fields(&method.function.body, &mut visit);
                }
            }
            ItemKind::Use(_) | ItemKind::Global(_) | ItemKind::Enum(_) => {}
        }
    }
}

fn visit_block_record_fields(block: &Block, visit: &mut impl FnMut(&[String], &RecordField)) {
    for statement in &block.statements {
        visit_statement_record_fields(statement, visit);
    }
}

fn visit_statement_record_fields(
    statement: &Stmt,
    visit: &mut impl FnMut(&[String], &RecordField),
) {
    match &statement.kind {
        StmtKind::Let { value, .. } | StmtKind::Return(value) => {
            if let Some(value) = value {
                visit_expr_record_fields(value, visit);
            }
        }
        StmtKind::For { iterable, body, .. } => {
            visit_expr_record_fields(iterable, visit);
            visit_block_record_fields(body, visit);
        }
        StmtKind::Expr(expr) => visit_expr_record_fields(expr, visit),
        StmtKind::Block(block) => visit_block_record_fields(block, visit),
        StmtKind::Break | StmtKind::Continue => {}
    }
}

fn visit_expr_record_fields(expr: &Expr, visit: &mut impl FnMut(&[String], &RecordField)) {
    match &expr.kind {
        ExprKind::Record { path, fields } => {
            for field in fields {
                if field.value.is_some() {
                    visit(path, field);
                }
                if let Some(value) = &field.value {
                    visit_expr_record_fields(value, visit);
                }
            }
        }
        ExprKind::InterpolatedString(parts) => {
            for part in parts {
                if let vela_syntax::ast::InterpolatedStringPart::Expr(expr) = part {
                    visit_expr_record_fields(expr, visit);
                }
            }
        }
        ExprKind::Unary { expr, .. } | ExprKind::Try(expr) => {
            visit_expr_record_fields(expr, visit);
        }
        ExprKind::Binary { left, right, .. } => {
            visit_expr_record_fields(left, visit);
            visit_expr_record_fields(right, visit);
        }
        ExprKind::Assign { target, value, .. } => {
            visit_expr_record_fields(target, visit);
            visit_expr_record_fields(value, visit);
        }
        ExprKind::Field { base, .. } => visit_expr_record_fields(base, visit),
        ExprKind::Call { callee, args } => {
            visit_expr_record_fields(callee, visit);
            for argument in args {
                visit_argument_record_fields(argument, visit);
            }
        }
        ExprKind::Index { base, index } => {
            visit_expr_record_fields(base, visit);
            visit_expr_record_fields(index, visit);
        }
        ExprKind::Array(values) => {
            for value in values {
                visit_expr_record_fields(value, visit);
            }
        }
        ExprKind::Map(entries) => {
            for entry in entries {
                visit_expr_record_fields(&entry.key, visit);
                visit_expr_record_fields(&entry.value, visit);
            }
        }
        ExprKind::Lambda { body, .. } => visit_expr_record_fields(body, visit),
        ExprKind::If(if_expr) => visit_if_record_fields(if_expr, visit),
        ExprKind::Match(match_expr) => {
            visit_expr_record_fields(&match_expr.scrutinee, visit);
            for arm in &match_expr.arms {
                visit_match_arm_record_fields(arm, visit);
            }
        }
        ExprKind::Block(block) => visit_block_record_fields(block, visit),
        ExprKind::Literal(_) | ExprKind::Path(_) | ExprKind::SelfValue | ExprKind::Error => {}
    }
}

fn visit_argument_record_fields(
    argument: &Argument,
    visit: &mut impl FnMut(&[String], &RecordField),
) {
    visit_expr_record_fields(&argument.value, visit);
}

fn visit_if_record_fields(if_expr: &IfExpr, visit: &mut impl FnMut(&[String], &RecordField)) {
    visit_expr_record_fields(&if_expr.condition, visit);
    visit_block_record_fields(&if_expr.then_branch, visit);
    if let Some(else_branch) = &if_expr.else_branch {
        match else_branch {
            ElseBranch::If(if_expr) => visit_if_record_fields(if_expr, visit),
            ElseBranch::Block(block) => visit_block_record_fields(block, visit),
        }
    }
}

fn visit_match_arm_record_fields(arm: &MatchArm, visit: &mut impl FnMut(&[String], &RecordField)) {
    if let Some(guard) = &arm.guard {
        visit_expr_record_fields(guard, visit);
    }
    visit_expr_record_fields(&arm.body, visit);
}
