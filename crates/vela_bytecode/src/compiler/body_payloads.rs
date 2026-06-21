use vela_common::SourceId;
use vela_syntax::ast::{Block, SyntaxBlock};

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
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "CST body payload is consumed by the upcoming body lowering migration"
        )
    )]
    pub(super) body: SyntaxBlock,
}

#[derive(Clone)]
pub(super) struct CompilerBodyPayload<'ast> {
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "CST body payload is consumed by the upcoming body lowering migration"
        )
    )]
    syntax: Option<SyntaxBodyPayload>,
    fallback: &'ast Block,
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

    #[cfg(test)]
    pub(super) fn syntax_payload(&self) -> Option<&SyntaxBodyPayload> {
        self.syntax.as_ref()
    }
}
