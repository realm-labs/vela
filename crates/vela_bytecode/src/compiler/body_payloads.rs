use vela_common::{SourceId, Span};
use vela_hir::body::{HirBody, HirBodyRoot, HirStmt, HirStmtKind};
use vela_hir::ids::{HirBlockId, HirPatternId};
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
    hir_body: &'ast HirBody,
    hir_block: Option<HirBlockId>,
}

pub(super) struct CompilerStatementPayload {
    source: SourceId,
    syntax: SyntaxStatement,
    hir_kind: HirStmtKind,
    patterns: Vec<HirPatternId>,
    span: Span,
}

pub(super) enum CompilerBlockValue<'payload> {
    Empty,
    TailExpression {
        prefix: &'payload [CompilerStatementPayload],
        tail: &'payload CompilerStatementPayload,
    },
    Statements(&'payload [CompilerStatementPayload]),
}

impl<'ast> CompilerBodyPayload<'ast> {
    pub(super) fn hir_body(source: SourceId, body: SyntaxBlock, hir_body: &'ast HirBody) -> Self {
        Self {
            syntax: SyntaxBodyPayload { source, body },
            hir_body,
            hir_block: None,
        }
    }

    pub(super) fn hir_block(
        source: SourceId,
        body: SyntaxBlock,
        hir_bodies: &[&'ast HirBody],
    ) -> CompileResult<Self> {
        let span = syntax_block_span(source, &body);
        let (hir_body, hir_block) = hir_bodies
            .iter()
            .find_map(|hir_body| {
                hir_body
                    .blocks
                    .values()
                    .find(|block| block.origin.span == span)
                    .map(|block| (*hir_body, block.id))
            })
            .ok_or_else(|| {
                CompileError::new(CompileErrorKind::UnsupportedSyntax(
                    "HIR block source origin",
                ))
                .with_span(span)
            })?;
        Ok(Self {
            syntax: SyntaxBodyPayload { source, body },
            hir_body,
            hir_block: Some(hir_block),
        })
    }

    pub(super) fn statement_payloads(&self) -> CompileResult<Vec<CompilerStatementPayload>> {
        hir_body_statements(
            self.hir_body,
            self.hir_block,
            self.syntax.source,
            &self.syntax.body,
        )
    }

    pub(super) fn syntax_statements_are_empty(&self) -> bool {
        hir_body_statement_ids(self.hir_body, self.hir_block).is_none_or(<[_]>::is_empty)
    }

    pub(super) fn block_value<'payload>(
        &self,
        statements: &'payload [CompilerStatementPayload],
    ) -> CompilerBlockValue<'payload> {
        let Some((tail, prefix)) = statements.split_last() else {
            return CompilerBlockValue::Empty;
        };
        if matches!(
            tail.hir_statement_kind(),
            HirStmtKind::Expr | HirStmtKind::If | HirStmtKind::Match
        ) {
            CompilerBlockValue::TailExpression { prefix, tail }
        } else {
            CompilerBlockValue::Statements(statements)
        }
    }
}

