use std::collections::{BTreeMap, BTreeSet};

use vela_def::MethodId;
use vela_hir::binding::BindingMap;
use vela_hir::ids::ModuleId;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph, ModulePath};
use vela_hir::type_hint::{FunctionSignature, ImplMetadata, ImplMetadataKind};
use vela_syntax::Parse as SyntaxParse;
use vela_syntax::ast::{
    ImplItem, ImplKind, ItemKind, SourceFile, SyntaxImplItem, SyntaxSourceFile, SyntaxTraitItem,
    TraitItem,
};

use super::body_payloads::CompilerBodyPayload;
use super::param_defaults::{ParamDefaultValue, syntax_param_default_values};

pub(super) struct ScriptImplMethod<'ast> {
    pub(super) target_type: String,
    pub(super) method_name: String,
    pub(super) method_id: MethodId,
    pub(super) symbol: String,
    pub(super) default_values: Vec<Option<ParamDefaultValue>>,
    pub(super) body: CompilerBodyPayload<'ast>,
    pub(super) signature: &'ast FunctionSignature,
    pub(super) bindings: &'ast BindingMap,
}

struct MethodBodyPayload<'ast> {
    default_values: Vec<Option<ParamDefaultValue>>,
    body: CompilerBodyPayload<'ast>,
}

pub(super) fn source_methods<'ast>(
    parsed: &'ast SourceFile,
    syntax: &SyntaxParse<SyntaxSourceFile>,
    source: vela_common::SourceId,
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
            let method_payloads = impl_method_payloads(parsed, syntax, source, impl_metadata);
            let target_type = local_target_name(&impl_metadata.target_path);
            let trait_item = impl_metadata
                .trait_path()
                .and_then(|trait_path| trait_declaration(graph, declaration.module, trait_path))
                .and_then(|declaration| graph.trait_shape(declaration))
                .zip(impl_metadata.trait_path().map(|trait_path| {
                    trait_default_method_payloads(parsed, syntax, source, trait_path)
                }));
            collect_methods(
                graph,
                module_path,
                impl_metadata,
                &method_payloads,
                trait_item,
                target_type,
            )
        })
        .collect()
}

pub(super) fn module_methods<'ast>(
    parsed: &'ast BTreeMap<ModuleId, SourceFile>,
    syntax: &BTreeMap<ModuleId, SyntaxParse<SyntaxSourceFile>>,
    source_ids: &BTreeMap<ModuleId, vela_common::SourceId>,
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
            let Some(syntax_source) = syntax.get(&declaration.module) else {
                return Vec::new();
            };
            let Some(source_id) = source_ids.get(&declaration.module).copied() else {
                return Vec::new();
            };
            let method_payloads =
                impl_method_payloads(source, syntax_source, source_id, impl_metadata);
            let target_type = module_target_name(module_path, &impl_metadata.target_path);
            let Some(trait_path) = impl_metadata.trait_path() else {
                return collect_methods(
                    graph,
                    module_path,
                    impl_metadata,
                    &method_payloads,
                    None,
                    target_type,
                );
            };
            let Some(trait_declaration) = trait_declaration(graph, declaration.module, trait_path)
            else {
                return collect_methods(
                    graph,
                    module_path,
                    impl_metadata,
                    &method_payloads,
                    None,
                    target_type,
                );
            };
            let trait_item = graph
                .declaration(trait_declaration)
                .and_then(|declaration| parsed.get(&declaration.module))
                .zip(
                    graph
                        .declaration(trait_declaration)
                        .and_then(|declaration| syntax.get(&declaration.module)),
                )
                .zip(
                    graph
                        .declaration(trait_declaration)
                        .and_then(|declaration| source_ids.get(&declaration.module))
                        .copied(),
                )
                .map(|((source, syntax), source_id)| {
                    trait_default_method_payloads(source, syntax, source_id, trait_path)
                })
                .zip(graph.trait_shape(trait_declaration));
            collect_methods(
                graph,
                module_path,
                impl_metadata,
                &method_payloads,
                trait_item.map(|(payloads, shape)| (shape, payloads)),
                target_type,
            )
        })
        .collect()
}

