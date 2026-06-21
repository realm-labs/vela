use vela_common::SourceId;
use vela_syntax::ast::{Block, Stmt, SyntaxBlock, SyntaxStatement};

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
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "CST statement payload is consumed by the upcoming statement lowering migration"
        )
    )]
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
                syntax: syntax_statements
                    .as_ref()
                    .and_then(|statements| statements.get(index).cloned()),
                fallback,
            })
            .collect()
    }

    #[cfg(test)]
    pub(super) fn syntax_payload(&self) -> Option<&SyntaxBodyPayload> {
        self.syntax.as_ref()
    }
}

impl<'ast> CompilerStatementPayload<'ast> {
    pub(super) fn fallback(&self) -> &'ast Stmt {
        self.fallback
    }

    #[cfg(test)]
    pub(super) fn syntax_statement(&self) -> Option<&SyntaxStatement> {
        self.syntax.as_ref()
    }
}
