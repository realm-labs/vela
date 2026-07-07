use std::marker::PhantomData;

use vela_common::{SourceId, Span};
#[cfg(test)]
use vela_syntax::ast::SyntaxRecordExprField;
use vela_syntax::ast::{
    AstNode, SyntaxArgument, SyntaxBlock, SyntaxExpression, SyntaxExpressionKind, SyntaxForStmt,
    SyntaxIfExpr, SyntaxMapEntry, SyntaxMatchExpr, SyntaxStatement, SyntaxStatementKind,
};
#[cfg(test)]
use vela_syntax::ast::{Expr, Pattern, Stmt, StmtKind};
#[cfg(test)]
use vela_syntax::ast::{SyntaxMatchArm, SyntaxPattern, SyntaxRecordPatternField};

mod expression_payloads;
mod simple_values;

// Temporary 1200-line exception: this module owns the transitional CST plus
// old-body-fallback pairing invariant. Splitting the remaining fallback side
// before the hard switch would obscure that invariant and create churn in code
// that is scheduled for deletion when body payloads become CST-only.

pub(super) use simple_values::{
    expression_syntax_literal, expression_syntax_negated_number_literal,
    expression_syntax_path_field, expression_syntax_path_or_field, expression_syntax_path_or_self,
    expression_syntax_range_operands,
};

#[cfg(test)]
use simple_values::syntax_statement_requires_body_block_lookup;

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
    #[cfg(test)]
    expression_fallbacks: StatementExpressionFallbacks<'ast>,
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
    #[cfg(test)]
    fallback: Option<&'ast vela_syntax::ast::Expr>,
}

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
#[derive(Clone, Copy, Default)]
struct StatementExpressionFallbacks<'ast> {
    statement_span: Option<Span>,
    for_iterable: Option<&'ast Expr>,
    for_index_pattern: Option<&'ast Pattern>,
    for_value_pattern: Option<&'ast Pattern>,
    let_initializer: Option<&'ast Expr>,
    return_value: Option<&'ast Expr>,
    expression: Option<&'ast Expr>,
}

