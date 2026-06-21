use vela_common::SourceId;
use vela_hir::type_hint::FunctionSignature;
use vela_syntax::Parse as SyntaxParse;
use vela_syntax::ast::{FunctionItem, ItemKind, SourceFile, SyntaxSourceFile};

use super::param_defaults::{ParamDefaultValue, syntax_param_default_values};

pub(super) struct FunctionBodyPayload<'ast> {
    pub(super) function: &'ast FunctionItem,
    pub(super) param_defaults: Vec<Option<ParamDefaultValue>>,
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
    let function = legacy_function_body(parsed, name)?;
    let param_defaults = syntax_param_default_values(
        source,
        syntax_function.param_list(),
        &function.params,
        signature.params.len(),
    );
    Some(FunctionBodyPayload {
        function,
        param_defaults,
    })
}

fn legacy_function_body<'ast>(parsed: &'ast SourceFile, name: &str) -> Option<&'ast FunctionItem> {
    parsed.items.iter().find_map(|item| match &item.kind {
        ItemKind::Function(function) if function.name == name => Some(function),
        _ => None,
    })
}
