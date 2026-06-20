use std::collections::{BTreeMap, BTreeSet};

use vela_def::MethodId;
use vela_hir::binding::BindingMap;
use vela_hir::ids::ModuleId;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph, ModulePath};
use vela_hir::type_hint::{FunctionSignature, ImplMetadata, ImplMetadataKind};
use vela_syntax::ast::{Block, Expr, ImplItem, ImplKind, ItemKind, SourceFile, TraitItem};

pub(super) struct ScriptImplMethod<'ast> {
    pub(super) target_type: String,
    pub(super) method_name: String,
    pub(super) method_id: MethodId,
    pub(super) symbol: String,
    pub(super) default_values: Vec<Option<Expr>>,
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
            let trait_item = impl_metadata
                .trait_path()
                .and_then(|trait_path| trait_declaration(graph, declaration.module, trait_path))
                .and_then(|declaration| graph.trait_shape(declaration))
                .zip(
                    impl_metadata
                        .trait_path()
                        .and_then(|trait_path| syntax_trait_item(parsed, trait_path)),
                );
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
            let Some(trait_path) = impl_metadata.trait_path() else {
                return collect_methods(graph, module_path, impl_metadata, item, None, target_type);
            };
            let Some(trait_declaration) = trait_declaration(graph, declaration.module, trait_path)
            else {
                return collect_methods(graph, module_path, impl_metadata, item, None, target_type);
            };
            let trait_item = graph
                .declaration(trait_declaration)
                .and_then(|declaration| parsed.get(&declaration.module))
                .and_then(|source| syntax_trait_item(source, trait_path))
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
                impl_metadata,
                &target_type,
                &method_metadata.name,
            );
            let method_id = stable_method_id(module_path, impl_metadata, &method_metadata.name);
            Some(ScriptImplMethod {
                target_type: target_type.clone(),
                method_name: method_metadata.name.clone(),
                method_id,
                symbol,
                default_values: default_values(&method.function.params),
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
                impl_metadata,
                target_type,
                &method_metadata.name,
            );
            let method_id = stable_method_id(module_path, impl_metadata, &method_metadata.name);
            Some(ScriptImplMethod {
                target_type: target_type.to_owned(),
                method_name: method_metadata.name.clone(),
                method_id,
                symbol,
                default_values: default_values(&method.params),
                body,
                signature: &method_metadata.signature,
                bindings,
            })
        })
        .collect()
}

fn default_values(params: &[vela_syntax::ast::Param]) -> Vec<Option<Expr>> {
    params
        .iter()
        .map(|param| param.default_value.clone())
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
        (impl_kind_matches(&item.kind, &metadata.kind) && item.target_path == metadata.target_path)
            .then_some(item)
    })
}

fn impl_kind_matches(item: &ImplKind, metadata: &ImplMetadataKind) -> bool {
    match (item, metadata) {
        (ImplKind::Inherent, ImplMetadataKind::Inherent) => true,
        (
            ImplKind::Trait {
                trait_path: item_trait,
            },
            ImplMetadataKind::Trait {
                trait_path: metadata_trait,
            },
        ) => item_trait == metadata_trait,
        _ => false,
    }
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
    let full_name = path.join("::");
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
        format!("{}::{}", module_path.join(), declaration.name)
    }
}

fn local_target_name(path: &[String]) -> String {
    path.join("::")
}

fn module_target_name(module_path: Option<&ModulePath>, path: &[String]) -> String {
    if path.len() != 1 {
        return path.join("::");
    }
    let Some(module_path) = module_path else {
        return path[0].clone();
    };
    if module_path.segments().is_empty() {
        path[0].clone()
    } else {
        format!("{}::{}", module_path.join(), path[0])
    }
}

fn method_symbol(
    module_path: Option<&ModulePath>,
    impl_metadata: &ImplMetadata,
    target_type: &str,
    method: &str,
) -> String {
    let prefix = module_path
        .filter(|path| !path.segments().is_empty())
        .map_or_else(String::new, |path| format!("{}.", path.join()));
    match &impl_metadata.kind {
        ImplMetadataKind::Inherent => format!("{prefix}__impl.{}.{}", target_type, method),
        ImplMetadataKind::Trait { trait_path } => {
            format!(
                "{prefix}__impl.{}.for.{}.{}",
                trait_path.join("::"),
                target_type,
                method
            )
        }
    }
}

fn stable_method_id(
    module_path: Option<&ModulePath>,
    impl_metadata: &ImplMetadata,
    method_name: &str,
) -> MethodId {
    match &impl_metadata.kind {
        ImplMetadataKind::Inherent => stable_inherent_method_id(
            &target_owner_name(module_path, &impl_metadata.target_path),
            method_name,
        ),
        ImplMetadataKind::Trait { trait_path } => stable_trait_method_id(
            &trait_method_owner_name(module_path, trait_path),
            method_name,
        ),
    }
}

fn target_owner_name(module_path: Option<&ModulePath>, target_path: &[String]) -> String {
    if target_path.len() != 1 {
        return target_path.join("::");
    }
    let Some(module_path) = module_path else {
        return target_path[0].clone();
    };
    if module_path.segments().is_empty() {
        target_path[0].clone()
    } else {
        format!("{}::{}", module_path.join(), target_path[0])
    }
}

fn trait_method_owner_name(module_path: Option<&ModulePath>, trait_path: &[String]) -> String {
    if is_builtin_operator_trait(trait_path) {
        return trait_path[0].clone();
    }
    if trait_path.len() != 1 {
        return trait_path.join("::");
    }
    let Some(module_path) = module_path else {
        return trait_path[0].clone();
    };
    if module_path.segments().is_empty() {
        trait_path[0].clone()
    } else {
        format!("{}::{}", module_path.join(), trait_path[0])
    }
}

fn is_builtin_operator_trait(path: &[String]) -> bool {
    let [name] = path else {
        return false;
    };
    matches!(name.as_str(), "PartialEq" | "Eq" | "PartialOrd" | "Ord")
}

fn stable_trait_method_id(trait_name: &str, method_name: &str) -> MethodId {
    MethodId::new(u128::from(vela_common::stable_id(
        "trait_method",
        trait_name,
        method_name,
    )))
}

fn stable_inherent_method_id(type_name: &str, method_name: &str) -> MethodId {
    MethodId::new(u128::from(vela_common::stable_id(
        "inherent_method",
        type_name,
        method_name,
    )))
}

trait ImplMetadataExt {
    fn trait_path(&self) -> Option<&[String]>;
}

impl ImplMetadataExt for ImplMetadata {
    fn trait_path(&self) -> Option<&[String]> {
        match &self.kind {
            ImplMetadataKind::Inherent => None,
            ImplMetadataKind::Trait { trait_path } => Some(trait_path),
        }
    }
}
