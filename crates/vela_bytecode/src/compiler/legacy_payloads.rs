use vela_common::{SourceId, Span};
use vela_syntax::ast::{Block, ItemKind, SourceFile};
use vela_syntax::parser::parse_source as parse_legacy_source;

pub(super) struct LegacySourceFallback {
    parsed: SourceFile,
}

impl LegacySourceFallback {
    pub(super) fn parse(source: SourceId, text: &str) -> Self {
        Self {
            parsed: parse_legacy_source(source, text),
        }
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
