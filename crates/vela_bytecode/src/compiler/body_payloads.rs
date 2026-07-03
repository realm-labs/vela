use std::marker::PhantomData;

use vela_common::SourceId;
use vela_common::Span;
#[cfg(test)]
use vela_syntax::ast::Argument;
#[cfg(test)]
use vela_syntax::ast::BinaryOp;
#[cfg(test)]
use vela_syntax::ast::InterpolatedStringPart;
#[cfg(test)]
use vela_syntax::ast::MapEntry;
#[cfg(test)]
use vela_syntax::ast::RecordField;
use vela_syntax::ast::{
    AssignOp, AstNode, Block, ElseBranch, ExprKind, IfExpr, MatchExpr, Pattern, Stmt, StmtKind,
    SyntaxArgument, SyntaxBlock, SyntaxExpression, SyntaxExpressionKind, SyntaxIfExpr,
    SyntaxMapEntry, SyntaxMatchArm, SyntaxMatchExpr, SyntaxPattern, SyntaxRecordExprField,
    SyntaxRecordPatternField, SyntaxStatement, SyntaxStatementKind,
};

mod expression_payloads;
mod simple_values;

// Temporary 1200-line exception: this module owns the transitional CST plus
// old-body-fallback pairing invariant. Splitting the remaining fallback side
// before the hard switch would obscure that invariant and create churn in code
// that is scheduled for deletion when body payloads become CST-only.

pub(super) use simple_values::{
    expression_syntax_negated_number_literal, expression_syntax_path_or_self,
    expression_syntax_range_operands,
};

use simple_values::{expression_syntax_literal, syntax_statement_requires_body_block_lookup};

#[derive(Clone)]
pub(super) struct SyntaxBodyPayload {
    pub(super) source: SourceId,
    pub(super) body: SyntaxBlock,
}

#[derive(Clone)]
pub(super) struct CompilerBodyPayload<'ast> {
    syntax: SyntaxBodyPayload,
    _ast: PhantomData<&'ast ()>,
    #[cfg(test)]
    fallback_statements: Option<&'ast [Stmt]>,
    #[cfg(test)]
    fallback_block: Option<&'ast Block>,
}

#[derive(Clone, Copy)]
pub(super) struct CompilerBodyFallback<'ast> {
    _ast: PhantomData<&'ast ()>,
    #[cfg(test)]
    statements: &'ast [Stmt],
    #[cfg(test)]
    block: Option<&'ast Block>,
}

pub(super) struct CompilerStatementPayload<'ast> {
    source: Option<SourceId>,
    syntax: Option<SyntaxStatement>,
    _ast: PhantomData<&'ast ()>,
    #[cfg(test)]
    fallback: Option<&'ast Stmt>,
}

pub(super) struct CompilerMatchArmPayload<'ast> {
    source: Option<SourceId>,
    syntax: Option<SyntaxMatchArm>,
    pattern_fallback: &'ast Pattern,
    body_fallback: &'ast vela_syntax::ast::Expr,
    #[cfg(test)]
    body_block_fallback: Option<&'ast Block>,
}

#[derive(Clone)]
pub(in crate::compiler) struct CompilerPatternPayload<'ast> {
    source: Option<SourceId>,
    syntax: Option<SyntaxPattern>,
    _ast: PhantomData<&'ast ()>,
}

pub(in crate::compiler) struct CompilerRecordPatternFieldPayload<'ast> {
    source: Option<SourceId>,
    syntax: Option<SyntaxRecordPatternField>,
    _ast: PhantomData<&'ast ()>,
}

pub(in crate::compiler) struct CompilerArgumentPayload<'ast> {
    source: Option<SourceId>,
    syntax: Option<SyntaxArgument>,
    _ast: PhantomData<&'ast ()>,
}

#[derive(Clone)]
pub(in crate::compiler) struct CompilerExpressionPayload<'ast> {
    source: Option<SourceId>,
    syntax: Option<SyntaxExpression>,
    fallback: &'ast vela_syntax::ast::Expr,
    fallback_kind: CompilerExpressionFallbackKind<'ast>,
}

