use std::marker::PhantomData;

use vela_common::{SourceId, Span};
#[cfg(test)]
use vela_syntax::ast::SyntaxLambdaBody;
#[cfg(test)]
use vela_syntax::ast::SyntaxMapEntry;
#[cfg(test)]
use vela_syntax::ast::SyntaxRecordExprField;
use vela_syntax::ast::{
    AstNode, SyntaxArgument, SyntaxBlock, SyntaxExpression, SyntaxExpressionKind, SyntaxForStmt,
    SyntaxIfExpr, SyntaxMatchExpr, SyntaxStatement, SyntaxStatementKind,
};
#[cfg(test)]
use vela_syntax::ast::{Expr, ExprKind, InterpolatedStringPart, Stmt, StmtKind};
#[cfg(test)]
use vela_syntax::ast::{SyntaxMatchArm, SyntaxPattern, SyntaxRecordPatternField};

mod expression_payloads;
mod simple_values;

// Temporary 1200-line exception: this module owns the CST body payload boundary.
// It is actively shrinking as the hard switch deletes the old payload pairing
// code before the module is split by syntax child payload responsibility.

#[cfg(test)]
use simple_values::syntax_statement_requires_body_block_lookup;
pub(super) use simple_values::{
    expression_syntax_literal, expression_syntax_negated_number_literal,
    expression_syntax_path_field, expression_syntax_path_or_field, expression_syntax_path_or_self,
    expression_syntax_range_operands,
};

#[derive(Clone)]
pub(super) struct SyntaxBodyPayload {
    pub(super) source: SourceId,
    pub(super) body: SyntaxBlock,
}

#[derive(Clone)]
pub(super) struct CompilerBodyPayload<'ast> {
    syntax: SyntaxBodyPayload,
    _ast: PhantomData<&'ast ()>,
}

pub(super) struct CompilerStatementPayload<'ast> {
    source: Option<SourceId>,
    syntax: Option<SyntaxStatement>,
    _ast: PhantomData<&'ast ()>,
}

#[cfg(test)]
pub(super) struct CompilerMatchArmPayload {
    source: Option<SourceId>,
    syntax: Option<SyntaxMatchArm>,
}

#[derive(Clone)]
#[cfg(test)]
pub(in crate::compiler) struct CompilerPatternPayload {
    source: Option<SourceId>,
    syntax: Option<SyntaxPattern>,
}

#[cfg(test)]
pub(in crate::compiler) struct CompilerRecordPatternFieldPayload {
    source: Option<SourceId>,
    syntax: Option<SyntaxRecordPatternField>,
}

pub(in crate::compiler) struct CompilerArgumentPayload {
    source: Option<SourceId>,
    syntax: Option<SyntaxArgument>,
}

#[cfg(test)]
pub(in crate::compiler) struct CompilerArrayElementPayload {
    source: Option<SourceId>,
    syntax: Option<SyntaxExpression>,
}

pub(in crate::compiler) struct CompilerInterpolationPayload {
    source: Option<SourceId>,
    syntax: Option<SyntaxExpression>,
}

#[derive(Clone)]
pub(in crate::compiler) struct CompilerExpressionPayload<'ast> {
    source: Option<SourceId>,
    syntax: Option<SyntaxExpression>,
    _ast: PhantomData<&'ast ()>,
}

#[cfg(test)]
pub(in crate::compiler) struct CompilerMapEntryPayload {
    source: Option<SourceId>,
    syntax: Option<SyntaxMapEntry>,
}

#[cfg(test)]
pub(in crate::compiler) struct CompilerRecordFieldPayload {
    source: Option<SourceId>,
    syntax: Option<SyntaxRecordExprField>,
}

#[cfg(test)]
pub(super) struct CompilerIfPayload<'ast> {
    source: Option<SourceId>,
    condition: Option<SyntaxExpression>,
    then_body: Option<CompilerBodyPayload<'ast>>,
    else_body: Option<CompilerBodyPayload<'ast>>,
    else_if: Option<Box<CompilerIfPayload<'ast>>>,
}

pub(super) enum CompilerBlockValue<'payload, 'ast> {
    Empty,
    TailExpression {
        prefix: &'payload [CompilerStatementPayload<'ast>],
        tail: &'payload CompilerStatementPayload<'ast>,
    },
    Statements(&'payload [CompilerStatementPayload<'ast>]),
}