#[cfg(test)]
impl<'ast> StatementExpressionFallbacks<'ast> {
    fn from_statement(fallback: Option<&'ast Stmt>) -> Self {
        let Some(statement) = fallback else {
            return Self::default();
        };
        match &statement.kind {
            StmtKind::For {
                index_pattern,
                pattern,
                iterable,
                ..
            } => Self {
                statement_span: Some(statement.span),
                for_iterable: Some(iterable),
                for_index_pattern: index_pattern.as_ref(),
                for_value_pattern: Some(pattern),
                ..Self::default()
            },
            StmtKind::Let {
                value: Some(value), ..
            } => Self {
                statement_span: Some(statement.span),
                let_initializer: Some(value),
                ..Self::default()
            },
            StmtKind::Return(Some(value)) => Self {
                statement_span: Some(statement.span),
                return_value: Some(value),
                ..Self::default()
            },
            StmtKind::Expr(expr) => Self {
                statement_span: Some(statement.span),
                expression: Some(expr),
                ..Self::default()
            },
            _ => Self {
                statement_span: Some(statement.span),
                ..Self::default()
            },
        }
    }
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
            #[cfg(test)]
            expression_fallbacks: StatementExpressionFallbacks::default(),
        }
    }

    #[cfg(test)]
    fn new_paired_for_tests(
        source: SourceId,
        syntax: Option<SyntaxStatement>,
        fallback: Option<&'ast Stmt>,
    ) -> Self {
        Self {
            source: Some(source),
            syntax,
            _ast: PhantomData,
            expression_fallbacks: StatementExpressionFallbacks::from_statement(fallback),
        }
    }

    #[cfg(test)]
    pub(super) fn missing_child_payload_context(syntax: SyntaxStatement) -> Self {
        Self {
            source: None,
            syntax: Some(syntax),
            _ast: PhantomData,
            expression_fallbacks: StatementExpressionFallbacks::default(),
        }
    }

    #[cfg(test)]
    pub(super) fn missing_let_child_payload_context(syntax: SyntaxStatement) -> Self {
        Self {
            source: None,
            syntax: Some(syntax),
            _ast: PhantomData,
            expression_fallbacks: StatementExpressionFallbacks::default(),
        }
    }

    #[cfg(test)]
    pub(super) fn missing_return_child_payload_context(syntax: SyntaxStatement) -> Self {
        Self {
            source: None,
            syntax: Some(syntax),
            _ast: PhantomData,
            expression_fallbacks: StatementExpressionFallbacks::default(),
        }
    }

    #[cfg(test)]
    pub(super) fn missing_for_child_payload_context(syntax: SyntaxStatement) -> Self {
        Self {
            source: None,
            syntax: Some(syntax),
            _ast: PhantomData,
            expression_fallbacks: StatementExpressionFallbacks::default(),
        }
    }

    #[cfg(test)]
    pub(in crate::compiler) fn fallback_span(&self) -> Option<Span> {
        self.expression_fallbacks.statement_span
    }

    #[cfg(test)]
    pub(in crate::compiler) fn for_iterable_expression_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        if self.is_syntax_only() {
            return None;
        }
        let iterable = self.expression_fallbacks.for_iterable?;
        Some(CompilerExpressionPayload::from_fallback(
            Some(self.syntax_statement_span()?.source),
            self.syntax_statement()?.as_for()?.iterable(),
            iterable,
        ))
    }

    #[cfg(test)]
    pub(in crate::compiler) fn for_index_pattern_payload(&self) -> Option<CompilerPatternPayload> {
        if self.is_syntax_only() {
            return None;
        }
        self.expression_fallbacks.for_index_pattern?;
        Some(CompilerPatternPayload::from_syntax(
            Some(self.syntax_statement_span()?.source),
            self.syntax_statement()?.as_for()?.index_pattern(),
        ))
    }

    #[cfg(test)]
    pub(in crate::compiler) fn for_value_pattern_payload(&self) -> Option<CompilerPatternPayload> {
        if self.is_syntax_only() {
            return None;
        }
        self.expression_fallbacks.for_value_pattern?;
        Some(CompilerPatternPayload::from_syntax(
            Some(self.syntax_statement_span()?.source),
            self.syntax_statement()?.as_for()?.value_pattern(),
        ))
    }

    pub(super) fn is_syntax_only(&self) -> bool {
        #[cfg(test)]
        {
            self.expression_fallbacks.statement_span.is_none()
        }
        #[cfg(not(test))]
        {
            true
        }
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
        let value = self.expression_fallbacks.let_initializer?;
        Some(CompilerExpressionPayload::from_fallback(
            self.source,
            self.syntax.as_ref()?.as_let()?.initializer(),
            value,
        ))
    }

    #[cfg(test)]
    pub(in crate::compiler) fn let_initializer_fallback_for_test(&self) -> Option<&'ast Expr> {
        self.expression_fallbacks.let_initializer
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
        let value = self.expression_fallbacks.return_value?;
        Some(CompilerExpressionPayload::from_fallback(
            self.source,
            self.syntax.as_ref()?.as_return()?.expression(),
            value,
        ))
    }

    #[cfg(test)]
    pub(in crate::compiler) fn return_value_fallback_for_test(&self) -> Option<&'ast Expr> {
        self.expression_fallbacks.return_value
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
        let expr = self.expression_fallbacks.expression?;
        Some(CompilerExpressionPayload::from_fallback(
            self.source,
            self.expression(),
            expr,
        ))
    }

    #[cfg(test)]
    pub(in crate::compiler) fn expression_fallback_for_test(&self) -> Option<&'ast Expr> {
        self.expression_fallbacks.expression
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
        fallback: &'ast vela_syntax::ast::Expr,
    ) -> Self {
        let _ = fallback;
        Self {
            source,
            syntax,
            _ast: PhantomData,
            #[cfg(test)]
            fallback: Some(fallback),
        }
    }

    pub(in crate::compiler) fn from_syntax(
        source: Option<SourceId>,
        syntax: Option<SyntaxExpression>,
    ) -> Self {
        Self {
            source,
            syntax,
            _ast: PhantomData,
            #[cfg(test)]
            fallback: None,
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
    pub(in crate::compiler) fn fallback(&self) -> &'ast vela_syntax::ast::Expr {
        self.fallback
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