#[derive(Clone, Copy)]
pub(in crate::compiler) enum CompilerExpressionFallbackKind<'ast> {
    Literal,
    Path,
    SelfValue,
    #[cfg(test)]
    Block(&'ast Block),
    #[cfg(not(test))]
    Block,
    #[cfg(test)]
    If(&'ast IfExpr),
    #[cfg(not(test))]
    If,
    #[cfg(test)]
    Match(&'ast MatchExpr),
    #[cfg(not(test))]
    Match,
    #[cfg(test)]
    Assign {
        target: &'ast vela_syntax::ast::Expr,
        value: &'ast vela_syntax::ast::Expr,
    },
    #[cfg(not(test))]
    Assign,
    #[cfg(test)]
    Unary {
        expr: &'ast vela_syntax::ast::Expr,
    },
    #[cfg(not(test))]
    Unary,
    #[cfg(test)]
    Try(&'ast vela_syntax::ast::Expr),
    #[cfg(not(test))]
    Try,
    #[cfg(test)]
    Binary {
        op: BinaryOp,
        left: &'ast vela_syntax::ast::Expr,
        right: &'ast vela_syntax::ast::Expr,
    },
    #[cfg(not(test))]
    Binary,
    #[cfg(test)]
    Call {
        callee: &'ast vela_syntax::ast::Expr,
        args: &'ast [Argument],
    },
    #[cfg(not(test))]
    Call,
    #[cfg(test)]
    Field {
        base: &'ast vela_syntax::ast::Expr,
    },
    #[cfg(not(test))]
    Field,
    #[cfg(test)]
    Index {
        base: &'ast vela_syntax::ast::Expr,
        index: &'ast vela_syntax::ast::Expr,
    },
    #[cfg(not(test))]
    Index,
    #[cfg(test)]
    Lambda {
        body: &'ast vela_syntax::ast::Expr,
    },
    #[cfg(not(test))]
    Lambda,
    #[cfg(test)]
    Array(&'ast [vela_syntax::ast::Expr]),
    #[cfg(not(test))]
    Array,
    #[cfg(test)]
    Map(&'ast [MapEntry]),
    #[cfg(not(test))]
    Map,
    #[cfg(test)]
    Record {
        fields: &'ast [RecordField],
    },
    #[cfg(not(test))]
    Record,
    #[cfg(test)]
    InterpolatedString(&'ast [InterpolatedStringPart]),
    #[cfg(not(test))]
    InterpolatedString,
    #[cfg(not(test))]
    _Ast(PhantomData<&'ast ()>),
    Other,
}

pub(in crate::compiler) struct CompilerMapEntryPayload<'ast> {
    source: Option<SourceId>,
    syntax: Option<SyntaxMapEntry>,
    _ast: PhantomData<&'ast ()>,
}

pub(in crate::compiler) struct CompilerRecordFieldPayload<'ast> {
    source: Option<SourceId>,
    syntax: Option<SyntaxRecordExprField>,
    _ast: PhantomData<&'ast ()>,
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
    pub(super) fn syntax(source: SourceId, body: SyntaxBlock, fallback: &'ast Block) -> Self {
        Self::with_fallback(source, body, CompilerBodyFallback::block(fallback))
    }

    #[cfg(test)]
    fn with_fallback(
        source: SourceId,
        body: SyntaxBlock,
        fallback: CompilerBodyFallback<'ast>,
    ) -> Self {
        Self {
            syntax: SyntaxBodyPayload { source, body },
            _ast: PhantomData,
            fallback_statements: Some(fallback.statements),
            fallback_block: fallback.block,
        }
    }

    #[cfg(test)]
    fn nested(source: SourceId, body: SyntaxBlock, fallback: CompilerBodyFallback<'ast>) -> Self {
        Self::with_fallback(source, body, fallback)
    }

    #[cfg(not(test))]
    fn syntax_only(source: SourceId, body: SyntaxBlock) -> Self {
        Self {
            syntax: SyntaxBodyPayload { source, body },
            _ast: PhantomData,
        }
    }

    pub(super) fn nested_syntax_optional(
        source: SourceId,
        body: SyntaxBlock,
        fallback: Option<CompilerBodyFallback<'ast>>,
    ) -> Option<Self> {
        #[cfg(not(test))]
        {
            let _ = fallback;
            Some(Self::syntax_only(source, body))
        }
        #[cfg(test)]
        {
            if !Self::requires_body_block_lookup(&body) {
                return Self::syntax_only_without_body_lookup(source, body);
            }
            fallback.map(|fallback| Self::nested(source, body, fallback))
        }
    }

    #[cfg(test)]
    pub(super) fn syntax_only_without_body_lookup(
        source: SourceId,
        body: SyntaxBlock,
    ) -> Option<Self> {
        (!Self::requires_body_block_lookup(&body)).then_some(Self {
            syntax: SyntaxBodyPayload { source, body },
            _ast: PhantomData,
            fallback_statements: None,
            fallback_block: None,
        })
    }

    pub(super) fn requires_body_block_lookup(body: &SyntaxBlock) -> bool {
        Self::requires_body_block_lookup_with_tail(body, true)
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

    pub(super) fn syntax_with_optional_body(
        source: SourceId,
        body: SyntaxBlock,
        fallback: Option<CompilerBodyFallback<'ast>>,
    ) -> Option<Self> {
        #[cfg(not(test))]
        {
            let _ = fallback;
            Some(Self::syntax_only(source, body))
        }
        #[cfg(test)]
        {
            match fallback {
                Some(fallback) => Some(Self::with_fallback(source, body, fallback)),
                None => Self::syntax_only_without_body_lookup(source, body),
            }
        }
    }

    #[cfg(test)]
    pub(super) fn fallback(&self) -> &'ast Block {
        self.fallback_block
            .expect("body payload has no owned body fallback")
    }

    #[cfg(test)]
    pub(super) fn has_fallback_statements(&self) -> bool {
        self.fallback_statements.is_some()
    }

    pub(super) fn statement_payloads(&self) -> Vec<CompilerStatementPayload<'ast>> {
        let syntax_statements = syntax_body_statements(&self.syntax.body);
        #[cfg(not(test))]
        {
            syntax_statements
                .into_iter()
                .map(|syntax| CompilerStatementPayload {
                    source: Some(self.syntax.source),
                    syntax: Some(syntax),
                    _ast: PhantomData,
                })
                .collect()
        }
        #[cfg(test)]
        {
            match self.fallback_statements {
                Some(fallback_statements) => fallback_statements
                    .iter()
                    .enumerate()
                    .map(|(index, fallback)| CompilerStatementPayload {
                        source: Some(self.syntax.source),
                        syntax: syntax_statements.get(index).cloned(),
                        _ast: PhantomData,
                        fallback: syntax_statements
                            .get(index)
                            .is_none_or(|statement| {
                                let is_unterminated_tail =
                                    index == syntax_statements.len().saturating_sub(1);
                                syntax_statement_requires_body_block_lookup(
                                    statement,
                                    is_unterminated_tail,
                                )
                            })
                            .then_some(fallback),
                    })
                    .collect(),
                None => syntax_statements
                    .into_iter()
                    .map(|syntax| CompilerStatementPayload {
                        source: Some(self.syntax.source),
                        syntax: Some(syntax),
                        _ast: PhantomData,
                        fallback: None,
                    })
                    .collect(),
            }
        }
    }

    pub(super) fn syntax_statements_are_empty(&self) -> bool {
        syntax_body_statements(&self.syntax.body).is_empty()
    }

    pub(super) fn has_unmatched_extra_statement_payloads(&self) -> bool {
        let syntax_statements = syntax_body_statements(&self.syntax.body);
        #[cfg(not(test))]
        {
            let tail_index = syntax_statements.len().saturating_sub(1);
            syntax_statements
                .iter()
                .enumerate()
                .any(|(index, statement)| {
                    syntax_statement_requires_body_block_lookup(statement, index == tail_index)
                })
        }
        #[cfg(test)]
        {
            match self.fallback_statements {
                Some(fallback_statements) => syntax_statements.len() != fallback_statements.len(),
                None => {
                    let tail_index = syntax_statements.len().saturating_sub(1);
                    syntax_statements
                        .iter()
                        .enumerate()
                        .any(|(index, statement)| {
                            syntax_statement_requires_body_block_lookup(
                                statement,
                                index == tail_index,
                            )
                        })
                }
            }
        }
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

impl<'ast> CompilerBodyFallback<'ast> {
    pub(super) fn block(block: &'ast Block) -> Self {
        #[cfg(not(test))]
        let _ = block;
        Self {
            _ast: PhantomData,
            #[cfg(test)]
            statements: &block.statements,
            #[cfg(test)]
            block: Some(block),
        }
    }

    #[cfg(test)]
    pub(super) const fn statements_with_block(
        statements: &'ast [Stmt],
        block: &'ast Block,
    ) -> Self {
        Self {
            _ast: PhantomData,
            statements,
            block: Some(block),
        }
    }
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

fn expression_block_syntax(expression: &SyntaxExpression) -> Option<SyntaxBlock> {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return expression_block_syntax(&inner);
    }
    expression.as_block()
}

fn match_arm_payloads_for_expr<'ast>(
    source: Option<SourceId>,
    syntax: SyntaxMatchExpr,
    fallback: &'ast MatchExpr,
) -> Option<Vec<CompilerMatchArmPayload<'ast>>> {
    let syntax_arms = syntax.arms();
    if source.is_some() && syntax_arms.len() > fallback.arms.len() {
        return None;
    }
    Some(
        fallback
            .arms
            .iter()
            .enumerate()
            .map(|(index, fallback)| CompilerMatchArmPayload {
                source,
                syntax: source.and_then(|_| syntax_arms.get(index).cloned()),
                pattern_fallback: &fallback.pattern,
                body_fallback: &fallback.body,
                #[cfg(test)]
                body_block_fallback: expression_fallback_block(&fallback.body),
            })
            .collect(),
    )
}

#[cfg(test)]
fn expression_fallback_block(expr: &vela_syntax::ast::Expr) -> Option<&Block> {
    match &expr.kind {
        ExprKind::Block(block) => Some(block),
        _ => None,
    }
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
    let then_body = syntax.then_block().and_then(|body| {
        CompilerBodyPayload::nested_syntax_optional(
            source,
            body,
            Some(CompilerBodyFallback::block(&fallback.then_branch)),
        )
    });
    let else_body = match fallback.else_branch.as_ref() {
        Some(ElseBranch::Block(block)) => syntax.else_block().and_then(|body| {
            CompilerBodyPayload::nested_syntax_optional(
                source,
                body,
                Some(CompilerBodyFallback::block(block)),
            )
        }),
        Some(ElseBranch::If(_)) | None => None,
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

    pub(super) fn optional_fallback(&self) -> Option<&'ast Stmt> {
        #[cfg(not(test))]
        {
            None
        }
        #[cfg(test)]
        {
            self.fallback
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
        #[cfg(test)]
        let fallback = match self.optional_fallback().map(|fallback| &fallback.kind) {
            Some(StmtKind::Let {
                value: Some(value), ..
            }) => match &value.kind {
                ExprKind::Block(block) => Some(block),
                _ => return None,
            },
            Some(_) => return None,
            None => None,
        };
        #[cfg(not(test))]
        let fallback = None;
        CompilerBodyPayload::nested_syntax_optional(
            self.source?,
            self.syntax.as_ref()?.as_let()?.initializer()?.as_block()?,
            fallback.map(CompilerBodyFallback::block),
        )
    }

    pub(super) fn let_initializer_if_payload(&self) -> Option<CompilerIfPayload<'ast>> {
        let StmtKind::Let {
            value: Some(value), ..
        } = &self.optional_fallback()?.kind
        else {
            return None;
        };
        let ExprKind::If(if_expr) = &value.kind else {
            return None;
        };
        if_payload_for_expr(
            self.source,
            self.syntax.as_ref()?.as_let()?.initializer()?.as_if()?,
            if_expr,
        )
    }

    pub(super) fn let_initializer_match_arm_payloads(
        &self,
    ) -> Option<Vec<CompilerMatchArmPayload<'ast>>> {
        let StmtKind::Let {
            value: Some(value), ..
        } = &self.optional_fallback()?.kind
        else {
            return None;
        };
        let ExprKind::Match(match_expr) = &value.kind else {
            return None;
        };
        match_arm_payloads_for_expr(
            self.source,
            self.syntax.as_ref()?.as_let()?.initializer()?.as_match()?,
            match_expr,
        )
    }

    pub(in crate::compiler) fn let_initializer_expression_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        let StmtKind::Let {
            value: Some(value), ..
        } = &self.optional_fallback()?.kind
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
        #[cfg(test)]
        let fallback = match self.optional_fallback().map(|fallback| &fallback.kind) {
            Some(StmtKind::Return(Some(value))) => match &value.kind {
                ExprKind::Block(block) => Some(block),
                _ => return None,
            },
            Some(_) => return None,
            None => None,
        };
        #[cfg(not(test))]
        let fallback = None;
        CompilerBodyPayload::nested_syntax_optional(
            self.source?,
            self.syntax
                .as_ref()?
                .as_return()?
                .expression()?
                .as_block()?,
            fallback.map(CompilerBodyFallback::block),
        )
    }

    pub(super) fn return_value_if_payload(&self) -> Option<CompilerIfPayload<'ast>> {
        let StmtKind::Return(Some(value)) = &self.optional_fallback()?.kind else {
            return None;
        };
        let ExprKind::If(if_expr) = &value.kind else {
            return None;
        };
        if_payload_for_expr(
            self.source,
            self.syntax.as_ref()?.as_return()?.expression()?.as_if()?,
            if_expr,
        )
    }

    pub(super) fn return_value_match_arm_payloads(
        &self,
    ) -> Option<Vec<CompilerMatchArmPayload<'ast>>> {
        let StmtKind::Return(Some(value)) = &self.optional_fallback()?.kind else {
            return None;
        };
        let ExprKind::Match(match_expr) = &value.kind else {
            return None;
        };
        match_arm_payloads_for_expr(
            self.source,
            self.syntax
                .as_ref()?
                .as_return()?
                .expression()?
                .as_match()?,
            match_expr,
        )
    }

    pub(in crate::compiler) fn return_value_expression_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        let StmtKind::Return(Some(value)) = &self.optional_fallback()?.kind else {
            return None;
        };
        Some(CompilerExpressionPayload::from_fallback(
            self.source,
            self.syntax.as_ref()?.as_return()?.expression(),
            value,
        ))
    }

    pub(super) fn for_iterable_expression_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        let StmtKind::For { iterable, .. } = &self.optional_fallback()?.kind else {
            return None;
        };
        self.source?;
        Some(CompilerExpressionPayload::from_fallback(
            self.source,
            self.syntax.as_ref()?.as_for()?.iterable(),
            iterable,
        ))
    }

    pub(super) fn for_index_pattern_payload(&self) -> Option<CompilerPatternPayload<'ast>> {
        let StmtKind::For { index_pattern, .. } = &self.optional_fallback()?.kind else {
            return None;
        };
        self.source?;
        Some(CompilerPatternPayload::from_fallback(
            self.source,
            self.syntax.as_ref()?.as_for()?.index_pattern(),
            index_pattern.as_ref()?,
        ))
    }

    pub(super) fn for_value_pattern_payload(&self) -> Option<CompilerPatternPayload<'ast>> {
        let StmtKind::For { pattern, .. } = &self.optional_fallback()?.kind else {
            return None;
        };
        self.source?;
        Some(CompilerPatternPayload::from_fallback(
            self.source,
            self.syntax.as_ref()?.as_for()?.value_pattern(),
            pattern,
        ))
    }

    pub(super) fn if_payload(&self) -> Option<CompilerIfPayload<'ast>> {
        let StmtKind::Expr(expr) = &self.optional_fallback()?.kind else {
            return None;
        };
        let ExprKind::If(if_expr) = &expr.kind else {
            return None;
        };
        if_payload_for_expr(self.source, self.syntax.as_ref()?.as_if()?, if_expr)
    }

    pub(super) fn block_body_payload(&self) -> Option<CompilerBodyPayload<'ast>> {
        #[cfg(test)]
        let fallback = match self.optional_fallback().map(|fallback| &fallback.kind) {
            Some(StmtKind::Block(fallback)) => Some(fallback),
            Some(_) => return None,
            None => None,
        };
        #[cfg(not(test))]
        let fallback = None;
        CompilerBodyPayload::nested_syntax_optional(
            self.source?,
            self.syntax.as_ref()?.as_block()?,
            fallback.map(CompilerBodyFallback::block),
        )
    }

    pub(super) fn for_body_payload(&self) -> Option<CompilerBodyPayload<'ast>> {
        #[cfg(test)]
        let fallback = match self.optional_fallback().map(|fallback| &fallback.kind) {
            Some(StmtKind::For { body, .. }) => Some(CompilerBodyFallback::block(body)),
            Some(_) => return None,
            None => None,
        };
        #[cfg(not(test))]
        let fallback = None;
        CompilerBodyPayload::nested_syntax_optional(
            self.source?,
            self.syntax.as_ref()?.as_for()?.body()?,
            fallback,
        )
    }

    pub(super) fn match_arm_payloads(&self) -> Option<Vec<CompilerMatchArmPayload<'ast>>> {
        let StmtKind::Expr(expr) = &self.optional_fallback()?.kind else {
            return None;
        };
        let ExprKind::Match(match_expr) = &expr.kind else {
            return None;
        };
        match_arm_payloads_for_expr(self.source, self.syntax.as_ref()?.as_match()?, match_expr)
    }

    pub(in crate::compiler) fn match_scrutinee_payload_with_fallback(
        &self,
    ) -> Option<(&'ast MatchExpr, CompilerExpressionPayload<'ast>)> {
        let StmtKind::Expr(expr) = &self.optional_fallback()?.kind else {
            return None;
        };
        let ExprKind::Match(match_expr) = &expr.kind else {
            return None;
        };
        self.source?;
        let payload = match_scrutinee_payload_for_expr(
            self.source,
            self.syntax.as_ref()?.as_match()?,
            match_expr,
        );
        Some((match_expr, payload))
    }

    #[cfg(test)]
    pub(super) fn match_scrutinee_payload(&self) -> Option<CompilerExpressionPayload<'ast>> {
        let StmtKind::Expr(expr) = &self.optional_fallback()?.kind else {
            return None;
        };
        let ExprKind::Match(match_expr) = &expr.kind else {
            return None;
        };
        self.source?;
        Some(match_scrutinee_payload_for_expr(
            self.source,
            self.syntax.as_ref()?.as_match()?,
            match_expr,
        ))
    }

    fn expression(&self) -> Option<SyntaxExpression> {
        self.syntax.as_ref()?.as_expr()?.expression()
    }

    pub(in crate::compiler) fn expression_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        let StmtKind::Expr(expr) = &self.optional_fallback()?.kind else {
            return None;
        };
        Some(CompilerExpressionPayload::from_fallback(
            self.source,
            self.expression(),
            expr,
        ))
    }

    fn assignment_value_expression(&self) -> Option<SyntaxExpression> {
        self.expression()?.as_assign()?.value()
    }

    fn assignment_target_expression(&self) -> Option<SyntaxExpression> {
        self.expression()?.as_assign()?.target()
    }

    #[cfg(test)]
    pub(super) fn syntax_assignment_operator(&self) -> Option<AssignOp> {
        self.source?;
        self.stored_assignment_operator()
    }

    pub(super) fn stored_assignment_operator(&self) -> Option<AssignOp> {
        let StmtKind::Expr(expr) = &self.optional_fallback()?.kind else {
            return None;
        };
        let ExprKind::Assign { .. } = &expr.kind else {
            return None;
        };
        self.expression()?.as_assign()?.operator()
    }

    pub(in crate::compiler) fn assignment_target_expression_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        let StmtKind::Expr(expr) = &self.optional_fallback()?.kind else {
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

    pub(in crate::compiler) fn assignment_value_expression_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        let StmtKind::Expr(expr) = &self.optional_fallback()?.kind else {
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

    pub(super) fn stored_assignment_value_kind(&self) -> Option<SyntaxExpressionKind> {
        self.assignment_value_expression()
            .map(|expression| expression.expression_kind())
    }

    pub(super) fn assignment_value_block_body_payload(&self) -> Option<CompilerBodyPayload<'ast>> {
        #[cfg(test)]
        let fallback = match self.optional_fallback().map(|fallback| &fallback.kind) {
            Some(StmtKind::Expr(expr)) => match &expr.kind {
                ExprKind::Assign { value, .. } => match &value.kind {
                    ExprKind::Block(block) => Some(CompilerBodyFallback::block(block)),
                    _ => return None,
                },
                _ => return None,
            },
            Some(_) => return None,
            None => None,
        };
        #[cfg(not(test))]
        let fallback = None;
        CompilerBodyPayload::nested_syntax_optional(
            self.source?,
            self.assignment_value_expression()?.as_block()?,
            fallback,
        )
    }

    pub(super) fn assignment_value_if_payload(&self) -> Option<CompilerIfPayload<'ast>> {
        let StmtKind::Expr(expr) = &self.optional_fallback()?.kind else {
            return None;
        };
        let ExprKind::Assign { value, .. } = &expr.kind else {
            return None;
        };
        let ExprKind::If(if_expr) = &value.kind else {
            return None;
        };
        if_payload_for_expr(
            self.source,
            self.assignment_value_expression()?.as_if()?,
            if_expr,
        )
    }

    pub(super) fn assignment_value_match_arm_payloads(
        &self,
    ) -> Option<Vec<CompilerMatchArmPayload<'ast>>> {
        let StmtKind::Expr(expr) = &self.optional_fallback()?.kind else {
            return None;
        };
        let ExprKind::Assign { value, .. } = &expr.kind else {
            return None;
        };
        let ExprKind::Match(match_expr) = &value.kind else {
            return None;
        };
        match_arm_payloads_for_expr(
            self.source,
            self.assignment_value_expression()?.as_match()?,
            match_expr,
        )
    }

    pub(in crate::compiler) fn call_argument_payloads(
        &self,
    ) -> Option<Vec<CompilerArgumentPayload<'ast>>> {
        let StmtKind::Expr(expr) = &self.optional_fallback()?.kind else {
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
                    _ast: PhantomData,
                })
                .collect(),
        )
    }

    #[cfg(test)]
    pub(in crate::compiler) fn call_argument_value_payloads(
        &self,
    ) -> Option<Vec<CompilerExpressionPayload<'ast>>> {
        let StmtKind::Expr(expr) = &self.optional_fallback()?.kind else {
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

    pub(in crate::compiler) fn call_callee_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        let StmtKind::Expr(expr) = &self.optional_fallback()?.kind else {
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

    pub(super) fn expression_block_body_payload(&self) -> Option<CompilerBodyPayload<'ast>> {
        #[cfg(test)]
        let fallback = match self.optional_fallback().map(|fallback| &fallback.kind) {
            Some(StmtKind::Expr(expr)) => match &expr.kind {
                ExprKind::Block(block) => Some(CompilerBodyFallback::block(block)),
                _ => return None,
            },
            Some(_) => return None,
            None => None,
        };
        #[cfg(not(test))]
        let fallback = None;
        CompilerBodyPayload::nested_syntax_optional(
            self.source?,
            self.expression()
                .and_then(|expression| expression.as_block())
                .or_else(|| self.syntax.as_ref()?.as_block())?,
            fallback,
        )
    }

    pub(super) fn expression_statement_block_body_payload(
        &self,
    ) -> Option<CompilerBodyPayload<'ast>> {
        #[cfg(test)]
        let fallback = match self.optional_fallback().map(|fallback| &fallback.kind) {
            Some(StmtKind::Expr(expr)) => match &expr.kind {
                ExprKind::Block(block) => Some(block),
                _ => return None,
            },
            Some(_) => return None,
            None => None,
        };
        #[cfg(not(test))]
        let fallback = None;
        CompilerBodyPayload::nested_syntax_optional(
            self.source?,
            expression_block_syntax(&self.expression()?)?,
            fallback.map(CompilerBodyFallback::block),
        )
    }

    pub(in crate::compiler) fn expression_if_payload_with_fallback(
        &self,
    ) -> Option<(&'ast IfExpr, CompilerIfPayload<'ast>)> {
        let StmtKind::Expr(expr) = &self.optional_fallback()?.kind else {
            return None;
        };
        let ExprKind::If(if_expr) = &expr.kind else {
            return None;
        };
        let payload = if_payload_for_expr(
            self.source,
            self.expression()
                .and_then(|expression| expression.as_if())
                .or_else(|| self.syntax.as_ref()?.as_if())?,
            if_expr,
        )?;
        Some((if_expr, payload))
    }

    pub(in crate::compiler) fn expression_match_payloads_with_fallback(
        &self,
    ) -> Option<(
        &'ast MatchExpr,
        CompilerExpressionPayload<'ast>,
        Vec<CompilerMatchArmPayload<'ast>>,
    )> {
        let StmtKind::Expr(expr) = &self.optional_fallback()?.kind else {
            return None;
        };
        let ExprKind::Match(match_expr) = &expr.kind else {
            return None;
        };
        self.source?;
        let syntax = self
            .expression()
            .and_then(|expression| expression.as_match())
            .or_else(|| self.syntax.as_ref()?.as_match())?;
        let scrutinee_payload =
            match_scrutinee_payload_for_expr(self.source, syntax.clone(), match_expr);
        let arm_payloads = match_arm_payloads_for_expr(self.source, syntax, match_expr)?;
        Some((match_expr, scrutinee_payload, arm_payloads))
    }

    #[cfg(test)]
    pub(super) fn syntax_statement(&self) -> Option<&SyntaxStatement> {
        self.source?;
        self.syntax.as_ref()
    }
}

