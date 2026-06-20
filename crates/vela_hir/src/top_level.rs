use vela_common::{Diagnostic, SourceId, Span};
use vela_syntax::TextRange;
use vela_syntax::ast::{
    Argument, AstNode, ConstItem, ElseBranch, Expr, ExprKind, InterpolatedStringPart, MapEntry,
    RecordField, Stmt, StmtKind, SyntaxBlock, SyntaxConstItem, SyntaxElseBranch, SyntaxExpression,
    SyntaxExpressionKind, SyntaxMatchArmBody, SyntaxStatement,
};

pub(crate) fn validate_const_initializer(item: &ConstItem) -> Vec<Diagnostic> {
    let mut validator = ConstInitializerValidator {
        const_name: &item.name,
        diagnostics: Vec::new(),
    };
    validator.visit_expr(&item.value);
    validator.diagnostics
}

pub(crate) fn validate_syntax_const_initializer(
    source: SourceId,
    item: &SyntaxConstItem,
) -> Vec<Diagnostic> {
    let mut validator = SyntaxConstInitializerValidator {
        source,
        const_name: item.name_text().unwrap_or_default(),
        diagnostics: Vec::new(),
    };
    if let Some(value) = item.value() {
        validator.visit_expr(&value);
    }
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
        self.diagnostics.push(side_effect_diagnostic(
            self.const_name,
            operation,
            expr.span,
        ));
    }

    fn report_statement(&mut self, statement: &Stmt, operation: &str) {
        self.diagnostics.push(side_effect_diagnostic(
            self.const_name,
            operation,
            statement.span,
        ));
    }
}

struct SyntaxConstInitializerValidator {
    source: SourceId,
    const_name: String,
    diagnostics: Vec<Diagnostic>,
}

impl SyntaxConstInitializerValidator {
    fn visit_expr(&mut self, expr: &SyntaxExpression) {
        match expr.expression_kind() {
            SyntaxExpressionKind::Assign => {
                self.report(expr.syntax().text_range(), "assignment");
                if let Some(assign) = expr.as_assign() {
                    if let Some(target) = assign.target() {
                        self.visit_expr(&target);
                    }
                    if let Some(value) = assign.value() {
                        self.visit_expr(&value);
                    }
                }
            }
            SyntaxExpressionKind::Call => {
                self.report(expr.syntax().text_range(), "call");
                if let Some(call) = expr.as_call() {
                    if let Some(callee) = call.callee() {
                        self.visit_expr(&callee);
                    }
                    for arg in call.arguments() {
                        if let Some(value) = arg.expression() {
                            self.visit_expr(&value);
                        }
                    }
                }
            }
            SyntaxExpressionKind::Unary => {
                if let Some(unary) = expr.as_unary()
                    && let Some(operand) = unary.expression()
                {
                    self.visit_expr(&operand);
                }
            }
            SyntaxExpressionKind::Try => {
                if let Some(try_expr) = expr.as_try()
                    && let Some(value) = try_expr.expression()
                {
                    self.visit_expr(&value);
                }
            }
            SyntaxExpressionKind::Binary => {
                if let Some(binary) = expr.as_binary() {
                    if let Some(lhs) = binary.lhs() {
                        self.visit_expr(&lhs);
                    }
                    if let Some(rhs) = binary.rhs() {
                        self.visit_expr(&rhs);
                    }
                }
            }
            SyntaxExpressionKind::Field => {
                if let Some(field) = expr.as_field()
                    && let Some(receiver) = field.receiver()
                {
                    self.visit_expr(&receiver);
                }
            }
            SyntaxExpressionKind::Index => {
                if let Some(index) = expr.as_index() {
                    if let Some(receiver) = index.receiver() {
                        self.visit_expr(&receiver);
                    }
                    if let Some(value) = index.index() {
                        self.visit_expr(&value);
                    }
                }
            }
            SyntaxExpressionKind::Array => {
                if let Some(array) = expr.as_array() {
                    for value in array.expressions() {
                        self.visit_expr(&value);
                    }
                }
            }
            SyntaxExpressionKind::Map => {
                if let Some(map) = expr.as_map() {
                    for entry in map.entries() {
                        if let Some(key) = entry.key() {
                            self.visit_expr(&key);
                        }
                        if let Some(value) = entry.value() {
                            self.visit_expr(&value);
                        }
                    }
                }
            }
            SyntaxExpressionKind::Record => {
                if let Some(record) = expr.as_record() {
                    for field in record.fields() {
                        if let Some(value) = field.expression() {
                            self.visit_expr(&value);
                        }
                    }
                }
            }
            SyntaxExpressionKind::Literal => {
                if let Some(literal) = expr.as_literal() {
                    for value in literal.interpolation_expressions() {
                        self.visit_expr(&value);
                    }
                }
            }
            SyntaxExpressionKind::Lambda => {}
            SyntaxExpressionKind::If => {
                if let Some(if_expr) = expr.as_if() {
                    if let Some(condition) = if_expr.condition() {
                        self.visit_expr(&condition);
                    }
                    if let Some(then_block) = if_expr.then_block() {
                        self.visit_block(&then_block);
                    }
                    match if_expr.else_branch() {
                        Some(SyntaxElseBranch::If(branch)) => {
                            if let Some(condition) = branch.condition() {
                                self.visit_expr(&condition);
                            }
                        }
                        Some(SyntaxElseBranch::Block(block)) => self.visit_block(&block),
                        None => {}
                    }
                }
            }
            SyntaxExpressionKind::Match => {
                if let Some(match_expr) = expr.as_match() {
                    if let Some(scrutinee) = match_expr.scrutinee() {
                        self.visit_expr(&scrutinee);
                    }
                    for arm in match_expr.arms() {
                        if let Some(guard) = arm.guard() {
                            self.visit_expr(&guard);
                        }
                        match arm.body() {
                            Some(SyntaxMatchArmBody::Expression(value)) => self.visit_expr(&value),
                            Some(SyntaxMatchArmBody::Block(block)) => self.visit_block(&block),
                            None => {}
                        }
                    }
                }
            }
            SyntaxExpressionKind::Block => {
                if let Some(block) = expr.as_block() {
                    self.visit_block(&block);
                }
            }
            SyntaxExpressionKind::Path | SyntaxExpressionKind::Paren => {
                if let Some(paren) = expr.as_paren()
                    && let Some(inner) = paren.expression()
                {
                    self.visit_expr(&inner);
                }
            }
        }
    }

