use vela_common::SourceId;
use vela_common::Span;
use vela_syntax::ast::{
    AstNode, Block, Stmt, SyntaxBlock, SyntaxExpression, SyntaxExpressionKind, SyntaxStatement,
    SyntaxStatementKind,
};

#[derive(Clone)]
pub(super) struct SyntaxBodyPayload {
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "CST body payload is consumed by the upcoming body lowering migration"
        )
    )]
    pub(super) source: SourceId,
    pub(super) body: SyntaxBlock,
}

#[derive(Clone)]
pub(super) struct CompilerBodyPayload<'ast> {
    syntax: Option<SyntaxBodyPayload>,
    fallback: &'ast Block,
}

pub(super) struct CompilerStatementPayload<'ast> {
    syntax: Option<SyntaxStatement>,
    fallback: &'ast Stmt,
}

impl<'ast> CompilerBodyPayload<'ast> {
    pub(super) fn syntax(source: SourceId, body: SyntaxBlock, fallback: &'ast Block) -> Self {
        Self {
            syntax: Some(SyntaxBodyPayload { source, body }),
            fallback,
        }
    }

    pub(super) fn legacy(fallback: &'ast Block) -> Self {
        Self {
            syntax: None,
            fallback,
        }
    }

    pub(super) fn fallback(&self) -> &'ast Block {
        self.fallback
    }

    pub(super) fn statement_payloads(&self) -> Vec<CompilerStatementPayload<'ast>> {
        let syntax_statements = self
            .syntax
            .as_ref()
            .map(|payload| payload.body.statements().collect::<Vec<_>>());

        self.fallback
            .statements
            .iter()
            .enumerate()
            .map(|(index, fallback)| CompilerStatementPayload {
                syntax: syntax_statements.as_ref().and_then(|statements| {
                    syntax_statement_for_fallback(statements, index, fallback)
                }),
                fallback,
            })
            .collect()
    }

    #[cfg(test)]
    pub(super) fn syntax_payload(&self) -> Option<&SyntaxBodyPayload> {
        self.syntax.as_ref()
    }
}

fn syntax_statement_for_fallback(
    statements: &[SyntaxStatement],
    fallback_index: usize,
    fallback: &Stmt,
) -> Option<SyntaxStatement> {
    statements
        .iter()
        .find(|statement| syntax_statement_matches_span(statement, fallback.span))
        .cloned()
        .or_else(|| statements.get(fallback_index).cloned())
}

fn syntax_statement_matches_span(statement: &SyntaxStatement, span: Span) -> bool {
    let range = statement.syntax().text_range();
    u32::from(range.start()) == span.start && u32::from(range.end()) == span.end
}

impl<'ast> CompilerStatementPayload<'ast> {
    pub(super) fn fallback(&self) -> &'ast Stmt {
        self.fallback
    }

    pub(super) fn statement_kind(&self) -> Option<SyntaxStatementKind> {
        self.syntax.as_ref().map(SyntaxStatement::statement_kind)
    }

    pub(super) fn expression_kind(&self) -> Option<SyntaxExpressionKind> {
        self.expression()
            .map(|expression| expression.expression_kind())
    }

    fn expression(&self) -> Option<SyntaxExpression> {
        self.syntax.as_ref()?.as_expr()?.expression()
    }

    #[cfg(test)]
    pub(super) fn syntax_statement(&self) -> Option<&SyntaxStatement> {
        self.syntax.as_ref()
    }
}
