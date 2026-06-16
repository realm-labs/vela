use vela_syntax::ast::{
    Argument, Block, ElseBranch, Expr, ExprKind, IfExpr, ItemKind, MatchArm, Pattern,
    RecordPatternField, SourceFile, Stmt, StmtKind,
};

pub(super) fn for_each_record_variant_pattern_field(
    parsed: &SourceFile,
    mut visit: impl FnMut(&[String], &RecordPatternField),
) {
    for item in &parsed.items {
        match &item.kind {
            ItemKind::Function(item) => visit_block_patterns(&item.body, &mut visit),
            ItemKind::Trait(item) => {
                for method in &item.methods {
                    if let Some(body) = &method.default_body {
                        visit_block_patterns(body, &mut visit);
                    }
                }
            }
            ItemKind::Impl(item) => {
                for method in &item.methods {
                    visit_block_patterns(&method.function.body, &mut visit);
                }
            }
            ItemKind::Const(item) => visit_expr_patterns(&item.value, &mut visit),
            ItemKind::Struct(item) => {
                for field in &item.fields {
                    if let Some(value) = &field.default_value {
                        visit_expr_patterns(value, &mut visit);
                    }
                }
            }
            ItemKind::Use(_) | ItemKind::Global(_) | ItemKind::Enum(_) => {}
        }
    }
}

fn visit_block_patterns(block: &Block, visit: &mut impl FnMut(&[String], &RecordPatternField)) {
    for statement in &block.statements {
        visit_statement_patterns(statement, visit);
    }
}

fn visit_statement_patterns(
    statement: &Stmt,
    visit: &mut impl FnMut(&[String], &RecordPatternField),
) {
    match &statement.kind {
        StmtKind::Let { value, .. } | StmtKind::Return(value) => {
            if let Some(value) = value {
                visit_expr_patterns(value, visit);
            }
        }
        StmtKind::For {
            index_pattern,
            pattern,
            iterable,
            body,
        } => {
            if let Some(index_pattern) = index_pattern {
                visit_pattern_fields(index_pattern, visit);
            }
            visit_pattern_fields(pattern, visit);
            visit_expr_patterns(iterable, visit);
            visit_block_patterns(body, visit);
        }
        StmtKind::Expr(expr) => visit_expr_patterns(expr, visit),
        StmtKind::Block(block) => visit_block_patterns(block, visit),
        StmtKind::Break | StmtKind::Continue => {}
    }
}

fn visit_expr_patterns(expr: &Expr, visit: &mut impl FnMut(&[String], &RecordPatternField)) {
    match &expr.kind {
        ExprKind::Match(match_expr) => {
            visit_expr_patterns(&match_expr.scrutinee, visit);
            for arm in &match_expr.arms {
                visit_match_arm_patterns(arm, visit);
            }
        }
        ExprKind::InterpolatedString(parts) => {
            for part in parts {
                if let vela_syntax::ast::InterpolatedStringPart::Expr(expr) = part {
                    visit_expr_patterns(expr, visit);
                }
            }
        }
        ExprKind::Unary { expr, .. } | ExprKind::Try(expr) => visit_expr_patterns(expr, visit),
        ExprKind::Binary { left, right, .. } => {
            visit_expr_patterns(left, visit);
            visit_expr_patterns(right, visit);
        }
        ExprKind::Assign { target, value, .. } => {
            visit_expr_patterns(target, visit);
            visit_expr_patterns(value, visit);
        }
        ExprKind::Field { base, .. } => visit_expr_patterns(base, visit),
        ExprKind::Call { callee, args } => {
            visit_expr_patterns(callee, visit);
            for argument in args {
                visit_argument_patterns(argument, visit);
            }
        }
        ExprKind::Index { base, index } => {
            visit_expr_patterns(base, visit);
            visit_expr_patterns(index, visit);
        }
        ExprKind::Record { fields, .. } => {
            for field in fields {
                if let Some(value) = &field.value {
                    visit_expr_patterns(value, visit);
                }
            }
        }
        ExprKind::Array(values) => {
            for value in values {
                visit_expr_patterns(value, visit);
            }
        }
        ExprKind::Map(entries) => {
            for entry in entries {
                visit_expr_patterns(&entry.key, visit);
                visit_expr_patterns(&entry.value, visit);
            }
        }
        ExprKind::Lambda { body, .. } => visit_expr_patterns(body, visit),
        ExprKind::If(if_expr) => visit_if_patterns(if_expr, visit),
        ExprKind::Block(block) => visit_block_patterns(block, visit),
        ExprKind::Literal(_) | ExprKind::Path(_) | ExprKind::SelfValue | ExprKind::Error => {}
    }
}

fn visit_argument_patterns(
    argument: &Argument,
    visit: &mut impl FnMut(&[String], &RecordPatternField),
) {
    visit_expr_patterns(&argument.value, visit);
}

fn visit_if_patterns(if_expr: &IfExpr, visit: &mut impl FnMut(&[String], &RecordPatternField)) {
    visit_expr_patterns(&if_expr.condition, visit);
    visit_block_patterns(&if_expr.then_branch, visit);
    if let Some(else_branch) = &if_expr.else_branch {
        match else_branch {
            ElseBranch::If(if_expr) => visit_if_patterns(if_expr, visit),
            ElseBranch::Block(block) => visit_block_patterns(block, visit),
        }
    }
}

fn visit_match_arm_patterns(
    arm: &MatchArm,
    visit: &mut impl FnMut(&[String], &RecordPatternField),
) {
    visit_pattern_fields(&arm.pattern, visit);
    if let Some(guard) = &arm.guard {
        visit_expr_patterns(guard, visit);
    }
    visit_expr_patterns(&arm.body, visit);
}

fn visit_pattern_fields(pattern: &Pattern, visit: &mut impl FnMut(&[String], &RecordPatternField)) {
    match pattern {
        Pattern::RecordVariant { path, fields } => {
            for field in fields {
                visit(path, field);
                if let Some(pattern) = &field.pattern {
                    visit_pattern_fields(pattern, visit);
                }
            }
        }
        Pattern::TupleVariant { fields, .. } => {
            for field in fields {
                visit_pattern_fields(field, visit);
            }
        }
        Pattern::Binding(_) | Pattern::Path(_) | Pattern::Wildcard | Pattern::Literal(_) => {}
    }
}