impl<'ast> CompilerBodyPayload<'ast> {
    #[cfg(test)]
    pub(super) fn paired_statement_payloads_for_test(
        source: SourceId,
        body: SyntaxBlock,
        fallback_statements: &'ast [Stmt],
    ) -> Vec<CompilerStatementPayload<'ast>> {
        let syntax_statements = syntax_body_statements(&body);
        paired_statement_payloads(source, &syntax_statements, fallback_statements)
    }

    #[cfg(test)]
    pub(super) fn paired_statement_payloads_with_fallbacks_for_test(
        source: SourceId,
        body: SyntaxBlock,
        fallback_statements: &'ast [Stmt],
    ) -> Vec<CompilerStatementPayload<'ast>> {
        let syntax_statements = syntax_body_statements(&body);
        syntax_statements
            .into_iter()
            .zip(fallback_statements)
            .map(|(syntax, fallback)| {
                CompilerStatementPayload::new_paired_for_tests(source, Some(syntax), Some(fallback))
            })
            .collect()
    }

    #[cfg(test)]
    pub(super) fn raw_statement_payloads_with_fallbacks_for_test(
        source: SourceId,
        body: SyntaxBlock,
        fallback_statements: &'ast [Stmt],
    ) -> Vec<CompilerStatementPayload<'ast>> {
        body.statements()
            .zip(fallback_statements)
            .map(|(syntax, fallback)| {
                CompilerStatementPayload::new_paired_for_tests(source, Some(syntax), Some(fallback))
            })
            .collect()
    }

    fn syntax_only(source: SourceId, body: SyntaxBlock) -> Self {
        Self {
            syntax: SyntaxBodyPayload { source, body },
            _ast: PhantomData,
        }
    }

    pub(super) fn nested_syntax(source: SourceId, body: SyntaxBlock) -> Self {
        Self::syntax_only(source, body)
    }

    pub(super) fn statement_payloads(&self) -> Vec<CompilerStatementPayload<'ast>> {
        syntax_body_statements(&self.syntax.body)
            .into_iter()
            .map(|syntax| CompilerStatementPayload::new_syntax(self.syntax.source, syntax))
            .collect()
    }

    pub(super) fn syntax_statements_are_empty(&self) -> bool {
        syntax_body_statements(&self.syntax.body).is_empty()
    }

    pub(super) fn block_value<'payload>(
        &self,
        statements: &'payload [CompilerStatementPayload<'ast>],
    ) -> CompilerBlockValue<'payload, 'ast> {
        let Some((tail, prefix)) = statements.split_last() else {
            return CompilerBlockValue::Empty;
        };
        if matches!(
            tail.stored_statement_kind(),
            Some(SyntaxStatementKind::Expr | SyntaxStatementKind::If | SyntaxStatementKind::Match)
        ) {
            CompilerBlockValue::TailExpression { prefix, tail }
        } else {
            CompilerBlockValue::Statements(statements)
        }
    }

    #[cfg(test)]
    pub(super) const fn syntax_payload(&self) -> &SyntaxBodyPayload {
        &self.syntax
    }
}

#[cfg(test)]
fn paired_statement_payloads<'ast>(
    source: SourceId,
    syntax_statements: &[SyntaxStatement],
    fallback_statements: &'ast [Stmt],
) -> Vec<CompilerStatementPayload<'ast>> {
    fallback_statements
        .iter()
        .enumerate()
        .map(|(index, fallback)| {
            let syntax = syntax_statements.get(index).cloned();
            let fallback = syntax_statements
                .get(index)
                .is_none_or(|statement| {
                    let is_unterminated_tail = index == syntax_statements.len().saturating_sub(1);
                    syntax_statement_requires_body_block_lookup(statement, is_unterminated_tail)
                })
                .then_some(fallback);
            CompilerStatementPayload::new_paired_for_tests(source, syntax, fallback)
        })
        .collect()
}

fn syntax_body_statements(body: &SyntaxBlock) -> Vec<SyntaxStatement> {
    body.statements()
        .filter(syntax_statement_counts_for_body_pairing)
        .collect()
}

fn syntax_statement_counts_for_body_pairing(statement: &SyntaxStatement) -> bool {
    if !matches!(statement.statement_kind(), SyntaxStatementKind::Expr) {
        return true;
    }
    let Some(expr_stmt) = statement.as_expr() else {
        return true;
    };
    if expr_stmt.expression().is_none() {
        return false;
    }
    !syntax_statement_starts_with_infix_continuation(statement)
}

fn syntax_statement_starts_with_infix_continuation(statement: &SyntaxStatement) -> bool {
    let Some(token) = statement
        .syntax()
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| !token.kind().is_trivia())
    else {
        return false;
    };
    matches!(
        token.text(),
        "+" | "*"
            | "/"
            | "%"
            | "&&"
            | "||"
            | "=="
            | "!="
            | "==="
            | "!=="
            | "<"
            | "<="
            | ">"
            | ">="
            | ".."
            | "..="
    )
}

#[cfg(test)]
thread_local! {
    static TEST_EXPRESSION_FALLBACKS:
        std::cell::RefCell<
            std::collections::HashMap<(u32, u32, u32), &'static Expr>
        > = std::cell::RefCell::new(std::collections::HashMap::new());
}

#[cfg(test)]
fn syntax_expression_key(source: SourceId, expression: &SyntaxExpression) -> (u32, u32, u32) {
    let range = expression.syntax().text_range();
    (source.get(), range.start().into(), range.end().into())
}