    fn visit_block(&mut self, block: &SyntaxBlock) {
        for statement in block.statements() {
            self.visit_statement(&statement);
        }
    }

    fn visit_statement(&mut self, statement: &SyntaxStatement) {
        if let Some(let_stmt) = statement.as_let() {
            if let Some(value) = let_stmt.initializer() {
                self.visit_expr(&value);
            }
            return;
        }
        if let Some(return_stmt) = statement.as_return() {
            if let Some(value) = return_stmt.expression() {
                self.visit_expr(&value);
            }
            return;
        }
        if let Some(for_stmt) = statement.as_for() {
            self.report_statement(statement.syntax().text_range(), "loop");
            if let Some(iterable) = for_stmt.iterable() {
                self.visit_expr(&iterable);
            }
            if let Some(body) = for_stmt.body() {
                self.visit_block(&body);
            }
            return;
        }
        if let Some(expr_stmt) = statement.as_expr() {
            if let Some(value) = expr_stmt.expression() {
                self.visit_expr(&value);
            }
            return;
        }
        if let Some(if_expr) = statement.as_if() {
            self.visit_expr(&SyntaxExpression::cast(if_expr.syntax().clone()).expect("if expr"));
            return;
        }
        if let Some(match_expr) = statement.as_match() {
            self.visit_expr(
                &SyntaxExpression::cast(match_expr.syntax().clone()).expect("match expr"),
            );
            return;
        }
        if let Some(block) = statement.as_block() {
            self.visit_block(&block);
        }
    }

    fn report(&mut self, range: TextRange, operation: &str) {
        self.diagnostics.push(side_effect_diagnostic(
            &self.const_name,
            operation,
            span_for(self.source, range),
        ));
    }

    fn report_statement(&mut self, range: TextRange, operation: &str) {
        self.diagnostics.push(side_effect_diagnostic(
            &self.const_name,
            operation,
            span_for(self.source, range),
        ));
    }
}

fn side_effect_diagnostic(const_name: &str, operation: &str, span: Span) -> Diagnostic {
    Diagnostic::error(format!(
        "top-level const `{const_name}` initializer cannot perform {operation}",
    ))
    .with_code("hir::top_level_side_effect")
    .with_span(span)
    .with_label(span, "side-effecting operation in const initializer")
    .with_label(
        span,
        "move this work into a runtime function instead of a top-level const",
    )
}

fn span_for(source: SourceId, range: TextRange) -> Span {
    Span::new(source, range.start().into(), range.end().into())
}