impl<'ast> CompilerArgumentPayload<'ast> {
    #[cfg(test)]
    pub(super) fn syntax(
        source: SourceId,
        syntax: SyntaxArgument,
        _fallback: &'ast vela_syntax::ast::Argument,
    ) -> Self {
        Self {
            source: Some(source),
            syntax: Some(syntax),
            _ast: PhantomData,
        }
    }

    #[cfg(test)]
    pub(super) fn missing_value_syntax(
        source: SourceId,
        _fallback: &'ast vela_syntax::ast::Argument,
    ) -> Self {
        Self {
            source: Some(source),
            syntax: None,
            _ast: PhantomData,
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

    pub(in crate::compiler) fn value_expression_payload(
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
        let fallback_kind = match &fallback.kind {
            ExprKind::Literal(_) => CompilerExpressionFallbackKind::Literal,
            ExprKind::Path(_) => CompilerExpressionFallbackKind::Path,
            ExprKind::SelfValue => CompilerExpressionFallbackKind::SelfValue,
            #[cfg(test)]
            ExprKind::Block(block) => CompilerExpressionFallbackKind::Block(block),
            #[cfg(not(test))]
            ExprKind::Block(_) => CompilerExpressionFallbackKind::Block,
            #[cfg(test)]
            ExprKind::If(if_expr) => CompilerExpressionFallbackKind::If(if_expr),
            #[cfg(not(test))]
            ExprKind::If(_) => CompilerExpressionFallbackKind::If,
            #[cfg(test)]
            ExprKind::Match(match_expr) => CompilerExpressionFallbackKind::Match(match_expr),
            #[cfg(not(test))]
            ExprKind::Match(_) => CompilerExpressionFallbackKind::Match,
            #[cfg(test)]
            ExprKind::Assign { target, value, .. } => {
                CompilerExpressionFallbackKind::Assign { target, value }
            }
            #[cfg(not(test))]
            ExprKind::Assign { .. } => CompilerExpressionFallbackKind::Assign,
            #[cfg(test)]
            ExprKind::Unary { expr, .. } => CompilerExpressionFallbackKind::Unary { expr },
            #[cfg(not(test))]
            ExprKind::Unary { .. } => CompilerExpressionFallbackKind::Unary,
            #[cfg(test)]
            ExprKind::Try(expr) => CompilerExpressionFallbackKind::Try(expr),
            #[cfg(not(test))]
            ExprKind::Try(_) => CompilerExpressionFallbackKind::Try,
            #[cfg(test)]
            ExprKind::Binary { op, left, right } => CompilerExpressionFallbackKind::Binary {
                op: *op,
                left,
                right,
            },
            #[cfg(not(test))]
            ExprKind::Binary { .. } => CompilerExpressionFallbackKind::Binary,
            #[cfg(test)]
            ExprKind::Call { callee, args } => {
                CompilerExpressionFallbackKind::Call { callee, args }
            }
            #[cfg(not(test))]
            ExprKind::Call { .. } => CompilerExpressionFallbackKind::Call,
            #[cfg(test)]
            ExprKind::Field { base, .. } => CompilerExpressionFallbackKind::Field { base },
            #[cfg(not(test))]
            ExprKind::Field { .. } => CompilerExpressionFallbackKind::Field,
            #[cfg(test)]
            ExprKind::Index { base, index } => {
                CompilerExpressionFallbackKind::Index { base, index }
            }
            #[cfg(not(test))]
            ExprKind::Index { .. } => CompilerExpressionFallbackKind::Index,
            #[cfg(test)]
            ExprKind::Lambda { body, .. } => CompilerExpressionFallbackKind::Lambda { body },
            #[cfg(not(test))]
            ExprKind::Lambda { .. } => CompilerExpressionFallbackKind::Lambda,
            #[cfg(test)]
            ExprKind::Array(items) => CompilerExpressionFallbackKind::Array(items),
            #[cfg(not(test))]
            ExprKind::Array(_) => CompilerExpressionFallbackKind::Array,
            #[cfg(test)]
            ExprKind::Map(entries) => CompilerExpressionFallbackKind::Map(entries),
            #[cfg(not(test))]
            ExprKind::Map(_) => CompilerExpressionFallbackKind::Map,
            #[cfg(test)]
            ExprKind::Record { fields, .. } => CompilerExpressionFallbackKind::Record { fields },
            #[cfg(not(test))]
            ExprKind::Record { .. } => CompilerExpressionFallbackKind::Record,
            #[cfg(test)]
            ExprKind::InterpolatedString(parts) => {
                CompilerExpressionFallbackKind::InterpolatedString(parts)
            }
            #[cfg(not(test))]
            ExprKind::InterpolatedString(_) => CompilerExpressionFallbackKind::InterpolatedString,
            _ => CompilerExpressionFallbackKind::Other,
        };
        Self {
            source,
            syntax,
            fallback,
            fallback_kind,
        }
    }

    #[cfg(test)]
    pub(in crate::compiler) fn missing_syntax(
        source: SourceId,
        fallback: &'ast vela_syntax::ast::Expr,
    ) -> Self {
        Self::from_fallback(Some(source), None, fallback)
    }

    pub(super) fn aligned_fallback_expr(&self) -> Option<&'ast vela_syntax::ast::Expr> {
        let syntax_kind = self.stored_syntax_kind()?;
        if !self.fallback_kind_matches_syntax_kind(syntax_kind) {
            return None;
        }
        if syntax_kind == SyntaxExpressionKind::Path && !self.fallback_path_shape_matches_syntax() {
            return None;
        }
        let syntax_span = self.syntax_span()?;
        spans_overlap(syntax_span, self.fallback.span).then_some(self.fallback)
    }

    fn fallback_kind_matches_syntax_kind(&self, syntax_kind: SyntaxExpressionKind) -> bool {
        match syntax_kind {
            SyntaxExpressionKind::Literal => {
                #[cfg(test)]
                {
                    matches!(
                        self.fallback_kind,
                        CompilerExpressionFallbackKind::Literal
                            | CompilerExpressionFallbackKind::InterpolatedString(_)
                    )
                }
                #[cfg(not(test))]
                {
                    matches!(
                        self.fallback_kind,
                        CompilerExpressionFallbackKind::Literal
                            | CompilerExpressionFallbackKind::InterpolatedString
                    )
                }
            }
            SyntaxExpressionKind::Path => matches!(
                self.fallback_kind,
                CompilerExpressionFallbackKind::Path | CompilerExpressionFallbackKind::SelfValue
            ),
            SyntaxExpressionKind::Paren => true,
            SyntaxExpressionKind::Unary => {
                #[cfg(test)]
                {
                    matches!(
                        self.fallback_kind,
                        CompilerExpressionFallbackKind::Unary { .. }
                    )
                }
                #[cfg(not(test))]
                {
                    matches!(self.fallback_kind, CompilerExpressionFallbackKind::Unary)
                }
            }
            SyntaxExpressionKind::Binary => {
                #[cfg(test)]
                {
                    matches!(
                        self.fallback_kind,
                        CompilerExpressionFallbackKind::Binary { .. }
                    )
                }
                #[cfg(not(test))]
                {
                    matches!(self.fallback_kind, CompilerExpressionFallbackKind::Binary)
                }
            }
            SyntaxExpressionKind::Assign => {
                #[cfg(test)]
                {
                    matches!(
                        self.fallback_kind,
                        CompilerExpressionFallbackKind::Assign { .. }
                    )
                }
                #[cfg(not(test))]
                {
                    matches!(self.fallback_kind, CompilerExpressionFallbackKind::Assign)
                }
            }
            SyntaxExpressionKind::Field => {
                #[cfg(test)]
                {
                    matches!(
                        self.fallback_kind,
                        CompilerExpressionFallbackKind::Field { .. }
                    )
                }
                #[cfg(not(test))]
                {
                    matches!(self.fallback_kind, CompilerExpressionFallbackKind::Field)
                }
            }
            SyntaxExpressionKind::Call => {
                #[cfg(test)]
                {
                    matches!(
                        self.fallback_kind,
                        CompilerExpressionFallbackKind::Call { .. }
                    )
                }
                #[cfg(not(test))]
                {
                    matches!(self.fallback_kind, CompilerExpressionFallbackKind::Call)
                }
            }
            SyntaxExpressionKind::Index => {
                #[cfg(test)]
                {
                    matches!(
                        self.fallback_kind,
                        CompilerExpressionFallbackKind::Index { .. }
                    )
                }
                #[cfg(not(test))]
                {
                    matches!(self.fallback_kind, CompilerExpressionFallbackKind::Index)
                }
            }
            SyntaxExpressionKind::Try => {
                #[cfg(test)]
                {
                    matches!(self.fallback_kind, CompilerExpressionFallbackKind::Try(_))
                }
                #[cfg(not(test))]
                {
                    matches!(self.fallback_kind, CompilerExpressionFallbackKind::Try)
                }
            }
            SyntaxExpressionKind::Array => {
                #[cfg(test)]
                {
                    matches!(self.fallback_kind, CompilerExpressionFallbackKind::Array(_))
                }
                #[cfg(not(test))]
                {
                    matches!(self.fallback_kind, CompilerExpressionFallbackKind::Array)
                }
            }
            SyntaxExpressionKind::Map => {
                #[cfg(test)]
                {
                    matches!(self.fallback_kind, CompilerExpressionFallbackKind::Map(_))
                }
                #[cfg(not(test))]
                {
                    matches!(self.fallback_kind, CompilerExpressionFallbackKind::Map)
                }
            }
            SyntaxExpressionKind::Record => {
                #[cfg(test)]
                {
                    matches!(
                        self.fallback_kind,
                        CompilerExpressionFallbackKind::Record { .. }
                    )
                }
                #[cfg(not(test))]
                {
                    matches!(self.fallback_kind, CompilerExpressionFallbackKind::Record)
                }
            }
            SyntaxExpressionKind::Lambda => {
                #[cfg(test)]
                {
                    matches!(
                        self.fallback_kind,
                        CompilerExpressionFallbackKind::Lambda { .. }
                    )
                }
                #[cfg(not(test))]
                {
                    matches!(self.fallback_kind, CompilerExpressionFallbackKind::Lambda)
                }
            }
            SyntaxExpressionKind::Block => {
                #[cfg(test)]
                {
                    matches!(self.fallback_kind, CompilerExpressionFallbackKind::Block(_))
                }
                #[cfg(not(test))]
                {
                    matches!(self.fallback_kind, CompilerExpressionFallbackKind::Block)
                }
            }
            SyntaxExpressionKind::If => {
                #[cfg(test)]
                {
                    matches!(self.fallback_kind, CompilerExpressionFallbackKind::If(_))
                }
                #[cfg(not(test))]
                {
                    matches!(self.fallback_kind, CompilerExpressionFallbackKind::If)
                }
            }
            SyntaxExpressionKind::Match => {
                #[cfg(test)]
                {
                    matches!(self.fallback_kind, CompilerExpressionFallbackKind::Match(_))
                }
                #[cfg(not(test))]
                {
                    matches!(self.fallback_kind, CompilerExpressionFallbackKind::Match)
                }
            }
        }
    }

    fn fallback_path_shape_matches_syntax(&self) -> bool {
        match self.fallback_kind {
            CompilerExpressionFallbackKind::Path => !self.syntax_is_self(),
            CompilerExpressionFallbackKind::SelfValue => self.syntax_is_self(),
            _ => false,
        }
    }

    pub(in crate::compiler) fn is_aligned_with_expr(&self, expr: &vela_syntax::ast::Expr) -> bool {
        std::ptr::eq(self.fallback, expr)
            || self
                .syntax_span()
                .is_some_and(|span| spans_overlap(span, expr.span))
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

    pub(in crate::compiler) fn syntax_expression(&self) -> Option<&SyntaxExpression> {
        self.source?;
        self.syntax.as_ref()
    }
}

fn spans_overlap(left: Span, right: Span) -> bool {
    left.start < right.end && right.start < left.end
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
