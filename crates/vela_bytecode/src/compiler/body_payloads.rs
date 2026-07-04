use std::marker::PhantomData;

use vela_common::SourceId;
use vela_common::Span;
#[cfg(test)]
use vela_syntax::ast::AssignOp;
#[cfg(test)]
use vela_syntax::ast::Stmt;
#[cfg(test)]
use vela_syntax::ast::StmtKind;
use vela_syntax::ast::{
    AstNode, ElseBranch, ExprKind, IfExpr, MatchExpr, SyntaxArgument, SyntaxBlock,
    SyntaxExpression, SyntaxExpressionKind, SyntaxIfExpr, SyntaxMapEntry, SyntaxMatchArm,
    SyntaxMatchExpr, SyntaxPattern, SyntaxRecordExprField, SyntaxRecordPatternField,
    SyntaxStatement, SyntaxStatementKind,
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

#[derive(Clone)]
pub(in crate::compiler) struct CompilerExpressionPayload<'ast> {
    source: Option<SourceId>,
    syntax: Option<SyntaxExpression>,
    fallback: &'ast vela_syntax::ast::Expr,
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
    condition: Option<CompilerExpressionPayload<'ast>>,
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
    pub(super) fn statement_counts_differ_for_test(
        body: &SyntaxBlock,
        fallback_statements: &[Stmt],
    ) -> bool {
        syntax_body_statements(body).len() != fallback_statements.len()
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

    pub(super) fn requires_body_block_lookup(body: &SyntaxBlock) -> bool {
        // Production lowering is CST-only here; tests still exercise legacy
        // pairing until expression fallback payload fields are removed.
        if cfg!(test) {
            Self::requires_body_block_lookup_with_tail(body, true)
        } else {
            let _ = body;
            false
        }
    }

    fn requires_body_block_lookup_with_tail(
        body: &SyntaxBlock,
        allow_unterminated_tail: bool,
    ) -> bool {
        let syntax_statements = syntax_body_statements(body);
        let tail_index = syntax_statements.len().saturating_sub(1);
        syntax_statements
            .iter()
            .enumerate()
            .any(|(index, statement)| {
                let is_unterminated_tail = allow_unterminated_tail && index == tail_index;
                syntax_statement_requires_body_block_lookup(statement, is_unterminated_tail)
            })
    }

    pub(super) fn syntax_with_optional_body(source: SourceId, body: SyntaxBlock) -> Option<Self> {
        Some(Self::syntax_only(source, body))
    }

    pub(super) fn statement_payloads(&self) -> Vec<CompilerStatementPayload<'ast>> {
        let syntax_statements = syntax_body_statements(&self.syntax.body);

        #[cfg(test)]
        if let Some(fallback_statements) =
            parsed_body_fallback_for_tests(self.syntax.source, &self.syntax.body)
        {
            return paired_statement_payloads(
                self.syntax.source,
                &syntax_statements,
                fallback_statements,
            );
        }

        syntax_statements
            .into_iter()
            .map(|syntax| CompilerStatementPayload::new_syntax(self.syntax.source, syntax))
            .collect()
    }

    pub(super) fn syntax_statements_are_empty(&self) -> bool {
        syntax_body_statements(&self.syntax.body).is_empty()
    }

    pub(super) fn has_unmatched_extra_statement_payloads(&self) -> bool {
        let syntax_statements = syntax_body_statements(&self.syntax.body);

        #[cfg(test)]
        if let Some(fallback_statements) =
            parsed_body_fallback_for_tests(self.syntax.source, &self.syntax.body)
        {
            return syntax_statements.len() != fallback_statements.len();
        }

        let tail_index = syntax_statements.len().saturating_sub(1);
        syntax_statements
            .iter()
            .enumerate()
            .any(|(index, statement)| {
                syntax_statement_requires_body_block_lookup(statement, index == tail_index)
            })
    }

    pub(super) fn block_value<'payload>(
        &self,
        statements: &'payload [CompilerStatementPayload<'ast>],
    ) -> CompilerBlockValue<'payload, 'ast> {
        let Some((tail, prefix)) = statements.split_last() else {
            return CompilerBlockValue::Empty;
        };
        if matches!(
            tail.statement_kind(),
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
fn parsed_body_fallback_for_tests(source: SourceId, body: &SyntaxBlock) -> Option<&'static [Stmt]> {
    let body_text = body.syntax().text().to_string();
    let range = body.syntax().text_range();
    let start: u32 = range.start().into();
    let end: u32 = range.end().into();
    let mut text = " ".repeat(usize::try_from(start).ok()?);
    text.push_str(&body_text);
    let span = Span::new(source, start, end);
    let block = parse_owned_body_blocks_for_tests(source, &text, &[span])
        .into_iter()
        .next()?;
    Some(Box::leak(block.statements.into_boxed_slice()))
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

fn fallback_expr_syntax_kind(fallback: &vela_syntax::ast::Expr) -> Option<SyntaxExpressionKind> {
    match fallback.kind {
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

fn fallback_expr_matches_syntax_kind(
    fallback: &vela_syntax::ast::Expr,
    syntax_kind: SyntaxExpressionKind,
) -> bool {
    syntax_kind == SyntaxExpressionKind::Paren
        || fallback_expr_syntax_kind(fallback) == Some(syntax_kind)
}

fn fallback_path_self_shape_matches(
    fallback: &vela_syntax::ast::Expr,
    syntax_is_self: bool,
) -> bool {
    fallback_expr_syntax_kind(fallback) == Some(SyntaxExpressionKind::Path)
        && matches!(fallback.kind, ExprKind::SelfValue) == syntax_is_self
}

fn spans_overlap(left: Span, right: Span) -> bool {
    left.start < right.end && right.start < left.end
}

fn expression_block_syntax(expression: &SyntaxExpression) -> Option<SyntaxBlock> {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return expression_block_syntax(&inner);
    }
    expression.as_block()
}

fn match_arm_payloads_for_expr(
    source: Option<SourceId>,
    syntax: SyntaxMatchExpr,
    fallback: &MatchExpr,
) -> Option<Vec<CompilerMatchArmPayload>> {
    let syntax_arms = syntax.arms();
    if source.is_some() && syntax_arms.len() > fallback.arms.len() {
        return None;
    }
    Some(
        fallback
            .arms
            .iter()
            .enumerate()
            .map(|(index, _fallback)| CompilerMatchArmPayload {
                source,
                syntax: source.and_then(|_| syntax_arms.get(index).cloned()),
            })
            .collect(),
    )
}

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

fn if_payload_for_expr<'ast>(
    source: Option<SourceId>,
    syntax: SyntaxIfExpr,
    fallback: &'ast IfExpr,
) -> Option<CompilerIfPayload<'ast>> {
    let source = source?;
    let condition_syntax = syntax.condition();
    let condition = Some(CompilerExpressionPayload::from_fallback(
        Some(source),
        condition_syntax,
        &fallback.condition,
    ));
    let then_body = syntax
        .then_block()
        .map(|body| CompilerBodyPayload::nested_syntax(source, body));
    let else_body = if matches!(fallback.else_branch.as_ref(), Some(ElseBranch::Block(_))) {
        syntax
            .else_block()
            .map(|body| CompilerBodyPayload::nested_syntax(source, body))
    } else {
        None
    };
    let else_if = match fallback.else_branch.as_ref() {
        Some(ElseBranch::If(if_expr)) => {
            let syntax_if = syntax.else_if()?;
            if_payload_for_expr(Some(source), syntax_if, if_expr).map(Box::new)
        }
        Some(ElseBranch::Block(_)) | None => None,
    };
    Some(CompilerIfPayload {
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
    pub(in crate::compiler) fn syntax_only_for_test(
        source: SourceId,
        syntax: SyntaxStatement,
    ) -> Self {
        Self {
            source: Some(source),
            syntax: Some(syntax),
            _ast: PhantomData,
            fallback: None,
        }
    }

    #[cfg(test)]
    pub(super) fn syntax(source: SourceId, syntax: SyntaxStatement, fallback: &'ast Stmt) -> Self {
        Self {
            source: Some(source),
            syntax: Some(syntax),
            _ast: PhantomData,
            fallback: Some(fallback),
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

    pub(super) fn statement_kind(&self) -> Option<SyntaxStatementKind> {
        self.syntax_statement_kind()
    }

    pub(super) fn syntax_statement_kind(&self) -> Option<SyntaxStatementKind> {
        self.source?;
        self.stored_statement_kind()
    }

    pub(super) fn stored_statement_kind(&self) -> Option<SyntaxStatementKind> {
        self.syntax.as_ref().map(SyntaxStatement::statement_kind)
    }

    #[cfg(test)]
    pub(super) fn expression_kind(&self) -> Option<SyntaxExpressionKind> {
        self.source?;
        self.syntax_expression_kind()
    }

    #[cfg(test)]
    pub(super) fn syntax_expression_kind(&self) -> Option<SyntaxExpressionKind> {
        self.source?;
        self.stored_expression_kind()
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
    pub(super) fn value_expression_kind(&self) -> Option<SyntaxExpressionKind> {
        self.source?;
        self.syntax_value_expression_kind()
    }

    #[cfg(test)]
    pub(super) fn syntax_value_expression_kind(&self) -> Option<SyntaxExpressionKind> {
        self.source?;
        self.stored_value_expression_kind()
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

    #[cfg(test)]
    pub(super) fn let_initializer_kind(&self) -> Option<SyntaxExpressionKind> {
        self.source?;
        self.syntax_let_initializer_kind()
    }

    #[cfg(test)]
    pub(super) fn syntax_let_initializer_kind(&self) -> Option<SyntaxExpressionKind> {
        self.source?;
        self.stored_let_initializer_kind()
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

    #[cfg(test)]
    pub(in crate::compiler) fn let_initializer_syntax_literal(
        &self,
    ) -> Option<vela_syntax::ast::Literal> {
        self.let_initializer_syntax_literal_and_span()
            .map(|(literal, _)| literal)
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

    #[cfg(test)]
    pub(super) fn return_value_kind(&self) -> Option<SyntaxExpressionKind> {
        self.source?;
        self.syntax_return_value_kind()
    }

    #[cfg(test)]
    pub(super) fn syntax_return_value_kind(&self) -> Option<SyntaxExpressionKind> {
        self.source?;
        self.stored_return_value_kind()
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

    #[cfg(test)]
    pub(in crate::compiler) fn return_value_syntax_literal(
        &self,
    ) -> Option<vela_syntax::ast::Literal> {
        self.return_value_syntax_literal_and_span()
            .map(|(literal, _)| literal)
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

    #[cfg(test)]
    pub(super) fn for_iterable_expression_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        let StmtKind::For { iterable, .. } = &self.fallback?.kind else {
            return None;
        };
        self.source?;
        Some(CompilerExpressionPayload::from_fallback(
            self.source,
            self.syntax.as_ref()?.as_for()?.iterable(),
            iterable,
        ))
    }

    #[cfg(test)]
    pub(super) fn for_index_pattern_payload(&self) -> Option<CompilerPatternPayload> {
        let StmtKind::For { index_pattern, .. } = &self.fallback?.kind else {
            return None;
        };
        index_pattern.as_ref()?;
        self.source?;
        Some(CompilerPatternPayload::from_syntax(
            self.source,
            self.syntax.as_ref()?.as_for()?.index_pattern(),
        ))
    }

    #[cfg(test)]
    pub(super) fn for_value_pattern_payload(&self) -> Option<CompilerPatternPayload> {
        let StmtKind::For { .. } = &self.fallback?.kind else {
            return None;
        };
        self.source?;
        Some(CompilerPatternPayload::from_syntax(
            self.source,
            self.syntax.as_ref()?.as_for()?.value_pattern(),
        ))
    }

    pub(super) fn block_body_payload(&self) -> Option<CompilerBodyPayload<'ast>> {
        let body = self.syntax.as_ref()?.as_block()?;
        Some(CompilerBodyPayload::nested_syntax(self.source?, body))
    }

    #[cfg(test)]
    pub(super) fn for_body_payload(&self) -> Option<CompilerBodyPayload<'ast>> {
        Some(CompilerBodyPayload::nested_syntax(
            self.source?,
            self.syntax.as_ref()?.as_for()?.body()?,
        ))
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

    #[cfg(test)]
    fn assignment_value_expression(&self) -> Option<SyntaxExpression> {
        self.expression()?.as_assign()?.value()
    }

    #[cfg(test)]
    fn assignment_target_expression(&self) -> Option<SyntaxExpression> {
        self.expression()?.as_assign()?.target()
    }

    #[cfg(test)]
    pub(super) fn syntax_assignment_operator(&self) -> Option<AssignOp> {
        self.source?;
        self.stored_assignment_operator()
    }

    #[cfg(test)]
    pub(super) fn stored_assignment_operator(&self) -> Option<AssignOp> {
        let StmtKind::Expr(expr) = &self.fallback?.kind else {
            return None;
        };
        let ExprKind::Assign { .. } = &expr.kind else {
            return None;
        };
        self.expression()?.as_assign()?.operator()
    }

    #[cfg(test)]
    pub(in crate::compiler) fn assignment_target_expression_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        let StmtKind::Expr(expr) = &self.fallback?.kind else {
            return None;
        };
        let ExprKind::Assign { target, .. } = &expr.kind else {
            return None;
        };
        self.source?;
        Some(CompilerExpressionPayload::from_fallback(
            self.source,
            self.assignment_target_expression(),
            target,
        ))
    }

    #[cfg(test)]
    pub(in crate::compiler) fn assignment_value_expression_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        let StmtKind::Expr(expr) = &self.fallback?.kind else {
            return None;
        };
        let ExprKind::Assign { value, .. } = &expr.kind else {
            return None;
        };
        self.source?;
        Some(CompilerExpressionPayload::from_fallback(
            self.source,
            self.assignment_value_expression(),
            value,
        ))
    }

    #[cfg(test)]
    pub(super) fn assignment_value_kind(&self) -> Option<SyntaxExpressionKind> {
        self.source?;
        self.syntax_assignment_value_kind()
    }

    #[cfg(test)]
    pub(super) fn syntax_assignment_value_kind(&self) -> Option<SyntaxExpressionKind> {
        self.source?;
        self.stored_assignment_value_kind()
    }

    #[cfg(test)]
    pub(super) fn stored_assignment_value_kind(&self) -> Option<SyntaxExpressionKind> {
        self.assignment_value_expression()
            .map(|expression| expression.expression_kind())
    }

    #[cfg(test)]
    pub(super) fn assignment_value_block_body_payload(&self) -> Option<CompilerBodyPayload<'ast>> {
        Some(CompilerBodyPayload::nested_syntax(
            self.source?,
            self.assignment_value_expression()?.as_block()?,
        ))
    }

    #[cfg(test)]
    pub(in crate::compiler) fn call_argument_payloads(
        &self,
    ) -> Option<Vec<CompilerArgumentPayload>> {
        let StmtKind::Expr(expr) = &self.fallback?.kind else {
            return None;
        };
        let ExprKind::Call { args, .. } = &expr.kind else {
            return None;
        };
        let syntax_args = self.expression()?.as_call()?.arguments();
        if syntax_args.len() > args.len() {
            return None;
        }
        Some(
            args.iter()
                .enumerate()
                .map(|(index, _fallback)| CompilerArgumentPayload {
                    source: self.source,
                    syntax: syntax_args.get(index).cloned(),
                })
                .collect(),
        )
    }

    #[cfg(test)]
    pub(in crate::compiler) fn call_argument_value_payloads(
        &self,
    ) -> Option<Vec<CompilerExpressionPayload<'ast>>> {
        let StmtKind::Expr(expr) = &self.fallback?.kind else {
            return None;
        };
        let ExprKind::Call { args, .. } = &expr.kind else {
            return None;
        };
        Some(
            args.iter()
                .zip(self.call_argument_payloads()?)
                .map(|(fallback, payload)| payload.value_expression_payload(&fallback.value))
                .collect(),
        )
    }

    #[cfg(test)]
    pub(in crate::compiler) fn call_callee_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        let StmtKind::Expr(expr) = &self.fallback?.kind else {
            return None;
        };
        let ExprKind::Call { callee, .. } = &expr.kind else {
            return None;
        };
        self.source?;
        Some(CompilerExpressionPayload::from_fallback(
            self.source,
            self.expression()?.as_call()?.callee(),
            callee,
        ))
    }

    #[cfg(test)]
    pub(super) fn expression_block_body_payload(&self) -> Option<CompilerBodyPayload<'ast>> {
        Some(CompilerBodyPayload::nested_syntax(
            self.source?,
            self.expression()
                .and_then(|expression| expression.as_block())
                .or_else(|| self.syntax.as_ref()?.as_block())?,
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
    #[cfg(test)]
    pub(super) fn syntax(source: SourceId, syntax: SyntaxArgument) -> Self {
        Self {
            source: Some(source),
            syntax: Some(syntax),
        }
    }

    #[cfg(test)]
    pub(super) fn missing_value_syntax(source: SourceId) -> Self {
        Self {
            source: Some(source),
            syntax: None,
        }
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
        fallback: &'ast vela_syntax::ast::Expr,
    ) -> CompilerExpressionPayload<'ast> {
        CompilerExpressionPayload::from_fallback(
            self.source,
            self.source
                .and_then(|_| self.syntax.as_ref().and_then(SyntaxArgument::expression)),
            fallback,
        )
    }

    #[cfg(test)]
    pub(super) fn syntax_argument(&self) -> Option<&SyntaxArgument> {
        self.source?;
        self.syntax.as_ref()
    }
}

impl<'ast> CompilerExpressionPayload<'ast> {
    pub(in crate::compiler) fn from_fallback(
        source: Option<SourceId>,
        syntax: Option<SyntaxExpression>,
        fallback: &'ast vela_syntax::ast::Expr,
    ) -> Self {
        Self {
            source,
            syntax,
            fallback,
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
        syntax_kind == SyntaxExpressionKind::Paren
            || fallback_expr_syntax_kind(self.fallback) == Some(syntax_kind)
    }

    fn paired_expr_matches_stored_syntax_expr(&self, expr: &vela_syntax::ast::Expr) -> bool {
        let Some(kind) = self.stored_syntax_kind() else {
            return true;
        };
        if !fallback_expr_matches_syntax_kind(expr, kind) {
            return false;
        }
        (kind == SyntaxExpressionKind::Literal
            || fallback_expr_syntax_kind(self.fallback) == fallback_expr_syntax_kind(expr))
            && self.paired_expr_matches_stored_syntax_shape(expr)
    }

    pub(in crate::compiler) fn matches_paired_expr(&self, expr: &vela_syntax::ast::Expr) -> bool {
        self.paired_expr_matches_stored_syntax_expr(expr)
    }

    pub(in crate::compiler) fn is_aligned_with_paired_expr(
        &self,
        expr: &vela_syntax::ast::Expr,
    ) -> bool {
        self.matches_paired_expr(expr)
            && self
                .syntax_span()
                .is_some_and(|syntax_span| spans_overlap(syntax_span, expr.span))
    }

    fn paired_expr_matches_stored_syntax_shape(&self, expr: &vela_syntax::ast::Expr) -> bool {
        if self.stored_syntax_kind() != Some(SyntaxExpressionKind::Path) {
            return true;
        }
        fallback_path_self_shape_matches(expr, self.syntax_is_self())
    }

    #[cfg(test)]
    pub(in crate::compiler) fn fallback(&self) -> &'ast vela_syntax::ast::Expr {
        self.fallback
    }

    pub(in crate::compiler) fn source(&self) -> Option<SourceId> {
        self.source
    }

    pub(in crate::compiler) fn has_missing_syntax(&self) -> bool {
        self.source.is_some() && self.syntax.is_none()
    }

    #[cfg(test)]
    pub(in crate::compiler) fn kind(&self) -> Option<SyntaxExpressionKind> {
        self.source?;
        self.syntax_kind()
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
    pub(super) fn condition_payload(&self) -> Option<&CompilerExpressionPayload<'ast>> {
        self.condition.as_ref()
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
}