#[cfg(test)]
fn store_expression_fallback(source: SourceId, syntax: &SyntaxExpression, fallback: &Expr) {
    let leaked = Box::leak(Box::new(fallback.clone()));
    TEST_EXPRESSION_FALLBACKS.with(|fallbacks| {
        fallbacks
            .borrow_mut()
            .insert(syntax_expression_key(source, syntax), leaked);
    });
}

#[cfg(test)]
fn fallback_expression_for_syntax(
    source: SourceId,
    syntax: &SyntaxExpression,
) -> Option<&'static Expr> {
    TEST_EXPRESSION_FALLBACKS.with(|fallbacks| {
        fallbacks
            .borrow()
            .get(&syntax_expression_key(source, syntax))
            .copied()
    })
}

#[cfg(test)]
fn register_statement_fallbacks(source: SourceId, syntax: &SyntaxStatement, fallback: &Stmt) {
    match &fallback.kind {
        StmtKind::Let {
            value: Some(value), ..
        } => {
            if let Some(initializer) = syntax
                .as_let()
                .and_then(|statement| statement.initializer())
            {
                register_expression_fallback(source, &initializer, value);
            }
        }
        StmtKind::Return(Some(value)) => {
            if let Some(expression) = syntax
                .as_return()
                .and_then(|statement| statement.expression())
            {
                register_expression_fallback(source, &expression, value);
            }
        }
        StmtKind::For { iterable, body, .. } => {
            if let Some(iterable_syntax) =
                syntax.as_for().and_then(|statement| statement.iterable())
            {
                register_expression_fallback(source, &iterable_syntax, iterable);
            }
            if let Some(body_syntax) = syntax.as_for().and_then(|statement| statement.body()) {
                register_block_fallbacks(source, &body_syntax, body);
            }
        }
        StmtKind::Expr(value) => {
            if let Some(expression) = syntax
                .as_expr()
                .and_then(|statement| statement.expression())
                .or_else(|| SyntaxExpression::cast(syntax.syntax().clone()))
            {
                register_expression_fallback(source, &expression, value);
            }
        }
        StmtKind::Block(body) => {
            if let Some(body_syntax) = syntax.as_block() {
                register_block_fallbacks(source, &body_syntax, body);
            }
        }
        StmtKind::Let { value: None, .. }
        | StmtKind::Return(None)
        | StmtKind::Break
        | StmtKind::Continue => {}
    }
}

#[cfg(test)]
fn register_block_fallbacks(
    source: SourceId,
    syntax: &SyntaxBlock,
    fallback: &vela_syntax::ast::Block,
) {
    for (syntax_statement, fallback_statement) in syntax_body_statements(syntax)
        .iter()
        .zip(&fallback.statements)
    {
        register_statement_fallbacks(source, syntax_statement, fallback_statement);
    }
}

