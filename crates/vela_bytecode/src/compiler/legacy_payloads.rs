use vela_common::{SourceId, Span};
use vela_syntax::ast::{AstNode, Block, ItemKind, SourceFile, SyntaxBlock};
use vela_syntax::parser::parse_source as parse_legacy_source;

use super::body_payloads::CompilerBodyPayload;

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

pub(super) struct LegacyFunctionBodyPayload<'ast> {
    pub(super) name: String,
    pub(super) body: CompilerBodyPayload<'ast>,
}

pub(super) fn function_body_payload<'ast>(
    source: SourceId,
    legacy: &'ast LegacySourceFallback,
    name: &str,
    syntax_body: SyntaxBlock,
) -> Option<LegacyFunctionBodyPayload<'ast>> {
    let legacy_body = legacy.body_by_span(syntax_body_span(source, &syntax_body))?;
    Some(LegacyFunctionBodyPayload {
        name: name.to_owned(),
        body: CompilerBodyPayload::syntax(source, syntax_body, legacy_body),
    })
}

fn syntax_body_span(source: SourceId, body: &SyntaxBlock) -> Span {
    let range = body.syntax().text_range();
    let start: u32 = range.start().into();
    let end: u32 = range.end().into();
    Span::new(source, start, end)
}
