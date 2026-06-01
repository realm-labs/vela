use std::collections::{BTreeMap, BTreeSet};

use vela_common::MethodId;
use vela_hir::binding::BindingMap;
use vela_hir::ids::ModuleId;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph, ModulePath};
use vela_hir::type_hint::{FunctionSignature, ImplMetadata};
use vela_syntax::ast::{Block, ImplItem, ItemKind, Param, SourceFile, TraitItem};

pub(super) struct ScriptImplMethod<'ast> {
    pub(super) target_type: String,
    pub(super) method_name: String,
    pub(super) method_id: MethodId,
    pub(super) symbol: String,
    pub(super) params: &'ast [Param],
    pub(super) body: &'ast Block,
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
            let trait_item =
                trait_declaration(graph, declaration.module, &impl_metadata.trait_path)
                    .and_then(|declaration| graph.trait_shape(declaration))
                    .zip(syntax_trait_item(parsed, &impl_metadata.trait_path));
            collect_methods(
                graph,
                module_path,
                impl_metadata,
                item,
                trait_item,
                target_type,
            )
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
            let Some(trait_declaration) =
                trait_declaration(graph, declaration.module, &impl_metadata.trait_path)
            else {
                return collect_methods(graph, module_path, impl_metadata, item, None, target_type);
            };
            let trait_item = graph
                .declaration(trait_declaration)
                .and_then(|declaration| parsed.get(&declaration.module))
                .and_then(|source| syntax_trait_item(source, &impl_metadata.trait_path))
                .zip(graph.trait_shape(trait_declaration))
                .map(|(item, shape)| (shape, item));
            collect_methods(
                graph,
                module_path,
                impl_metadata,
                item,
                trait_item,
                target_type,
            )
        })
        .collect()
}

fn collect_methods<'ast>(
    graph: &'ast ModuleGraph,
    module_path: Option<&'ast ModulePath>,
    impl_metadata: &'ast ImplMetadata,
    item: &'ast ImplItem,
    trait_item: Option<(&'ast vela_hir::type_hint::TraitShape, &'ast TraitItem)>,
    target_type: String,
) -> Vec<ScriptImplMethod<'ast>> {
    let explicit_names = item
        .methods
        .iter()
        .map(|method| method.function.name.clone())
        .collect::<BTreeSet<_>>();
    let mut methods = item
        .methods
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
            let method_id = stable_trait_method_id(
                &trait_method_owner_name(module_path, &impl_metadata.trait_path),
                &method_metadata.name,
            );
            Some(ScriptImplMethod {
                target_type: target_type.clone(),
                method_name: method_metadata.name.clone(),
                method_id,
                symbol,
                params: &method.function.params,
                body: &method.function.body,
                signature: &method_metadata.signature,
                bindings,
            })
        })
        .collect::<Vec<_>>();
    if let Some((trait_shape, trait_item)) = trait_item {
        methods.extend(collect_default_methods(
            graph,
            module_path,
            impl_metadata,
            trait_shape,
            trait_item,
            &target_type,
            &explicit_names,
        ));
    }
    methods
}

fn collect_default_methods<'ast>(
    graph: &'ast ModuleGraph,
    module_path: Option<&'ast ModulePath>,
    impl_metadata: &'ast ImplMetadata,
    trait_shape: &'ast vela_hir::type_hint::TraitShape,
    trait_item: &'ast TraitItem,
    target_type: &str,
    explicit_names: &BTreeSet<String>,
) -> Vec<ScriptImplMethod<'ast>> {
    trait_item
        .methods
        .iter()
        .zip(&trait_shape.methods)
        .filter_map(|(method, method_metadata)| {
            if explicit_names.contains(&method_metadata.name) {
                return None;
            }
            let body = method.default_body.as_ref()?;
            let node = method_metadata.default_body_node?;
            let bindings = graph.trait_default_method_bindings(node)?;
            let symbol = method_symbol(
                module_path,
                &impl_metadata.trait_path,
                target_type,
                &method_metadata.name,
            );
            let method_id = stable_trait_method_id(
                &trait_method_owner_name(module_path, &impl_metadata.trait_path),
                &method_metadata.name,
            );
            Some(ScriptImplMethod {
                target_type: target_type.to_owned(),
                method_name: method_metadata.name.clone(),
                method_id,
                symbol,
                params: &method.params,
                body,
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

fn syntax_trait_item<'ast>(parsed: &'ast SourceFile, path: &[String]) -> Option<&'ast TraitItem> {
    let name = path.last()?;
    parsed.items.iter().find_map(|item| {
        let ItemKind::Trait(item) = &item.kind else {
            return None;
        };
        (item.name == *name).then_some(item)
    })
}

fn trait_declaration(
    graph: &ModuleGraph,
    owner_module: ModuleId,
    path: &[String],
) -> Option<vela_hir::ids::HirDeclId> {
    if path.len() == 1 {
        let declaration = graph.module(owner_module)?.get(&path[0])?;
        return (graph.declaration(declaration)?.kind == DeclarationKind::Trait)
            .then_some(declaration);
    }
    let full_name = path.join(".");
    graph.declarations().find_map(|declaration| {
        (declaration.kind == DeclarationKind::Trait
            && declaration_qualified_name(graph, declaration) == full_name)
            .then_some(declaration.id)
    })
}

fn declaration_qualified_name(
    graph: &ModuleGraph,
    declaration: &vela_hir::module_graph::Declaration,
) -> String {
    let Some(module_path) = graph.module_path(declaration.module) else {
        return declaration.name.clone();
    };
    if module_path.segments().is_empty() {
        declaration.name.clone()
    } else {
        format!("{}.{}", module_path.join(), declaration.name)
    }
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

fn trait_method_owner_name(module_path: Option<&ModulePath>, trait_path: &[String]) -> String {
    if trait_path.len() != 1 {
        return trait_path.join(".");
    }
    let Some(module_path) = module_path else {
        return trait_path[0].clone();
    };
    if module_path.segments().is_empty() {
        trait_path[0].clone()
    } else {
        format!("{}.{}", module_path.join(), trait_path[0])
    }
}

fn stable_trait_method_id(trait_name: &str, method_name: &str) -> MethodId {
    MethodId::new(stable_id("trait_method", trait_name, method_name))
}

fn stable_id(kind: &str, owner: &str, member: &str) -> u32 {
    let mut hash = 0x811c_9dc5;
    for byte in kind
        .bytes()
        .chain([0])
        .chain(owner.bytes())
        .chain([0])
        .chain(member.bytes())
    {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    if hash == 0 { 1 } else { hash }
}
