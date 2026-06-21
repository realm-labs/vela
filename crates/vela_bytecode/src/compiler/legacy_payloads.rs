use std::collections::HashMap;

use vela_common::{SourceId, Span};
use vela_syntax::ast::{AstNode, Block, FunctionItem, ItemKind, SourceFile, SyntaxBlock};
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

    pub(super) fn impl_methods_by_body_span(&self) -> HashMap<Span, LegacyMethodFallback<'_>> {
        let mut methods = HashMap::new();
        for item in &self.parsed.items {
            let ItemKind::Impl(item) = &item.kind else {
                continue;
            };
            for method in &item.methods {
                methods.insert(
                    method.function.body.span,
                    LegacyMethodFallback {
                        body: &method.function.body,
                    },
                );
            }
        }
        methods
    }

    pub(super) fn trait_default_methods_by_body_span(
        &self,
    ) -> HashMap<Span, LegacyMethodFallback<'_>> {
        let mut methods = HashMap::new();
        for item in &self.parsed.items {
            let ItemKind::Trait(item) = &item.kind else {
                continue;
            };
            for method in &item.methods {
                let Some(body) = &method.default_body else {
                    continue;
                };
                methods.insert(body.span, LegacyMethodFallback { body });
            }
        }
        methods
    }
}

pub(super) struct LegacyMethodFallback<'ast> {
    pub(super) body: &'ast Block,
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
    let function = legacy_function_body(&legacy.parsed, syntax_body_span(source, &syntax_body))?;
    Some(LegacyFunctionBodyPayload {
        name: name.to_owned(),
        body: CompilerBodyPayload::syntax(source, syntax_body, &function.body),
    })
}

fn legacy_function_body(parsed: &SourceFile, body_span: Span) -> Option<&FunctionItem> {
    parsed.items.iter().find_map(|item| match &item.kind {
        ItemKind::Function(function) if function.body.span == body_span => Some(function),
        _ => None,
    })
}

fn syntax_body_span(source: SourceId, body: &SyntaxBlock) -> Span {
    let range = body.syntax().text_range();
    let start: u32 = range.start().into();
    let end: u32 = range.end().into();
    Span::new(source, start, end)
}