fn collect_methods<'ast>(
    graph: &'ast ModuleGraph,
    module_path: Option<&'ast ModulePath>,
    impl_metadata: &'ast ImplMetadata,
    method_payloads: &BTreeMap<String, MethodBodyPayload<'ast>>,
    trait_item: Option<(
        &'ast vela_hir::type_hint::TraitShape,
        BTreeMap<String, MethodBodyPayload<'ast>>,
    )>,
    target_type: String,
) -> Vec<ScriptImplMethod<'ast>> {
    let explicit_names = impl_metadata
        .methods
        .iter()
        .map(|method| method.name.clone())
        .collect::<BTreeSet<_>>();
    let mut methods = impl_metadata
        .methods
        .iter()
        .filter_map(|method_metadata| {
            let payload = method_payloads.get(&method_metadata.name)?;
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
                default_values: payload.default_values.clone(),
                body: payload.body.clone(),
                signature: &method_metadata.signature,
                bindings,
            })
        })
        .collect::<Vec<_>>();
    if let Some((trait_shape, trait_payloads)) = trait_item {
        methods.extend(collect_default_methods(
            graph,
            module_path,
            impl_metadata,
            trait_shape,
            &trait_payloads,
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
    trait_payloads: &BTreeMap<String, MethodBodyPayload<'ast>>,
    target_type: &str,
    explicit_names: &BTreeSet<String>,
) -> Vec<ScriptImplMethod<'ast>> {
    trait_shape
        .methods
        .iter()
        .filter_map(|method_metadata| {
            if explicit_names.contains(&method_metadata.name) {
                return None;
            }
            let node = method_metadata.default_body_node?;
            let payload = trait_payloads.get(&method_metadata.name)?;
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
                default_values: payload.default_values.clone(),
                body: payload.body.clone(),
                signature: &method_metadata.signature,
                bindings,
            })
        })
        .collect()
}

fn impl_method_payloads<'ast>(
    parsed: &'ast SourceFile,
    syntax: &SyntaxParse<SyntaxSourceFile>,
    source: vela_common::SourceId,
    metadata: &ImplMetadata,
) -> BTreeMap<String, MethodBodyPayload<'ast>> {
    let Some(syntax_item) = syntax_impl_item(syntax, metadata) else {
        return BTreeMap::new();
    };
    legacy_impl_item(parsed, metadata)
        .map(|item| {
            item.methods
                .iter()
                .filter_map(|method| {
                    let syntax_method = syntax_item.methods().find(|syntax_method| {
                        syntax_method.name_text().as_deref() == Some(method.function.name.as_str())
                    })?;
                    let syntax_body = syntax_method.body()?;
                    Some((
                        method.function.name.clone(),
                        MethodBodyPayload {
                            default_values: syntax_param_default_values(
                                source,
                                syntax_method.param_list(),
                                &method.function.params,
                                method.function.params.len(),
                            ),
                            body: CompilerBodyPayload::syntax(
                                source,
                                syntax_body,
                                &method.function.body,
                            ),
                        },
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn trait_default_method_payloads<'ast>(
    parsed: &'ast SourceFile,
    syntax: &SyntaxParse<SyntaxSourceFile>,
    source: vela_common::SourceId,
    path: &[String],
) -> BTreeMap<String, MethodBodyPayload<'ast>> {
    let Some(syntax_item) = syntax_trait_item(syntax, path) else {
        return BTreeMap::new();
    };
    legacy_trait_item(parsed, path)
        .map(|item| {
            item.methods
                .iter()
                .filter_map(|method| {
                    let body = method.default_body.as_ref()?;
                    let syntax_method = syntax_item.methods().find(|syntax_method| {
                        syntax_method.name_text().as_deref() == Some(method.name.as_str())
                    })?;
                    let syntax_body = syntax_method.body()?;
                    Some((
                        method.name.clone(),
                        MethodBodyPayload {
                            default_values: syntax_param_default_values(
                                source,
                                syntax_method.param_list(),
                                &method.params,
                                method.params.len(),
                            ),
                            body: CompilerBodyPayload::syntax(source, syntax_body, body),
                        },
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn legacy_impl_item<'ast>(
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

fn syntax_impl_item(
    parsed: &SyntaxParse<SyntaxSourceFile>,
    metadata: &ImplMetadata,
) -> Option<SyntaxImplItem> {
    parsed.tree().impls().find(|item| {
        impl_kind_matches_syntax(&item.trait_path_segments(), &metadata.kind)
            && item.target_path_segments() == metadata.target_path
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

fn impl_kind_matches_syntax(item_trait: &[String], metadata: &ImplMetadataKind) -> bool {
    match metadata {
        ImplMetadataKind::Inherent => item_trait.is_empty(),
        ImplMetadataKind::Trait { trait_path } => item_trait == trait_path,
    }
}

fn legacy_trait_item<'ast>(parsed: &'ast SourceFile, path: &[String]) -> Option<&'ast TraitItem> {
    let name = path.last()?;
    parsed.items.iter().find_map(|item| {
        let ItemKind::Trait(item) = &item.kind else {
            return None;
        };
        (item.name == *name).then_some(item)
    })
}

fn syntax_trait_item(
    parsed: &SyntaxParse<SyntaxSourceFile>,
    path: &[String],
) -> Option<SyntaxTraitItem> {
    let name = path.last()?;
    parsed
        .tree()
        .traits()
        .find(|item| item.name_text().as_deref() == Some(name.as_str()))
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
