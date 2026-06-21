use std::collections::HashMap;

use vela_common::{SourceId, Span};
use vela_hir::type_hint::FunctionSignature;
use vela_syntax::Parse as SyntaxParse;
use vela_syntax::ast::{
    AstNode, Block, Expr, FunctionItem, ItemKind, Param, SourceFile, SyntaxBlock, SyntaxSourceFile,
};
use vela_syntax::parser::parse_source as parse_legacy_source;

use super::body_payloads::CompilerBodyPayload;
use super::param_defaults::{ParamDefaultValue, syntax_param_default_values};

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
                        param_defaults: legacy_param_defaults(&method.function.params),
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
                methods.insert(
                    body.span,
                    LegacyMethodFallback {
                        param_defaults: legacy_param_defaults(&method.params),
                        body,
                    },
                );
            }
        }
        methods
    }
}

pub(super) struct LegacyMethodFallback<'ast> {
    pub(super) param_defaults: Vec<Option<&'ast Expr>>,
    pub(super) body: &'ast Block,
}

pub(super) struct FunctionBodyPayload<'ast> {
    pub(super) name: String,
    pub(super) body: CompilerBodyPayload<'ast>,
    pub(super) param_defaults: Vec<Option<ParamDefaultValue<'ast>>>,
}

pub(super) fn function_body_payload<'ast>(
    source: SourceId,
    syntax: &SyntaxParse<SyntaxSourceFile>,
    legacy: &'ast LegacySourceFallback,
    name: &str,
    signature: &FunctionSignature,
) -> Option<FunctionBodyPayload<'ast>> {
    let syntax_function = syntax
        .tree()
        .functions()
        .find(|function| function.name_text().as_deref() == Some(name))?;
    let syntax_body = syntax_function.body()?;
    let function = legacy_function_body(&legacy.parsed, syntax_body_span(source, &syntax_body))?;
    let legacy_defaults = legacy_param_defaults(&function.params);
    let param_defaults = syntax_param_default_values(
        source,
        syntax_function.param_list(),
        &legacy_defaults,
        signature.params.len(),
    );
    Some(FunctionBodyPayload {
        name: name.to_owned(),
        body: CompilerBodyPayload::syntax(source, syntax_body, &function.body),
        param_defaults,
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

fn legacy_param_defaults(params: &[Param]) -> Vec<Option<&Expr>> {
    params
        .iter()
        .map(|param| param.default_value.as_ref())
        .collect()
}
