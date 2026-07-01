use vela_common::{SourceId, Span};
use vela_syntax::ast::{AstNode, Block, ItemKind, SourceFile, SyntaxBlock};
use vela_syntax::parser::parse_source as parse_body_fallback_source;

pub(super) struct BodyFallbackSource {
    parsed: SourceFile,
}

impl BodyFallbackSource {
    pub(super) fn parse(source: SourceId, text: &str) -> Self {
        Self {
            parsed: parse_body_fallback_source(source, text),
        }
    }

    pub(super) fn body_for_syntax(&self, source: SourceId, body: &SyntaxBlock) -> Option<&Block> {
        self.body_by_span(syntax_body_span(source, body))
    }

    pub(super) fn body_by_span(&self, span: Span) -> Option<&Block> {
        for item in &self.parsed.items {
            if let ItemKind::Function(function) = &item.kind
                && function.body.span == span
            {
                return Some(&function.body);
            }
            if let ItemKind::Impl(item) = &item.kind {
                for method in &item.methods {
                    if method.function.body.span == span {
                        return Some(&method.function.body);
                    }
                }
            }
            if let ItemKind::Trait(item) = &item.kind {
                for method in &item.methods {
                    let Some(body) = &method.default_body else {
                        continue;
                    };
                    if body.span == span {
                        return Some(body);
                    }
                }
            }
        }
        None
    }
}

fn syntax_body_span(source: SourceId, body: &SyntaxBlock) -> Span {
    let range = body.syntax().text_range();
    let start: u32 = range.start().into();
    let end: u32 = range.end().into();
    Span::new(source, start, end)
}
