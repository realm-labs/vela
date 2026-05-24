use std::collections::BTreeMap;

use vela_hir::{
    BindingMap, DeclarationKind, FunctionSignature, ImplMetadata, ModuleGraph, ModuleId, ModulePath,
};
use vela_syntax::{FunctionItem, ImplItem, ItemKind, SourceFile};

pub(super) struct ScriptImplMethod<'ast> {
    pub(super) target_type: String,
    pub(super) method_name: String,
    pub(super) symbol: String,
    pub(super) function: &'ast FunctionItem,
    pub(super) signature: &'ast FunctionSignature,
    pub(super) bindings: &'ast BindingMap,
}

pub(super) fn source_methods<'ast>(
    parsed: &'ast SourceFile,
    graph: &'ast ModuleGraph,
    module: ModuleId,
) -> Vec<ScriptImplMethod<'ast>> {
    graph
        .declarations()
        .filter(|declaration| declaration.module == module)
        .filter(|declaration| declaration.kind == DeclarationKind::Impl)
        .flat_map(|declaration| {
            let module_path = graph.module_path(declaration.module);
            let Some(impl_metadata) = graph.impl_metadata(declaration.id) else {
                return Vec::new();
            };
            let Some(item) = syntax_impl_item(parsed, impl_metadata) else {
                return Vec::new();
            };
            let target_type = local_target_name(&impl_metadata.target_path);
            collect_methods(graph, module_path, impl_metadata, item, target_type)
        })
        .collect()
}

pub(super) fn module_methods<'ast>(
    parsed: &'ast BTreeMap<ModuleId, SourceFile>,
    graph: &'ast ModuleGraph,
) -> Vec<ScriptImplMethod<'ast>> {
    graph
        .declarations()
        .filter(|declaration| declaration.kind == DeclarationKind::Impl)
        .flat_map(|declaration| {
            let module_path = graph.module_path(declaration.module);
            let Some(impl_metadata) = graph.impl_metadata(declaration.id) else {
                return Vec::new();
            };
            let Some(source) = parsed.get(&declaration.module) else {
                return Vec::new();
            };
            let Some(item) = syntax_impl_item(source, impl_metadata) else {
                return Vec::new();
            };
            let target_type = module_target_name(module_path, &impl_metadata.target_path);
            collect_methods(graph, module_path, impl_metadata, item, target_type)
        })
        .collect()
}

fn collect_methods<'ast>(
    graph: &'ast ModuleGraph,
    module_path: Option<&'ast ModulePath>,
    impl_metadata: &'ast ImplMetadata,
    item: &'ast ImplItem,
    target_type: String,
) -> Vec<ScriptImplMethod<'ast>> {
    item.methods
        .iter()
        .zip(&impl_metadata.methods)
        .filter_map(|(method, method_metadata)| {
            let bindings = graph.impl_method_bindings(method_metadata.node)?;
            let symbol = method_symbol(
                module_path,
                &impl_metadata.trait_path,
                &target_type,
                &method_metadata.name,
            );
            Some(ScriptImplMethod {
                target_type: target_type.clone(),
                method_name: method_metadata.name.clone(),
                symbol,
                function: &method.function,
                signature: &method_metadata.signature,
                bindings,
            })
        })
        .collect()
}

fn syntax_impl_item<'ast>(
    parsed: &'ast SourceFile,
    metadata: &ImplMetadata,
) -> Option<&'ast ImplItem> {
    parsed.items.iter().find_map(|item| {
        let ItemKind::Impl(item) = &item.kind else {
            return None;
        };
        (item.trait_path == metadata.trait_path && item.target_path == metadata.target_path)
            .then_some(item)
    })
}

fn local_target_name(path: &[String]) -> String {
    path.join(".")
}

fn module_target_name(module_path: Option<&ModulePath>, path: &[String]) -> String {
    if path.len() != 1 {
        return path.join(".");
    }
    let Some(module_path) = module_path else {
        return path[0].clone();
    };
    if module_path.segments().is_empty() {
        path[0].clone()
    } else {
        format!("{}.{}", module_path.join(), path[0])
    }
}

fn method_symbol(
    module_path: Option<&ModulePath>,
    trait_path: &[String],
    target_type: &str,
    method: &str,
) -> String {
    let prefix = module_path
        .filter(|path| !path.segments().is_empty())
        .map_or_else(String::new, |path| format!("{}.", path.join()));
    format!(
        "{prefix}__impl.{}.for.{}.{}",
        trait_path.join("."),
        target_type,
        method
    )
}