#[cfg(test)]
fn register_expression_fallback(source: SourceId, syntax: &SyntaxExpression, fallback: &Expr) {
    if let Some(inner) = syntax.as_paren().and_then(|paren| paren.expression()) {
        register_expression_fallback(source, &inner, fallback);
    }

    store_expression_fallback(source, syntax, fallback);

    match &fallback.kind {
        ExprKind::Unary { expr, .. } => {
            if let Some(operand) = syntax.as_unary().and_then(|unary| unary.expression()) {
                register_expression_fallback(source, &operand, expr);
            }
        }
        ExprKind::Binary { left, right, .. } => {
            if let Some((left_syntax, right_syntax)) = syntax.as_binary().and_then(|binary| {
                let mut expressions = binary.expressions();
                Some((expressions.next()?, expressions.next()?))
            }) {
                register_expression_fallback(source, &left_syntax, left);
                register_expression_fallback(source, &right_syntax, right);
            }
        }
        ExprKind::Assign { target, value, .. } => {
            if let Some(assign) = syntax.as_assign() {
                if let Some(target_syntax) = assign.target() {
                    register_expression_fallback(source, &target_syntax, target);
                }
                if let Some(value_syntax) = assign.value() {
                    register_expression_fallback(source, &value_syntax, value);
                }
            }
        }
        ExprKind::Field { base, .. } => {
            if let Some(receiver) = syntax.as_field().and_then(|field| field.receiver()) {
                register_expression_fallback(source, &receiver, base);
            }
        }
        ExprKind::Call { callee, args } => {
            if let Some(call) = syntax.as_call() {
                if let Some(callee_syntax) = call.callee() {
                    register_expression_fallback(source, &callee_syntax, callee);
                }
                for (argument_syntax, argument) in call.arguments().iter().zip(args) {
                    if let Some(value_syntax) = argument_syntax.expression() {
                        register_expression_fallback(source, &value_syntax, &argument.value);
                    }
                }
            }
        }
        ExprKind::Index { base, index } => {
            if let Some(index_syntax) = syntax.as_index() {
                if let Some(receiver) = index_syntax.receiver() {
                    register_expression_fallback(source, &receiver, base);
                }
                if let Some(index_value) = index_syntax.index() {
                    register_expression_fallback(source, &index_value, index);
                }
            }
        }
        ExprKind::Try(value) => {
            if let Some(operand) = syntax.as_try().and_then(|try_expr| try_expr.expression()) {
                register_expression_fallback(source, &operand, value);
            }
        }
        ExprKind::Array(values) => {
            if let Some(array) = syntax.as_array() {
                for (value_syntax, value) in array.expressions().zip(values) {
                    register_expression_fallback(source, &value_syntax, value);
                }
            }
        }
        ExprKind::Map(entries) => {
            if let Some(map) = syntax.as_map() {
                for (entry_syntax, entry) in map.entries().zip(entries) {
                    if let Some(key_syntax) = entry_syntax.key() {
                        register_expression_fallback(source, &key_syntax, &entry.key);
                    }
                    if let Some(value_syntax) = entry_syntax.value() {
                        register_expression_fallback(source, &value_syntax, &entry.value);
                    }
                }
            }
        }
        ExprKind::Record { fields, .. } => {
            if let Some(record) = syntax.as_record() {
                for (field_syntax, field) in record.fields().into_iter().zip(fields) {
                    if let (Some(value_syntax), Some(value)) =
                        (field_syntax.expression(), field.value.as_ref())
                    {
                        register_expression_fallback(source, &value_syntax, value);
                    }
                }
            }
        }
        ExprKind::Lambda { body, .. } => {
            if let Some(lambda) = syntax.as_lambda() {
                match lambda.body() {
                    Some(SyntaxLambdaBody::Expression(value_syntax)) => {
                        register_expression_fallback(source, &value_syntax, body);
                    }
                    Some(SyntaxLambdaBody::Block(block_syntax)) => {
                        if let ExprKind::Block(block) = &body.kind {
                            register_block_fallbacks(source, &block_syntax, block);
                        }
                    }
                    None => {}
                }
            }
        }
        ExprKind::If(if_expr) => {
            if let Some(if_syntax) = syntax.as_if() {
                if let Some(condition) = if_syntax.condition() {
                    register_expression_fallback(source, &condition, &if_expr.condition);
                }
                if let Some(then_block) = if_syntax.then_block() {
                    register_block_fallbacks(source, &then_block, &if_expr.then_branch);
                }
                match (if_syntax.else_branch(), if_expr.else_branch.as_ref()) {
                    (
                        Some(vela_syntax::ast::SyntaxElseBranch::If(else_if_syntax)),
                        Some(vela_syntax::ast::ElseBranch::If(else_if)),
                    ) => {
                        let else_if_expr = Expr {
                            kind: ExprKind::If(else_if.clone()),
                            span: else_if.condition.span,
                        };
                        if let Some(else_if_expression) =
                            SyntaxExpression::cast(else_if_syntax.syntax().clone())
                        {
                            register_expression_fallback(
                                source,
                                &else_if_expression,
                                &else_if_expr,
                            );
                        }
                    }
                    (
                        Some(vela_syntax::ast::SyntaxElseBranch::Block(block_syntax)),
                        Some(vela_syntax::ast::ElseBranch::Block(block)),
                    ) => register_block_fallbacks(source, &block_syntax, block),
                    _ => {}
                }
            }
        }
        ExprKind::Match(match_expr) => {
            if let Some(match_syntax) = syntax.as_match() {
                if let Some(scrutinee) = match_syntax.scrutinee() {
                    register_expression_fallback(source, &scrutinee, &match_expr.scrutinee);
                }
                for (arm_syntax, arm) in match_syntax.arms().into_iter().zip(&match_expr.arms) {
                    if let Some(guard) = arm_syntax.guard()
                        && let Some(guard_fallback) = arm.guard.as_ref()
                    {
                        register_expression_fallback(source, &guard, guard_fallback);
                    }
                    if let Some(body) = arm_syntax.body_as_expression() {
                        register_expression_fallback(source, &body, &arm.body);
                    }
                }
            }
        }
        ExprKind::Block(block) => {
            if let Some(block_syntax) = syntax.as_block() {
                register_block_fallbacks(source, &block_syntax, block);
            }
        }
        ExprKind::InterpolatedString(parts) => {
            if let Some(literal) = syntax.as_literal() {
                let expressions = parts.iter().filter_map(|part| match part {
                    InterpolatedStringPart::Expr(expr) => Some(expr),
                    InterpolatedStringPart::Text(_) => None,
                });
                for (expression_syntax, expression) in
                    literal.interpolation_expressions().zip(expressions)
                {
                    register_expression_fallback(source, &expression_syntax, expression);
                }
            }
        }
        ExprKind::Literal(_) | ExprKind::Path(_) | ExprKind::SelfValue | ExprKind::Error => {}
    }
}

#[cfg(test)]
fn expression_block_syntax(expression: &SyntaxExpression) -> Option<SyntaxBlock> {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return expression_block_syntax(&inner);
    }
    expression.as_block()
}

