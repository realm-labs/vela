use std::collections::BTreeSet;

use vela_def::{MethodId, script_inherent_method_id, script_trait_method_id};
use vela_hir::binding::BindingMap;
use vela_hir::body::HirBody;
use vela_hir::ids::{HirBodyId, HirNodeId, ModuleId};
use vela_hir::module_graph::{DeclarationKind, ModuleGraph, ModulePath};
use vela_hir::type_hint::{FunctionSignature, ImplMetadata, ImplMetadataKind};

use super::param_defaults::{ParamDefaultValue, param_default_values};

#[derive(Clone)]
pub(super) struct ScriptImplMethod<'ast> {
    pub(super) target_type: String,
    pub(super) method_name: String,
    pub(super) method_id: MethodId,
    pub(super) node: HirNodeId,
    pub(super) symbol: String,
    pub(super) default_values: Vec<Option<ParamDefaultValue>>,
    pub(super) body: HirBodyId,
    pub(super) signature: &'ast FunctionSignature,
    pub(super) bindings: &'ast BindingMap,
    pub(super) hir_bodies: Vec<&'ast HirBody>,
}

struct MethodBuildInput<'ast> {
    target_type: String,
    method_name: String,
    signature: &'ast FunctionSignature,
    body: &'ast HirBody,
    bindings: &'ast BindingMap,
}

pub(super) fn source_methods(graph: &ModuleGraph, module: ModuleId) -> Vec<ScriptImplMethod<'_>> {
    graph
        .declarations()
        .filter(|declaration| declaration.module == module)
        .filter(|declaration| declaration.kind == DeclarationKind::Impl)
        .flat_map(|declaration| {
            collect_impl_methods(
                graph,
                declaration.id,
                false,
                Some(SINGLE_SOURCE_IDENTITY_MODULE),
            )
        })
        .collect()
}

pub(super) fn module_methods(graph: &ModuleGraph) -> Vec<ScriptImplMethod<'_>> {
    graph
        .declarations()
        .filter(|declaration| declaration.kind == DeclarationKind::Impl)
        .flat_map(|declaration| collect_impl_methods(graph, declaration.id, true, None))
        .collect()
}

// The source front door exposes a root module in emitted metadata, while the
// established hot-reload/dispatch identity contract hashes its methods under
// the synthetic `main` owner. Keep that compatibility mapping confined to
// source-method identity; symbols and module compilation use real graph paths.
const SINGLE_SOURCE_IDENTITY_MODULE: &str = "main";

fn collect_impl_methods<'ast>(
    graph: &'ast ModuleGraph,
    declaration: vela_hir::ids::HirDeclId,
    qualified_target: bool,
    source_identity_module: Option<&'static str>,
) -> Vec<ScriptImplMethod<'ast>> {
    let Some(declaration_metadata) = graph.declaration(declaration) else {
        return Vec::new();
    };
    let Some(impl_metadata) = graph.impl_metadata(declaration) else {
        return Vec::new();
    };
    let module_path = graph.module_path(declaration_metadata.module);
    let target_type = if qualified_target {
        module_target_name(module_path, &impl_metadata.target_path)
    } else {
        local_target_name(&impl_metadata.target_path)
    };
    let mut methods = impl_metadata
        .methods
        .iter()
        .filter_map(|method| {
            let body = graph.impl_method_body(method.node)?;
            let bindings = graph.impl_method_bindings(method.node)?;
            Some(build_method(
                graph,
                module_path,
                source_identity_module,
                impl_metadata,
                MethodBuildInput {
                    target_type: target_type.clone(),
                    method_name: method.name.clone(),
                    signature: &method.signature,
                    body,
                    bindings,
                },
            ))
        })
        .collect::<Vec<_>>();

    let explicit = impl_metadata
        .methods
        .iter()
        .map(|method| method.name.as_str())
        .collect::<BTreeSet<_>>();
    if let Some(trait_path) = impl_metadata.trait_path()
        && let Some(trait_declaration) =
            trait_declaration(graph, declaration_metadata.module, trait_path)
        && let Some(shape) = graph.trait_shape(trait_declaration)
    {
        methods.extend(shape.methods.iter().filter_map(|method| {
            if explicit.contains(method.name.as_str()) {
                return None;
            }
            let node = method.default_body_node?;
            let body = graph.trait_default_method_body(node)?;
            let bindings = graph.trait_default_method_bindings(node)?;
            Some(build_method(
                graph,
                module_path,
                source_identity_module,
                impl_metadata,
                MethodBuildInput {
                    target_type: target_type.clone(),
                    method_name: method.name.clone(),
                    signature: &method.signature,
                    body,
                    bindings,
                },
            ))
        }));
    }
    methods
}

