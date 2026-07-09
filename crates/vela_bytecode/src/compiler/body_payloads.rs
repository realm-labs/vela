use std::marker::PhantomData;

use vela_common::{SourceId, Span};
use vela_hir::body::{HirBody, HirBodyRoot, HirStmt, HirStmtKind};
use vela_syntax::ast::{
    AstNode, SyntaxBlock, SyntaxExpression, SyntaxExpressionKind, SyntaxForStmt, SyntaxIfExpr,
    SyntaxMatchExpr, SyntaxPattern, SyntaxStatement, SyntaxStatementKind,
};

use crate::compiler::{CompileError, CompileErrorKind, CompileResult};

mod simple_values;

// Temporary 1200-line exception: this module owns the syntax body payload boundary.
// It is actively shrinking as the hard switch deletes the old payload pairing
// code before the module is split by syntax child payload responsibility.

pub(super) use simple_values::{
    expression_syntax_literal, expression_syntax_negated_number_literal,
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
    hir_body: Option<&'ast HirBody>,
    _ast: PhantomData<&'ast ()>,
}

pub(super) struct CompilerStatementPayload<'ast> {
    source: Option<SourceId>,
    syntax: Option<SyntaxStatement>,
    hir_kind: Option<HirStmtKind>,
    _ast: PhantomData<&'ast ()>,
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
    pub(super) fn nested_syntax(source: SourceId, body: SyntaxBlock) -> Self {
        Self {
            syntax: SyntaxBodyPayload { source, body },
            hir_body: None,
            _ast: PhantomData,
        }
    }

    pub(super) fn hir_body(source: SourceId, body: SyntaxBlock, hir_body: &'ast HirBody) -> Self {
        Self {
            syntax: SyntaxBodyPayload { source, body },
            hir_body: Some(hir_body),
            _ast: PhantomData,
        }
    }

    pub(super) fn statement_payloads(&self) -> CompileResult<Vec<CompilerStatementPayload<'ast>>> {
        if let Some(hir_body) = self.hir_body {
            return hir_body_statements(hir_body, self.syntax.source, &self.syntax.body);
        }
        Ok(syntax_body_statements(&self.syntax.body)
            .into_iter()
            .map(|syntax| CompilerStatementPayload::new_syntax(self.syntax.source, syntax))
            .collect())
    }

    pub(super) fn syntax_statements_are_empty(&self) -> bool {
        if let Some(hir_body) = self.hir_body
            && let Some(statement_ids) = hir_body_root_statement_ids(hir_body)
        {
            return statement_ids.is_empty();
        }
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
}

fn hir_body_statements<'ast>(
    hir_body: &'ast HirBody,
    source: SourceId,
    body: &SyntaxBlock,
) -> CompileResult<Vec<CompilerStatementPayload<'ast>>> {
    let syntax_statements = syntax_body_statements(body);
    let statement_ids = hir_body_root_statement_ids(hir_body).ok_or_else(|| {
        CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "HIR function body root block",
        ))
    })?;
    statement_ids
        .iter()
        .map(|statement| {
            let statement = hir_body.statements.get(statement).ok_or_else(|| {
                CompileError::new(CompileErrorKind::UnsupportedSyntax("HIR statement"))
            })?;
            let syntax = syntax_statement_for_hir(source, statement, &syntax_statements)
                .ok_or_else(|| {
                    CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "HIR statement source origin",
                    ))
                    .with_span(statement.origin.span)
                })?;
            Ok(CompilerStatementPayload::new_hir_syntax(
                source, syntax, statement,
            ))
        })
        .collect()
}

fn hir_body_root_statement_ids(hir_body: &HirBody) -> Option<&[vela_hir::ids::HirStmtId]> {
    let HirBodyRoot::Block(block) = hir_body.root else {
        return None;
    };
    Some(hir_body.blocks.get(&block)?.statements.as_slice())
}

fn syntax_statement_for_hir(
    source: SourceId,
    statement: &HirStmt,
    syntax_statements: &[SyntaxStatement],
) -> Option<SyntaxStatement> {
    syntax_statements
        .iter()
        .find(|syntax| syntax_statement_span(source, syntax) == statement.origin.span)
        .cloned()
}

fn syntax_statement_span(source: SourceId, statement: &SyntaxStatement) -> Span {
    let range = statement.syntax().text_range();
    Span::new(source, u32::from(range.start()), u32::from(range.end()))
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

impl<'ast> CompilerStatementPayload<'ast> {
    fn new_syntax(source: SourceId, syntax: SyntaxStatement) -> Self {
        Self {
            source: Some(source),
            syntax: Some(syntax),
            hir_kind: None,
            _ast: PhantomData,
        }
    }

    fn new_hir_syntax(source: SourceId, syntax: SyntaxStatement, statement: &HirStmt) -> Self {
        Self {
            source: Some(source),
            syntax: Some(syntax),
            hir_kind: Some(statement.kind),
            _ast: PhantomData,
        }
    }

    pub(super) fn stored_statement_kind(&self) -> Option<SyntaxStatementKind> {
        self.hir_kind
            .and_then(hir_statement_kind_to_syntax)
            .or_else(|| self.syntax.as_ref().map(SyntaxStatement::statement_kind))
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

    pub(super) fn let_pattern_initializer_syntax_expression_and_span(
        &self,
    ) -> Option<(SourceId, SyntaxPattern, SyntaxExpression, Span)> {
        let source = self.source?;
        let statement = self.syntax.as_ref()?.as_let()?;
        let pattern = statement.pattern()?;
        let expression = statement.initializer()?;
        let range = statement.syntax().text_range();
        let span = Span::new(source, range.start().into(), range.end().into());
        Some((source, pattern, expression, span))
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

    pub(in crate::compiler) fn let_initializer_syntax_expression_and_span(
        &self,
    ) -> Option<(SourceId, SyntaxExpression, Span)> {
        let source = self.source?;
        let expression = self.syntax.as_ref()?.as_let()?.initializer()?;
        let range = expression.syntax().text_range();
        let span = Span::new(source, range.start().into(), range.end().into());
        Some((source, expression, span))
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
}

fn hir_statement_kind_to_syntax(kind: HirStmtKind) -> Option<SyntaxStatementKind> {
    match kind {
        HirStmtKind::Let => Some(SyntaxStatementKind::Let),
        HirStmtKind::Return => Some(SyntaxStatementKind::Return),
        HirStmtKind::Break => Some(SyntaxStatementKind::Break),
        HirStmtKind::Continue => Some(SyntaxStatementKind::Continue),
        HirStmtKind::For => Some(SyntaxStatementKind::For),
        HirStmtKind::If => Some(SyntaxStatementKind::If),
        HirStmtKind::Match => Some(SyntaxStatementKind::Match),
        HirStmtKind::Block => Some(SyntaxStatementKind::Block),
        HirStmtKind::Expr => Some(SyntaxStatementKind::Expr),
        HirStmtKind::Unknown => None,
    }
}
