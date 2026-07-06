use std::marker::PhantomData;

use vela_common::SourceId;
use vela_common::Span;
#[cfg(test)]
use vela_syntax::ast::ExprKind;
#[cfg(test)]
use vela_syntax::ast::MatchExpr;
#[cfg(test)]
use vela_syntax::ast::Stmt;
#[cfg(test)]
use vela_syntax::ast::StmtKind;
use vela_syntax::ast::{
    AstNode, SyntaxArgument, SyntaxBlock, SyntaxExpression, SyntaxExpressionKind, SyntaxForStmt,
    SyntaxIfExpr, SyntaxMapEntry, SyntaxMatchArm, SyntaxMatchExpr, SyntaxPattern,
    SyntaxRecordExprField, SyntaxRecordPatternField, SyntaxStatement, SyntaxStatementKind,
};
#[cfg(test)]
use vela_syntax::body_parser_support::parse_owned_body_blocks_for_tests;

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
    fallback: Option<&'ast Stmt>,
}

pub(super) struct CompilerMatchArmPayload {
    source: Option<SourceId>,
    syntax: Option<SyntaxMatchArm>,
}

#[derive(Clone)]
pub(in crate::compiler) struct CompilerPatternPayload {
    source: Option<SourceId>,
    syntax: Option<SyntaxPattern>,
}

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

pub(in crate::compiler) struct CompilerRecordFieldPayload {
    source: Option<SourceId>,
    syntax: Option<SyntaxRecordExprField>,
}

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

    #[cfg(test)]
    pub(super) fn compilation_statement_payloads(&self) -> Vec<CompilerStatementPayload<'ast>> {
        let fallback_statements =
            fallback_statements_for_syntax_body(self.syntax.source, &self.syntax.body);
        let syntax_statements = syntax_body_statements(&self.syntax.body);
        paired_statement_payloads(self.syntax.source, &syntax_statements, fallback_statements)
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
fn fallback_statements_for_syntax_body(source: SourceId, body: &SyntaxBlock) -> &'static [Stmt] {
    let body_text = body.syntax().text().to_string();
    let range = body.syntax().text_range();
    let start: u32 = range.start().into();
    let end: u32 = range.end().into();
    let mut text = " ".repeat(usize::try_from(start).expect("body start should fit usize"));
    text.push_str(&body_text);
    let span = Span::new(source, start, end);
    let Some(block) = parse_owned_body_blocks_for_tests(source, &text, &[span])
        .into_iter()
        .next()
    else {
        return &[];
    };
    Box::leak(block.statements.into_boxed_slice())
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
fn expr_syntax_kind(expr: &vela_syntax::ast::Expr) -> Option<SyntaxExpressionKind> {
    match expr.kind {
        ExprKind::Literal(_) | ExprKind::InterpolatedString(_) => {
            Some(SyntaxExpressionKind::Literal)
        }
        ExprKind::Path(_) | ExprKind::SelfValue => Some(SyntaxExpressionKind::Path),
        ExprKind::Unary { .. } => Some(SyntaxExpressionKind::Unary),
        ExprKind::Binary { .. } => Some(SyntaxExpressionKind::Binary),
        ExprKind::Assign { .. } => Some(SyntaxExpressionKind::Assign),
        ExprKind::Field { .. } => Some(SyntaxExpressionKind::Field),
        ExprKind::Call { .. } => Some(SyntaxExpressionKind::Call),
        ExprKind::Index { .. } => Some(SyntaxExpressionKind::Index),
        ExprKind::Try(_) => Some(SyntaxExpressionKind::Try),
        ExprKind::Array(_) => Some(SyntaxExpressionKind::Array),
        ExprKind::Map(_) => Some(SyntaxExpressionKind::Map),
        ExprKind::Record { .. } => Some(SyntaxExpressionKind::Record),
        ExprKind::Lambda { .. } => Some(SyntaxExpressionKind::Lambda),
        ExprKind::Block(_) => Some(SyntaxExpressionKind::Block),
        ExprKind::If(_) => Some(SyntaxExpressionKind::If),
        ExprKind::Match(_) => Some(SyntaxExpressionKind::Match),
        ExprKind::Error => None,
    }
}

#[cfg(test)]
fn expr_matches_syntax_kind(
    expr: &vela_syntax::ast::Expr,
    syntax_kind: SyntaxExpressionKind,
) -> bool {
    syntax_kind == SyntaxExpressionKind::Paren || expr_syntax_kind(expr) == Some(syntax_kind)
}

#[cfg(test)]
fn expr_path_self_shape_matches(expr: &vela_syntax::ast::Expr, syntax_is_self: bool) -> bool {
    expr_syntax_kind(expr) == Some(SyntaxExpressionKind::Path)
        && matches!(expr.kind, ExprKind::SelfValue) == syntax_is_self
}

