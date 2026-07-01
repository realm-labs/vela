use std::collections::HashSet;

use vela_common::{SourceId, Span};
use vela_syntax::Parse as SyntaxParse;
use vela_syntax::ast::SyntaxSourceFile;
use vela_syntax::ast::{AstNode, Block, SyntaxBlock};
use vela_syntax::parser::parse_source as parse_body_fallback_source;

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
        let parsed = parse_body_fallback_source(source, text);
        let mut bodies = Vec::new();
        for item in parsed.items {
            match item.kind {
                vela_syntax::ast::ItemKind::Function(function) => {
                    push_required_body(&mut bodies, &required_spans, function.body);
                }
                vela_syntax::ast::ItemKind::Impl(item) => {
                    for method in item.methods {
                        push_required_body(&mut bodies, &required_spans, method.function.body);
                    }
                }
                vela_syntax::ast::ItemKind::Trait(item) => {
                    for body in item
                        .methods
                        .into_iter()
                        .filter_map(|method| method.default_body)
                    {
                        push_required_body(&mut bodies, &required_spans, body);
                    }
                }
                vela_syntax::ast::ItemKind::Use(_)
                | vela_syntax::ast::ItemKind::Const(_)
                | vela_syntax::ast::ItemKind::Global(_)
                | vela_syntax::ast::ItemKind::Struct(_)
                | vela_syntax::ast::ItemKind::Enum(_) => {}
            }
        }
        Self { bodies }
    }

    pub(super) fn body_for_syntax(&self, source: SourceId, body: &SyntaxBlock) -> Option<&Block> {
        self.body_by_span(syntax_body_span(source, body))
    }

    pub(super) fn body_by_span(&self, span: Span) -> Option<&Block> {
        self.bodies.iter().find(|body| body.span == span)
    }
}

fn syntax_body_spans(source: SourceId, syntax: &SyntaxParse<SyntaxSourceFile>) -> HashSet<Span> {
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

fn push_required_body(bodies: &mut Vec<Block>, required_spans: &HashSet<Span>, body: Block) {
    if required_spans.contains(&body.span) {
        bodies.push(body);
    }
}

fn syntax_body_span(source: SourceId, body: &SyntaxBlock) -> Span {
    let range = body.syntax().text_range();
    let start: u32 = range.start().into();
    let end: u32 = range.end().into();
    Span::new(source, start, end)
}
