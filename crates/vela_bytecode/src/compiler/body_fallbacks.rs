use vela_common::{SourceId, Span};
use vela_syntax::ast::{AstNode, Block, SyntaxBlock};
use vela_syntax::parser::parse_source as parse_body_fallback_source;

pub(super) struct BodyFallbackSource {
    bodies: Vec<Block>,
}

impl BodyFallbackSource {
    pub(super) fn parse(source: SourceId, text: &str) -> Self {
        let parsed = parse_body_fallback_source(source, text);
        let mut bodies = Vec::new();
        for item in parsed.items {
            match item.kind {
                vela_syntax::ast::ItemKind::Function(function) => {
                    bodies.push(function.body);
                }
                vela_syntax::ast::ItemKind::Impl(item) => {
                    bodies.extend(item.methods.into_iter().map(|method| method.function.body));
                }
                vela_syntax::ast::ItemKind::Trait(item) => {
                    bodies.extend(
                        item.methods
                            .into_iter()
                            .filter_map(|method| method.default_body),
                    );
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

fn syntax_body_span(source: SourceId, body: &SyntaxBlock) -> Span {
    let range = body.syntax().text_range();
    let start: u32 = range.start().into();
    let end: u32 = range.end().into();
    Span::new(source, start, end)
}
