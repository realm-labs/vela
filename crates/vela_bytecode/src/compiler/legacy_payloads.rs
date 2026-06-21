use vela_common::{SourceId, Span};
use vela_hir::type_hint::FunctionSignature;
use vela_syntax::Parse as SyntaxParse;
use vela_syntax::ast::{
    AstNode, FunctionItem, ItemKind, SourceFile, SyntaxBlock, SyntaxSourceFile,
};

use super::body_payloads::CompilerBodyPayload;
use super::param_defaults::{ParamDefaultValue, syntax_param_default_values};

pub(super) struct FunctionBodyPayload<'ast> {
    pub(super) name: String,
    pub(super) body: CompilerBodyPayload<'ast>,
    pub(super) param_defaults: Vec<Option<ParamDefaultValue<'ast>>>,
}

pub(super) fn function_body_payload<'ast>(
    source: SourceId,
    syntax: &SyntaxParse<SyntaxSourceFile>,
    parsed: &'ast SourceFile,
    name: &str,
    signature: &FunctionSignature,
) -> Option<FunctionBodyPayload<'ast>> {
    let syntax_function = syntax
        .tree()
        .functions()
        .find(|function| function.name_text().as_deref() == Some(name))?;
    let syntax_body = syntax_function.body()?;
    let function = legacy_function_body(parsed, syntax_body_span(source, &syntax_body))?;
    let param_defaults = syntax_param_default_values(
        source,
        syntax_function.param_list(),
        &function.params,
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