#[cfg(test)]
fn match_arm_payloads_for_syntax(
    source: Option<SourceId>,
    syntax: SyntaxMatchExpr,
) -> Option<Vec<CompilerMatchArmPayload>> {
    let source = source?;
    Some(
        syntax
            .arms()
            .into_iter()
            .map(|syntax| CompilerMatchArmPayload {
                source: Some(source),
                syntax: Some(syntax),
            })
            .collect(),
    )
}

#[cfg(test)]
fn if_payload_for_syntax<'ast>(
    source: Option<SourceId>,
    syntax: SyntaxIfExpr,
) -> Option<CompilerIfPayload<'ast>> {
    let source = source?;
    let condition = syntax.condition();
    let then_body = syntax
        .then_block()
        .map(|body| CompilerBodyPayload::nested_syntax(source, body));
    let else_body = syntax
        .else_block()
        .map(|body| CompilerBodyPayload::nested_syntax(source, body));
    let else_if = if let Some(syntax_if) = syntax.else_if() {
        if_payload_for_syntax(Some(source), syntax_if).map(Box::new)
    } else {
        None
    };
    Some(CompilerIfPayload {
        source: Some(source),
        condition,
        then_body,
        else_body,
        else_if,
    })
}

#[cfg(test)]
impl<'ast> CompilerIfPayload<'ast> {
    #[cfg(test)]
    pub(in crate::compiler) fn truncated_for_test() -> Self {
        Self {
            source: None,
            condition: None,
            then_body: None,
            else_body: None,
            else_if: None,
        }
    }

    #[cfg(test)]
    pub(in crate::compiler) fn without_then_body_for_test(mut self) -> Self {
        self.then_body = None;
        self
    }

    #[cfg(test)]
    pub(in crate::compiler) fn without_else_body_for_test(mut self) -> Self {
        self.else_body = None;
        self
    }

    #[cfg(test)]
    pub(in crate::compiler) fn without_else_if_for_test(mut self) -> Self {
        self.else_if = None;
        self
    }
}

impl<'ast> CompilerStatementPayload<'ast> {
    fn new_syntax(source: SourceId, syntax: SyntaxStatement) -> Self {
        Self {
            source: Some(source),
            syntax: Some(syntax),
            _ast: PhantomData,
        }
    }

    #[cfg(test)]
    pub(super) fn missing_child_payload_context(syntax: SyntaxStatement) -> Self {
        Self {
            source: None,
            syntax: Some(syntax),
            _ast: PhantomData,
        }
    }

    #[cfg(test)]
    pub(super) fn missing_let_child_payload_context(syntax: SyntaxStatement) -> Self {
        Self {
            source: None,
            syntax: Some(syntax),
            _ast: PhantomData,
        }
    }

    #[cfg(test)]
    pub(super) fn missing_return_child_payload_context(syntax: SyntaxStatement) -> Self {
        Self {
            source: None,
            syntax: Some(syntax),
            _ast: PhantomData,
        }
    }

    #[cfg(test)]
    pub(super) fn missing_for_child_payload_context(syntax: SyntaxStatement) -> Self {
        Self {
            source: None,
            syntax: Some(syntax),
            _ast: PhantomData,
        }
    }

    #[cfg(test)]
    fn new_paired_for_tests(
        source: SourceId,
        syntax: Option<SyntaxStatement>,
        fallback: Option<&'ast Stmt>,
    ) -> Self {
        if let (Some(syntax), Some(fallback)) = (syntax.as_ref(), fallback) {
            register_statement_fallbacks(source, syntax, fallback);
        }
        Self {
            source: Some(source),
            syntax,
            _ast: PhantomData,
        }
    }

    #[cfg(test)]
    pub(in crate::compiler) fn fallback_span(&self) -> Option<Span> {
        self.syntax_statement_span()
    }