fn build_method<'ast>(
    graph: &'ast ModuleGraph,
    module_path: Option<&'ast ModulePath>,
    source_identity_module: Option<&str>,
    impl_metadata: &'ast ImplMetadata,
    input: MethodBuildInput<'ast>,
) -> ScriptImplMethod<'ast> {
    let MethodBuildInput {
        target_type,
        method_name,
        signature,
        body,
        bindings,
    } = input;
    ScriptImplMethod {
        node: match body.owner {
            vela_hir::body::HirBodyOwner::TraitDefaultMethod(node)
            | vela_hir::body::HirBodyOwner::ImplMethod(node) => node,
            _ => unreachable!("script impl methods own method HIR bodies"),
        },
        method_id: stable_method_id(
            module_path,
            source_identity_module,
            impl_metadata,
            &method_name,
        ),
        symbol: method_symbol(module_path, impl_metadata, &target_type, &method_name),
        default_values: param_default_values(body, signature),
        body: body.id,
        target_type,
        method_name,
        signature,
        bindings,
        hir_bodies: graph.bodies().collect(),
    }
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
            && graph.qualified_declaration_name(declaration.id).as_deref()
                == Some(full_name.as_str()))
        .then_some(declaration.id)
    })
}

fn local_target_name(path: &[String]) -> String {
    path.join("::")
}

fn module_target_name(module_path: Option<&ModulePath>, path: &[String]) -> String {
    if path.len() != 1 {
        return path.join("::");
    }
    module_path
        .filter(|path| !path.segments().is_empty())
        .map_or_else(
            || path[0].clone(),
            |module| format!("{}::{}", module.join(), path[0]),
        )
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
        ImplMetadataKind::Inherent => format!("{prefix}__impl.{target_type}.{method}"),
        ImplMetadataKind::Trait { trait_path } => format!(
            "{prefix}__impl.{}.for.{target_type}.{method}",
            trait_path.join("::")
        ),
    }
}

fn stable_method_id(
    module_path: Option<&ModulePath>,
    source_identity_module: Option<&str>,
    impl_metadata: &ImplMetadata,
    method_name: &str,
) -> MethodId {
    match &impl_metadata.kind {
        ImplMetadataKind::Inherent => script_inherent_method_id(
            &target_owner_name(
                module_path,
                source_identity_module,
                &impl_metadata.target_path,
            ),
            method_name,
        ),
        ImplMetadataKind::Trait { trait_path } => script_trait_method_id(
            &trait_method_owner_name(module_path, source_identity_module, trait_path),
            method_name,
        ),
    }
}

fn target_owner_name(
    module_path: Option<&ModulePath>,
    source_identity_module: Option<&str>,
    target_path: &[String],
) -> String {
    if target_path.len() != 1 {
        return target_path.join("::");
    }
    if let Some(module) = source_identity_module {
        return format!("{module}::{}", target_path[0]);
    }
    module_path
        .filter(|path| !path.segments().is_empty())
        .map_or_else(
            || target_path[0].clone(),
            |module| format!("{}::{}", module.join(), target_path[0]),
        )
}

fn trait_method_owner_name(
    module_path: Option<&ModulePath>,
    source_identity_module: Option<&str>,
    trait_path: &[String],
) -> String {
    if is_builtin_operator_trait(trait_path) || trait_path.len() != 1 {
        return trait_path.join("::");
    }
    if let Some(module) = source_identity_module {
        return format!("{module}::{}", trait_path[0]);
    }
    module_path
        .filter(|path| !path.segments().is_empty())
        .map_or_else(
            || trait_path[0].clone(),
            |module| format!("{}::{}", module.join(), trait_path[0]),
        )
}

fn is_builtin_operator_trait(path: &[String]) -> bool {
    matches!(path, [name] if matches!(name.as_str(), "PartialEq" | "Eq" | "PartialOrd" | "Ord"))
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
