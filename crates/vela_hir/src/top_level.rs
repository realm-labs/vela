use vela_common::Diagnostic;
use vela_syntax::ast::{
    Argument, ConstItem, ElseBranch, Expr, ExprKind, InterpolatedStringPart, MapEntry, RecordField,
    Stmt, StmtKind,
};

pub(crate) fn validate_const_initializer(item: &ConstItem) -> Vec<Diagnostic> {
    let mut validator = ConstInitializerValidator {
        const_name: &item.name,
        diagnostics: Vec::new(),
    };
    validator.visit_expr(&item.value);
    validator.diagnostics
}

struct ConstInitializerValidator<'a> {
    const_name: &'a str,
    diagnostics: Vec<Diagnostic>,
}

impl ConstInitializerValidator<'_> {
    fn visit_expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Assign { target, value, .. } => {
                self.report(expr, "assignment");
                self.visit_expr(target);
                self.visit_expr(value);
            }
            ExprKind::Call { callee, args } => {
                self.report(expr, "call");
                self.visit_expr(callee);
                for arg in args {
                    self.visit_argument(arg);
                }
            }
            ExprKind::Unary { expr, .. } | ExprKind::Try(expr) => {
                self.visit_expr(expr);
            }
            ExprKind::Binary { left, right, .. } => {
                self.visit_expr(left);
                self.visit_expr(right);
            }
            ExprKind::Field { base, .. } => {
                self.visit_expr(base);
            }
            ExprKind::Index { base, index } => {
                self.visit_expr(base);
                self.visit_expr(index);
            }
            ExprKind::Array(values) => {
                for value in values {
                    self.visit_expr(value);
                }
            }
            ExprKind::Map(entries) => {
                for entry in entries {
                    self.visit_map_entry(entry);
                }
            }
            ExprKind::Record { fields, .. } => {
                for field in fields {
                    self.visit_record_field(field);
                }
            }
            ExprKind::InterpolatedString(parts) => {
                for part in parts {
                    if let InterpolatedStringPart::Expr(expr) = part {
                        self.visit_expr(expr);
                    }
                }
            }
            ExprKind::Lambda { .. } => {}
            ExprKind::If(if_expr) => {
                self.visit_expr(&if_expr.condition);
                for statement in &if_expr.then_branch.statements {
                    self.visit_statement(statement);
                }
                match &if_expr.else_branch {
                    Some(ElseBranch::If(if_expr)) => {
                        self.visit_expr(&if_expr.condition);
                    }
                    Some(ElseBranch::Block(block)) => {
                        for statement in &block.statements {
                            self.visit_statement(statement);
                        }
                    }
                    None => {}
                }
            }
            ExprKind::Match(match_expr) => {
                self.visit_expr(&match_expr.scrutinee);
                for arm in &match_expr.arms {
                    if let Some(guard) = &arm.guard {
                        self.visit_expr(guard);
                    }
                    self.visit_expr(&arm.body);
                }
            }
            ExprKind::Block(block) => {
                for statement in &block.statements {
                    self.visit_statement(statement);
                }
            }
            ExprKind::Literal(_) | ExprKind::Path(_) | ExprKind::SelfValue | ExprKind::Error => {}
        }
    }

    fn visit_argument(&mut self, argument: &Argument) {
        self.visit_expr(&argument.value);
    }

    fn visit_statement(&mut self, statement: &Stmt) {
        match &statement.kind {
            StmtKind::Let { value, .. } | StmtKind::Return(value) => {
                if let Some(value) = value {
                    self.visit_expr(value);
                }
            }
            StmtKind::For { iterable, body, .. } => {
                self.report_statement(statement, "loop");
                self.visit_expr(iterable);
                for statement in &body.statements {
                    self.visit_statement(statement);
                }
            }
            StmtKind::Expr(expr) => self.visit_expr(expr),
            StmtKind::Block(block) => {
                for statement in &block.statements {
                    self.visit_statement(statement);
                }
            }
            StmtKind::Break | StmtKind::Continue => {}
        }
    }

    fn visit_map_entry(&mut self, entry: &MapEntry) {
        self.visit_expr(&entry.key);
        self.visit_expr(&entry.value);
    }

    fn visit_record_field(&mut self, field: &RecordField) {
        if let Some(value) = &field.value {
            self.visit_expr(value);
        }
    }

    fn report(&mut self, expr: &Expr, operation: &str) {
        self.diagnostics.push(
            Diagnostic::error(format!(
                "top-level const `{}` initializer cannot perform {operation}",
                self.const_name
            ))
            .with_code("hir::top_level_side_effect")
            .with_span(expr.span)
            .with_label(expr.span, "side-effecting operation in const initializer")
            .with_label(
                expr.span,
                "move this work into a runtime function instead of a top-level const",
            ),
        );
    }

    fn report_statement(&mut self, statement: &Stmt, operation: &str) {
        self.diagnostics.push(
            Diagnostic::error(format!(
                "top-level const `{}` initializer cannot perform {operation}",
                self.const_name
            ))
            .with_code("hir::top_level_side_effect")
            .with_span(statement.span)
            .with_label(
                statement.span,
                "side-effecting operation in const initializer",
            )
            .with_label(
                statement.span,
                "move this work into a runtime function instead of a top-level const",
            ),
        );
    }
}