    #[cfg(test)]
    pub(in crate::compiler) fn for_iterable_expression_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        Some(CompilerExpressionPayload::from_syntax(
            Some(self.syntax_statement_span()?.source),
            self.syntax_statement()?.as_for()?.iterable(),
        ))
    }

    #[cfg(test)]
    pub(in crate::compiler) fn for_index_pattern_payload(&self) -> Option<CompilerPatternPayload> {
        Some(CompilerPatternPayload::from_syntax(
            Some(self.syntax_statement_span()?.source),
            self.syntax_statement()?.as_for()?.index_pattern(),
        ))
    }

    #[cfg(test)]
    pub(in crate::compiler) fn for_value_pattern_payload(&self) -> Option<CompilerPatternPayload> {
        Some(CompilerPatternPayload::from_syntax(
            Some(self.syntax_statement_span()?.source),
            self.syntax_statement()?.as_for()?.value_pattern(),
        ))
    }

    pub(super) fn is_syntax_only(&self) -> bool {
        true
    }

    pub(super) fn stored_statement_kind(&self) -> Option<SyntaxStatementKind> {
        self.syntax.as_ref().map(SyntaxStatement::statement_kind)
    }

    pub(super) fn stored_expression_kind(&self) -> Option<SyntaxExpressionKind> {
        self.expression()
            .map(|expression| expression.expression_kind())
    }

    pub(in crate::compiler) fn expression_statement_syntax_expression(
        &self,
    ) -> Option<(SourceId, SyntaxExpression)> {
        Some((self.source?, self.expression()?))
    }

    #[cfg(test)]
    pub(super) fn stored_value_expression_kind(&self) -> Option<SyntaxExpressionKind> {
        self.stored_expression_kind()
            .or_else(|| match self.stored_statement_kind()? {
                SyntaxStatementKind::Block => Some(SyntaxExpressionKind::Block),
                SyntaxStatementKind::If => Some(SyntaxExpressionKind::If),
                SyntaxStatementKind::Match => Some(SyntaxExpressionKind::Match),
                _ => None,
            })
    }

    pub(super) fn stored_let_initializer_kind(&self) -> Option<SyntaxExpressionKind> {
        self.syntax
            .as_ref()?
            .as_let()?
            .initializer()
            .map(|expression| expression.expression_kind())
    }

    pub(super) fn let_initializer_missing_in_syntax(&self) -> bool {
        self.source.is_some()
            && self
                .syntax
                .as_ref()
                .and_then(SyntaxStatement::as_let)
                .is_some_and(|statement| statement.initializer().is_none())
    }

    pub(super) fn let_name_text(&self) -> Option<String> {
        self.source?;
        self.syntax.as_ref()?.as_let()?.name_text()
    }

    pub(in crate::compiler) fn let_initializer_syntax_literal_and_span(
        &self,
    ) -> Option<(vela_syntax::ast::Literal, Span)> {
        let source = self.source?;
        let expression = self.syntax.as_ref()?.as_let()?.initializer()?;
        let range = expression.syntax().text_range();
        let span = Span::new(source, range.start().into(), range.end().into());
        let literal = expression_syntax_literal(&expression)?;
        Some((literal, span))
    }

    pub(in crate::compiler) fn let_initializer_syntax_negated_literal_and_span(
        &self,
    ) -> Option<(vela_syntax::ast::Literal, Span)> {
        let source = self.source?;
        let expression = self.syntax.as_ref()?.as_let()?.initializer()?;
        let range = expression.syntax().text_range();
        let span = Span::new(source, range.start().into(), range.end().into());
        let literal = expression_syntax_negated_number_literal(&expression)?;
        Some((literal, span))
    }

    pub(in crate::compiler) fn let_initializer_syntax_path_and_span(
        &self,
    ) -> Option<(Vec<String>, Span)> {
        let source = self.source?;
        let expression = self.syntax.as_ref()?.as_let()?.initializer()?;
        let range = expression.syntax().text_range();
        let span = Span::new(source, range.start().into(), range.end().into());
        let path = expression_syntax_path_or_self(&expression)?;
        (!path.is_empty()).then_some((path, span))
    }

    pub(in crate::compiler) fn let_initializer_syntax_expression_and_span(
        &self,
    ) -> Option<(SourceId, SyntaxExpression, Span)> {
        let source = self.source?;
        let expression = self.syntax.as_ref()?.as_let()?.initializer()?;
        let range = expression.syntax().text_range();
        let span = Span::new(source, range.start().into(), range.end().into());
        Some((source, expression, span))
    }

    #[cfg(test)]
    pub(super) fn let_initializer_block_body_payload(&self) -> Option<CompilerBodyPayload<'ast>> {
        let body = self.syntax.as_ref()?.as_let()?.initializer()?.as_block()?;
        Some(CompilerBodyPayload::nested_syntax(self.source?, body))
    }

    #[cfg(test)]
    pub(in crate::compiler) fn let_initializer_expression_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        let source = self.source?;
        let expression = self.syntax.as_ref()?.as_let()?.initializer()?;
        Some(CompilerExpressionPayload::from_syntax(
            Some(source),
            Some(expression),
        ))
    }

    #[cfg(test)]
    pub(in crate::compiler) fn let_initializer_fallback_for_test(&self) -> Option<&'ast Expr> {
        let payload = self.let_initializer_expression_payload()?;
        Some(payload.fallback())
    }

    pub(super) fn stored_return_value_kind(&self) -> Option<SyntaxExpressionKind> {
        self.syntax
            .as_ref()?
            .as_return()?
            .expression()
            .map(|expression| expression.expression_kind())
    }

    pub(in crate::compiler) fn syntax_if(&self) -> Option<(SourceId, SyntaxIfExpr)> {
        Some((self.source?, self.syntax.as_ref()?.as_if()?))
    }

    pub(in crate::compiler) fn syntax_for(&self) -> Option<(SourceId, SyntaxForStmt)> {
        Some((self.source?, self.syntax.as_ref()?.as_for()?))
    }

    pub(in crate::compiler) fn syntax_match(&self) -> Option<(SourceId, SyntaxMatchExpr)> {
        Some((self.source?, self.expression()?.as_match()?))
    }

    pub(in crate::compiler) fn return_value_syntax_literal_and_span(
        &self,
    ) -> Option<(vela_syntax::ast::Literal, Span)> {
        let source = self.source?;
        let expression = self.syntax.as_ref()?.as_return()?.expression()?;
        let range = expression.syntax().text_range();
        let span = Span::new(source, range.start().into(), range.end().into());
        let literal = expression_syntax_literal(&expression)?;
        Some((literal, span))
    }

    pub(in crate::compiler) fn return_value_syntax_negated_literal_and_span(
        &self,
    ) -> Option<(vela_syntax::ast::Literal, Span)> {
        let source = self.source?;
        let expression = self.syntax.as_ref()?.as_return()?.expression()?;
        let range = expression.syntax().text_range();
        let span = Span::new(source, range.start().into(), range.end().into());
        let literal = expression_syntax_negated_number_literal(&expression)?;
        Some((literal, span))
    }

    pub(in crate::compiler) fn return_value_syntax_path_and_span(
        &self,
    ) -> Option<(Vec<String>, Span)> {
        let source = self.source?;
        let expression = self.syntax.as_ref()?.as_return()?.expression()?;
        let range = expression.syntax().text_range();
        let span = Span::new(source, range.start().into(), range.end().into());
        let path = expression_syntax_path_or_self(&expression)?;
        (!path.is_empty()).then_some((path, span))
    }

    pub(in crate::compiler) fn return_value_syntax_expression_and_span(
        &self,
    ) -> Option<(SourceId, SyntaxExpression, Span)> {
        let source = self.source?;
        let expression = self.syntax.as_ref()?.as_return()?.expression()?;
        let range = expression.syntax().text_range();
        let span = Span::new(source, range.start().into(), range.end().into());
        Some((source, expression, span))
    }

    pub(super) fn syntax_statement_span(&self) -> Option<Span> {
        let source = self.source?;
        let range = self.syntax.as_ref()?.syntax().text_range();
        Some(Span::new(source, range.start().into(), range.end().into()))
    }

    pub(super) fn return_value_missing_in_syntax(&self) -> bool {
        self.source.is_some()
            && self
                .syntax
                .as_ref()
                .and_then(SyntaxStatement::as_return)
                .is_some_and(|statement| statement.expression().is_none())
    }

    #[cfg(test)]
    pub(super) fn return_value_block_body_payload(&self) -> Option<CompilerBodyPayload<'ast>> {
        let body = self
            .syntax
            .as_ref()?
            .as_return()?
            .expression()?
            .as_block()?;
        Some(CompilerBodyPayload::nested_syntax(self.source?, body))
    }

    #[cfg(test)]
    pub(in crate::compiler) fn return_value_expression_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        let source = self.source?;
        let expression = self.syntax.as_ref()?.as_return()?.expression()?;
        Some(CompilerExpressionPayload::from_syntax(
            Some(source),
            Some(expression),
        ))
    }

    #[cfg(test)]
    pub(in crate::compiler) fn return_value_fallback_for_test(&self) -> Option<&'ast Expr> {
        let payload = self.return_value_expression_payload()?;
        Some(payload.fallback())
    }

    pub(super) fn block_body_payload(&self) -> Option<CompilerBodyPayload<'ast>> {
        let body = self.syntax.as_ref()?.as_block()?;
        Some(CompilerBodyPayload::nested_syntax(self.source?, body))
    }

    fn expression(&self) -> Option<SyntaxExpression> {
        let syntax = self.syntax.as_ref()?;
        syntax
            .as_expr()
            .and_then(|stmt| stmt.expression())
            .or_else(|| SyntaxExpression::cast(syntax.syntax().clone()))
    }

    #[cfg(test)]
    pub(in crate::compiler) fn expression_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        let source = self.source?;
        let expression = self.expression()?;
        Some(CompilerExpressionPayload::from_syntax(
            Some(source),
            Some(expression),
        ))
    }

    #[cfg(test)]
    pub(in crate::compiler) fn expression_fallback_for_test(&self) -> Option<&'ast Expr> {
        let payload = self.expression_payload()?;
        Some(payload.fallback())
    }

    #[cfg(test)]
    pub(super) fn expression_statement_block_body_payload(
        &self,
    ) -> Option<CompilerBodyPayload<'ast>> {
        let body = expression_block_syntax(&self.expression()?)?;
        Some(CompilerBodyPayload::nested_syntax(self.source?, body))
    }

    #[cfg(test)]
    pub(super) fn syntax_statement(&self) -> Option<&SyntaxStatement> {
        self.source?;
        self.syntax.as_ref()
    }
}