fn expression_block_syntax(expression: &SyntaxExpression) -> Option<SyntaxBlock> {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return expression_block_syntax(&inner);
    }
    expression.as_block()
}

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
fn match_scrutinee_payload_for_expr<'ast>(
    source: Option<SourceId>,
    syntax: SyntaxMatchExpr,
    fallback: &'ast MatchExpr,
) -> CompilerExpressionPayload<'ast> {
    CompilerExpressionPayload::from_fallback(
        source,
        source.and_then(|_| syntax.scrutinee()),
        &fallback.scrutinee,
    )
}

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
            fallback: None,
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
            fallback,
        }
    }

    #[cfg(test)]
    pub(super) fn missing_child_payload_context(
        syntax: SyntaxStatement,
        fallback: &'ast Stmt,
    ) -> Self {
        Self {
            source: None,
            syntax: Some(syntax),
            _ast: PhantomData,
            fallback: Some(fallback),
        }
    }

    #[cfg(test)]
    pub(super) fn fallback(&self) -> &'ast Stmt {
        self.fallback
            .expect("statement payload has no owned statement fallback")
    }

    #[cfg(test)]
    pub(in crate::compiler) fn for_iterable_expression_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        if self.is_syntax_only() {
            return None;
        }
        let StmtKind::For { iterable, .. } = &self.fallback().kind else {
            return None;
        };
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
        let StmtKind::For { index_pattern, .. } = &self.fallback().kind else {
            return None;
        };
        index_pattern.as_ref()?;
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
        let StmtKind::For { .. } = &self.fallback().kind else {
            return None;
        };
        Some(CompilerPatternPayload::from_syntax(
            Some(self.syntax_statement_span()?.source),
            self.syntax_statement()?.as_for()?.value_pattern(),
        ))
    }

    pub(super) fn is_syntax_only(&self) -> bool {
        #[cfg(test)]
        {
            self.fallback.is_none()
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

    pub(super) fn let_initializer_block_body_payload(&self) -> Option<CompilerBodyPayload<'ast>> {
        let body = self.syntax.as_ref()?.as_let()?.initializer()?.as_block()?;
        Some(CompilerBodyPayload::nested_syntax(self.source?, body))
    }

    #[cfg(test)]
    pub(in crate::compiler) fn let_initializer_expression_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        let StmtKind::Let {
            value: Some(value), ..
        } = &self.fallback?.kind
        else {
            return None;
        };
        Some(CompilerExpressionPayload::from_fallback(
            self.source,
            self.syntax.as_ref()?.as_let()?.initializer(),
            value,
        ))
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
        let StmtKind::Return(Some(value)) = &self.fallback?.kind else {
            return None;
        };
        Some(CompilerExpressionPayload::from_fallback(
            self.source,
            self.syntax.as_ref()?.as_return()?.expression(),
            value,
        ))
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
        let StmtKind::Expr(expr) = &self.fallback?.kind else {
            return None;
        };
        Some(CompilerExpressionPayload::from_fallback(
            self.source,
            self.expression(),
            expr,
        ))
    }

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

    #[cfg(test)]
    pub(super) fn syntax_argument(&self) -> Option<&SyntaxArgument> {
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
    pub(in crate::compiler) fn missing_syntax(
        source: SourceId,
        fallback: &'ast vela_syntax::ast::Expr,
    ) -> Self {
        Self::from_fallback(Some(source), None, fallback)
    }

    fn matches_syntax_kind(&self, syntax_kind: SyntaxExpressionKind) -> bool {
        #[cfg(test)]
        if let Some(fallback) = self.fallback {
            return syntax_kind == SyntaxExpressionKind::Paren
                || expr_syntax_kind(fallback) == Some(syntax_kind);
        }

        syntax_kind == SyntaxExpressionKind::Paren || self.stored_syntax_kind() == Some(syntax_kind)
    }

    #[cfg(test)]
    fn paired_expr_matches_stored_syntax_expr(&self, expr: &vela_syntax::ast::Expr) -> bool {
        let Some(kind) = self.stored_syntax_kind() else {
            return true;
        };
        if !expr_matches_syntax_kind(expr, kind) {
            return false;
        }
        if let Some(fallback) = self.fallback
            && kind != SyntaxExpressionKind::Literal
            && expr_syntax_kind(fallback) != expr_syntax_kind(expr)
        {
            return false;
        }
        self.paired_expr_matches_stored_syntax_shape(expr)
    }

    #[cfg(test)]
    pub(in crate::compiler) fn matches_paired_expr(&self, expr: &vela_syntax::ast::Expr) -> bool {
        self.paired_expr_matches_stored_syntax_expr(expr)
    }

    #[cfg(test)]
    fn paired_expr_matches_stored_syntax_shape(&self, expr: &vela_syntax::ast::Expr) -> bool {
        if self.stored_syntax_kind() != Some(SyntaxExpressionKind::Path) {
            return true;
        }
        expr_path_self_shape_matches(expr, self.syntax_is_self())
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

    pub(in crate::compiler) fn matches_stored_syntax_kind(&self) -> bool {
        self.stored_syntax_kind()
            .is_some_and(|kind| self.matches_syntax_kind(kind))
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