fn hir_body_statements(
    hir_body: &HirBody,
    hir_block: Option<HirBlockId>,
    source: SourceId,
    body: &SyntaxBlock,
) -> CompileResult<Vec<CompilerStatementPayload>> {
    let syntax_statements = syntax_body_statements(body);
    let statement_ids = hir_body_statement_ids(hir_body, hir_block)
        .ok_or_else(|| CompileError::new(CompileErrorKind::UnsupportedSyntax("HIR block")))?;
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

fn hir_body_statement_ids(
    hir_body: &HirBody,
    hir_block: Option<HirBlockId>,
) -> Option<&[vela_hir::ids::HirStmtId]> {
    if let Some(block) = hir_block {
        return Some(hir_body.blocks.get(&block)?.statements.as_slice());
    }
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

fn syntax_block_span(source: SourceId, block: &SyntaxBlock) -> Span {
    let range = block.syntax().text_range();
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

impl CompilerStatementPayload {
    fn new_hir_syntax(source: SourceId, syntax: SyntaxStatement, statement: &HirStmt) -> Self {
        Self {
            source,
            syntax,
            hir_kind: statement.kind,
            patterns: statement.patterns.clone(),
            span: statement.origin.span,
        }
    }

    pub(super) fn hir_statement_kind(&self) -> HirStmtKind {
        self.hir_kind
    }

    pub(super) fn hir_patterns(&self) -> &[HirPatternId] {
        &self.patterns
    }

    pub(super) fn stored_expression_kind(&self) -> Option<SyntaxExpressionKind> {
        self.expression()
            .map(|expression| expression.expression_kind())
    }

    pub(in crate::compiler) fn expression_statement_syntax_expression(
        &self,
    ) -> Option<(SourceId, SyntaxExpression)> {
        Some((self.source, self.expression()?))
    }

    pub(super) fn stored_let_initializer_kind(&self) -> Option<SyntaxExpressionKind> {
        self.syntax
            .as_let()?
            .initializer()
            .map(|expression| expression.expression_kind())
    }

    pub(super) fn let_initializer_missing_in_syntax(&self) -> bool {
        self.syntax
            .as_let()
            .is_some_and(|statement| statement.initializer().is_none())
    }

    pub(super) fn let_name_text(&self) -> Option<String> {
        self.syntax.as_let()?.name_text()
    }

    pub(super) fn let_pattern_initializer_syntax_expression_and_span(
        &self,
    ) -> Option<(SourceId, SyntaxPattern, SyntaxExpression, Span)> {
        let source = self.source;
        let statement = self.syntax.as_let()?;
        let pattern = statement.pattern()?;
        let expression = statement.initializer()?;
        Some((source, pattern, expression, self.span))
    }

    pub(in crate::compiler) fn let_initializer_syntax_literal_and_span(
        &self,
    ) -> Option<(vela_syntax::ast::Literal, Span)> {
        let source = self.source;
        let expression = self.syntax.as_let()?.initializer()?;
        let range = expression.syntax().text_range();
        let span = Span::new(source, range.start().into(), range.end().into());
        let literal = expression_syntax_literal(&expression)?;
        Some((literal, span))
    }

    pub(in crate::compiler) fn let_initializer_syntax_negated_literal_and_span(
        &self,
    ) -> Option<(vela_syntax::ast::Literal, Span)> {
        let source = self.source;
        let expression = self.syntax.as_let()?.initializer()?;
        let range = expression.syntax().text_range();
        let span = Span::new(source, range.start().into(), range.end().into());
        let literal = expression_syntax_negated_number_literal(&expression)?;
        Some((literal, span))
    }

    pub(in crate::compiler) fn let_initializer_syntax_expression_and_span(
        &self,
    ) -> Option<(SourceId, SyntaxExpression, Span)> {
        let source = self.source;
        let expression = self.syntax.as_let()?.initializer()?;
        let range = expression.syntax().text_range();
        let span = Span::new(source, range.start().into(), range.end().into());
        Some((source, expression, span))
    }

    pub(super) fn stored_return_value_kind(&self) -> Option<SyntaxExpressionKind> {
        self.syntax
            .as_return()?
            .expression()
            .map(|expression| expression.expression_kind())
    }

    pub(in crate::compiler) fn syntax_if(&self) -> Option<(SourceId, SyntaxIfExpr)> {
        Some((self.source, self.syntax.as_if()?))
    }

    pub(in crate::compiler) fn syntax_for(&self) -> Option<(SourceId, SyntaxForStmt)> {
        Some((self.source, self.syntax.as_for()?))
    }

    pub(in crate::compiler) fn syntax_match(&self) -> Option<(SourceId, SyntaxMatchExpr)> {
        Some((self.source, self.expression()?.as_match()?))
    }

    pub(in crate::compiler) fn return_value_syntax_literal_and_span(
        &self,
    ) -> Option<(vela_syntax::ast::Literal, Span)> {
        let source = self.source;
        let expression = self.syntax.as_return()?.expression()?;
        let range = expression.syntax().text_range();
        let span = Span::new(source, range.start().into(), range.end().into());
        let literal = expression_syntax_literal(&expression)?;
        Some((literal, span))
    }

    pub(in crate::compiler) fn return_value_syntax_negated_literal_and_span(
        &self,
    ) -> Option<(vela_syntax::ast::Literal, Span)> {
        let source = self.source;
        let expression = self.syntax.as_return()?.expression()?;
        let range = expression.syntax().text_range();
        let span = Span::new(source, range.start().into(), range.end().into());
        let literal = expression_syntax_negated_number_literal(&expression)?;
        Some((literal, span))
    }

    pub(in crate::compiler) fn return_value_syntax_expression_and_span(
        &self,
    ) -> Option<(SourceId, SyntaxExpression, Span)> {
        let source = self.source;
        let expression = self.syntax.as_return()?.expression()?;
        let range = expression.syntax().text_range();
        let span = Span::new(source, range.start().into(), range.end().into());
        Some((source, expression, span))
    }

    pub(super) fn statement_span(&self) -> Span {
        self.span
    }

    pub(super) fn return_value_missing_in_syntax(&self) -> bool {
        self.syntax
            .as_return()
            .is_some_and(|statement| statement.expression().is_none())
    }

    pub(super) fn block_syntax_body(&self) -> Option<(SourceId, SyntaxBlock)> {
        Some((self.source, self.syntax.as_block()?))
    }

    fn expression(&self) -> Option<SyntaxExpression> {
        let syntax = &self.syntax;
        syntax
            .as_expr()
            .and_then(|stmt| stmt.expression())
            .or_else(|| SyntaxExpression::cast(syntax.syntax().clone()))
    }
}