impl CompilerArgumentPayload {
    pub(in crate::compiler) fn source(&self) -> Option<SourceId> {
        self.source
    }

    pub(in crate::compiler) fn has_value_syntax(&self) -> bool {
        self.source.is_some()
            && self
                .syntax
                .as_ref()
                .is_some_and(|syntax| syntax.expression().is_some())
    }

    pub(in crate::compiler) fn syntax_name(&self) -> Option<String> {
        self.source?;
        self.syntax.as_ref().and_then(SyntaxArgument::name_text)
    }

    pub(in crate::compiler) fn value_expression_payload<'ast>(
        &self,
    ) -> CompilerExpressionPayload<'ast> {
        CompilerExpressionPayload::from_syntax(
            self.source,
            self.source
                .and_then(|_| self.syntax.as_ref().and_then(SyntaxArgument::expression)),
        )
    }

    pub(in crate::compiler) fn syntax_argument(&self) -> Option<&SyntaxArgument> {
        self.source?;
        self.syntax.as_ref()
    }
}

impl<'ast> CompilerExpressionPayload<'ast> {
    #[cfg(test)]
    pub(in crate::compiler) fn from_fallback(
        source: Option<SourceId>,
        syntax: Option<SyntaxExpression>,
        fallback: &'ast Expr,
    ) -> Self {
        if let (Some(source), Some(syntax)) = (source, syntax.as_ref()) {
            register_expression_fallback(source, syntax, fallback);
        }
        Self::from_syntax(source, syntax)
    }

