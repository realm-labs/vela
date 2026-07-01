use vela_common::{SourceId, Span};
use vela_syntax::Parse as SyntaxParse;
use vela_syntax::ast::SyntaxSourceFile;
use vela_syntax::ast::{AstNode, Block, SyntaxBlock};
use vela_syntax::parse_body_blocks_at_spans;

pub(super) struct BodyFallbackSource {
    bodies: Vec<Block>,
}

impl BodyFallbackSource {
    pub(super) fn from_syntax(
        source: SourceId,
        text: &str,
        syntax: &SyntaxParse<SyntaxSourceFile>,
    ) -> Self {
        let required_spans = syntax_body_spans(source, syntax);
        let bodies = parse_body_blocks_at_spans(source, text, &required_spans);
        Self { bodies }
    }

    pub(super) fn body_for_syntax(&self, source: SourceId, body: &SyntaxBlock) -> Option<&Block> {
        self.body_by_span(syntax_body_span(source, body))
    }

    pub(super) fn body_by_span(&self, span: Span) -> Option<&Block> {
        self.bodies.iter().find(|body| body.span == span)
    }
}

fn syntax_body_spans(source: SourceId, syntax: &SyntaxParse<SyntaxSourceFile>) -> Vec<Span> {
    syntax
        .tree()
        .functions()
        .filter_map(|function| function.body())
        .chain(
            syntax
                .tree()
                .impls()
                .flat_map(|item| item.methods().filter_map(|method| method.body())),
        )
        .chain(
            syntax
                .tree()
                .traits()
                .flat_map(|item| item.methods().filter_map(|method| method.body())),
        )
        .map(|body| syntax_body_span(source, &body))
        .collect()
}

fn syntax_body_span(source: SourceId, body: &SyntaxBlock) -> Span {
    let range = body.syntax().text_range();
    let start: u32 = range.start().into();
    let end: u32 = range.end().into();
    Span::new(source, start, end)
}