    pub(in crate::compiler) fn from_syntax(
        source: Option<SourceId>,
        syntax: Option<SyntaxExpression>,
    ) -> Self {
        Self {
            source,
            syntax,
            _ast: PhantomData,
        }
    }

    #[cfg(test)]
    pub(in crate::compiler) fn missing_syntax(source: SourceId) -> Self {
        Self::from_syntax(Some(source), None)
    }

    fn matches_syntax_kind(&self, syntax_kind: SyntaxExpressionKind) -> bool {
        syntax_kind == SyntaxExpressionKind::Paren || self.stored_syntax_kind() == Some(syntax_kind)
    }

    #[cfg(test)]
    pub(in crate::compiler) fn fallback(&self) -> &'ast Expr {
        let source = self.source.expect("expression payload has no source");
        let syntax = self
            .syntax
            .as_ref()
            .expect("expression payload has no CST expression");
        fallback_expression_for_syntax(source, syntax)
            .expect("expression payload has no owned expression fallback")
    }

    pub(in crate::compiler) fn source(&self) -> Option<SourceId> {
        self.source
    }

    pub(in crate::compiler) fn syntax_kind(&self) -> Option<SyntaxExpressionKind> {
        self.source?;
        self.stored_syntax_kind()
    }

    pub(super) fn stored_syntax_kind(&self) -> Option<SyntaxExpressionKind> {
        self.syntax
            .as_ref()
            .map(|expression| expression.expression_kind())
    }

    pub(in crate::compiler) fn requires_matching_payload(&self) -> bool {
        [
            SyntaxExpressionKind::Block,
            SyntaxExpressionKind::If,
            SyntaxExpressionKind::Match,
            SyntaxExpressionKind::Array,
            SyntaxExpressionKind::Map,
            SyntaxExpressionKind::Record,
            SyntaxExpressionKind::Path,
        ]
        .into_iter()
        .any(|kind| self.matches_syntax_kind(kind))
    }

    pub(in crate::compiler) fn rejects_missing_payload(&self) -> bool {
        if self.source.is_some() && self.syntax.is_none() {
            return true;
        }
        self.requires_matching_payload()
            || [
                SyntaxExpressionKind::Assign,
                SyntaxExpressionKind::Binary,
                SyntaxExpressionKind::Call,
                SyntaxExpressionKind::Field,
                SyntaxExpressionKind::Index,
                SyntaxExpressionKind::Literal,
                SyntaxExpressionKind::Lambda,
                SyntaxExpressionKind::Try,
                SyntaxExpressionKind::Unary,
            ]
            .into_iter()
            .any(|kind| self.matches_syntax_kind(kind))
    }

    pub(in crate::compiler) fn syntax_expression(&self) -> Option<&SyntaxExpression> {
        self.source?;
        self.syntax.as_ref()
    }
}

#[cfg(test)]
impl<'ast> CompilerIfPayload<'ast> {
    pub(super) fn condition_payload(&self) -> Option<CompilerExpressionPayload<'ast>> {
        let condition = self.condition.clone()?;
        Some(CompilerExpressionPayload::from_syntax(
            self.source,
            Some(condition),
        ))
    }

    pub(super) fn then_body(&self) -> Option<&CompilerBodyPayload<'ast>> {
        self.then_body.as_ref()
    }

    pub(super) fn else_body(&self) -> Option<&CompilerBodyPayload<'ast>> {
        self.else_body.as_ref()
    }

    pub(super) fn else_if(&self) -> Option<&CompilerIfPayload<'ast>> {
        self.else_if.as_deref()
    }

    #[cfg(test)]
    pub(super) fn condition_expression(&self) -> Option<&SyntaxExpression> {
        self.condition.as_ref()
    }
}
