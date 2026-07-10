//! Backend-neutral catalog of executable script methods.
//!
//! The catalog expands trait defaults and owns only HIR identities, signatures,
//! source origins, and stable method-identity inputs. Backends may derive their
//! own lowering context from the graph without placing bindings or backend
//! constants in this shared boundary.

use std::collections::BTreeSet;

use vela_def::{MethodId, script_inherent_method_id, script_trait_method_id};

use crate::body::{HirBody, HirSourceOrigin};
use crate::ids::{HirBodyId, HirDeclId, HirNodeId, ModuleId};
use crate::module_graph::{DeclarationKind, ModuleGraph, ModulePath};
use crate::type_hint::{FunctionSignature, ImplMetadata, ImplMetadataKind};

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScriptMethodCatalogMode {
    SingleSource {
        module: ModuleId,
        identity_namespace: String,
    },
    ModuleGraph,
}

impl ScriptMethodCatalogMode {
    #[must_use]
    pub fn single_source(module: ModuleId, identity_namespace: impl Into<String>) -> Self {
        Self::SingleSource {
            module,
            identity_namespace: identity_namespace.into(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ScriptMethodCatalog {
    methods: Vec<ScriptMethod>,
}

impl ScriptMethodCatalog {
    #[must_use]
    pub fn from_graph(graph: &ModuleGraph, mode: ScriptMethodCatalogMode) -> Self {
        let methods = match mode {
            ScriptMethodCatalogMode::SingleSource {
                module,
                identity_namespace,
            } => graph
                .declarations()
                .filter(|declaration| declaration.module == module)
                .filter(|declaration| declaration.kind == DeclarationKind::Impl)
                .flat_map(|declaration| {
                    collect_impl_methods(
                        graph,
                        declaration.id,
                        TargetQualification::Local,
                        Some(identity_namespace.as_str()),
                    )
                })
                .collect(),
            ScriptMethodCatalogMode::ModuleGraph => graph
                .declarations()
                .filter(|declaration| declaration.kind == DeclarationKind::Impl)
                .flat_map(|declaration| {
                    collect_impl_methods(graph, declaration.id, TargetQualification::Module, None)
                })
                .collect(),
        };
        Self { methods }
    }

    pub fn methods(&self) -> impl ExactSizeIterator<Item = &ScriptMethod> {
        self.methods.iter()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.methods.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.methods.len()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScriptMethod {
    owner: ScriptMethodOwner,
    node: HirNodeId,
    name: String,
    body: HirBodyId,
    signature: FunctionSignature,
    parameter_default_bodies: Vec<Option<HirBodyId>>,
    module: ModuleId,
    signature_module: ModuleId,
    origin: HirSourceOrigin,
    name_origin: HirSourceOrigin,
    owner_origin: HirSourceOrigin,
}

impl ScriptMethod {
    #[must_use]
    pub const fn owner(&self) -> &ScriptMethodOwner {
        &self.owner
    }

    #[must_use]
    pub const fn node(&self) -> HirNodeId {
        self.node
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn body(&self) -> HirBodyId {
        self.body
    }

    #[must_use]
    pub const fn signature(&self) -> &FunctionSignature {
        &self.signature
    }

    #[must_use]
    pub fn parameter_default_bodies(&self) -> &[Option<HirBodyId>] {
        &self.parameter_default_bodies
    }

    #[must_use]
    pub const fn module(&self) -> ModuleId {
        self.module
    }

    #[must_use]
    pub const fn signature_module(&self) -> ModuleId {
        self.signature_module
    }

    #[must_use]
    pub const fn origin(&self) -> HirSourceOrigin {
        self.origin
    }

    #[must_use]
    pub const fn name_origin(&self) -> HirSourceOrigin {
        self.name_origin
    }

    #[must_use]
    pub const fn owner_origin(&self) -> HirSourceOrigin {
        self.owner_origin
    }

    #[must_use]
    pub fn method_id(&self) -> MethodId {
        self.owner.identity.method_id(&self.name)
    }

    /// Canonical source symbol seed used by compile-target and direct-lowering
    /// adapters. It is not a bytecode or linker handle.
    #[must_use]
    pub fn symbol_seed(&self) -> String {
        self.owner.symbol_seed(&self.name)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScriptMethodOwner {
    target_type: String,
    actual_module: ModulePath,
    identity: ScriptMethodIdentity,
}

impl ScriptMethodOwner {
    #[must_use]
    pub fn target_type(&self) -> &str {
        &self.target_type
    }

    #[must_use]
    pub const fn actual_module(&self) -> &ModulePath {
        &self.actual_module
    }

    #[must_use]
    pub const fn identity(&self) -> &ScriptMethodIdentity {
        &self.identity
    }

    fn symbol_seed(&self, method: &str) -> String {
        let prefix = if self.actual_module.segments().is_empty() {
            String::new()
        } else {
            format!("{}.", self.actual_module.join())
        };
        match &self.identity {
            ScriptMethodIdentity::Inherent { .. } => {
                format!("{prefix}__impl.{}.{method}", self.target_type)
            }
            ScriptMethodIdentity::Trait {
                source_trait_path, ..
            } => format!(
                "{prefix}__impl.{source_trait_path}.for.{}.{method}",
                self.target_type
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScriptMethodIdentity {
    Inherent {
        owner: String,
    },
    Trait {
        trait_name: String,
        source_trait_path: String,
    },
}

impl ScriptMethodIdentity {
    #[must_use]
    pub fn canonical_owner(&self) -> &str {
        match self {
            Self::Inherent { owner } => owner,
            Self::Trait { trait_name, .. } => trait_name,
        }
    }

    fn method_id(&self, method: &str) -> MethodId {
        match self {
            Self::Inherent { owner } => script_inherent_method_id(owner, method),
            Self::Trait { trait_name, .. } => script_trait_method_id(trait_name, method),
        }
    }
}

#[derive(Clone, Copy)]
enum TargetQualification {
    Local,
    Module,
}

struct MethodBuildInput<'graph> {
    target_type: String,
    name: String,
    name_origin: HirSourceOrigin,
    signature: &'graph FunctionSignature,
    body: &'graph HirBody,
    module: ModuleId,
    signature_module: ModuleId,
}

fn collect_impl_methods(
    graph: &ModuleGraph,
    declaration: HirDeclId,
    qualification: TargetQualification,
    identity_namespace: Option<&str>,
) -> Vec<ScriptMethod> {
    let Some(declaration_metadata) = graph.declaration(declaration) else {
        return Vec::new();
    };
    let Some(impl_metadata) = graph.impl_metadata(declaration) else {
        return Vec::new();
    };
    let actual_module = graph
        .module_path(declaration_metadata.module)
        .cloned()
        .unwrap_or_else(ModulePath::root);
    let target_type = match qualification {
        TargetQualification::Local => impl_metadata.target_path.join("::"),
        TargetQualification::Module => {
            module_target_name(Some(&actual_module), &impl_metadata.target_path)
        }
    };
    let owner_origin = origin(declaration_metadata.span);
    let mut methods = impl_metadata
        .methods
        .iter()
        .filter_map(|method| {
            let body = graph.impl_method_body(method.node)?;
            Some(build_method(
                &actual_module,
                identity_namespace,
                impl_metadata,
                owner_origin,
                MethodBuildInput {
                    target_type: target_type.clone(),
                    name: method.name.clone(),
                    name_origin: origin(method.name_span),
                    signature: &method.signature,
                    body,
                    module: declaration_metadata.module,
                    signature_module: declaration_metadata.module,
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
        && let Some(trait_metadata) = graph.declaration(trait_declaration)
        && let Some(shape) = graph.trait_shape(trait_declaration)
    {
        methods.extend(shape.methods.iter().filter_map(|method| {
            if explicit.contains(method.name.as_str()) {
                return None;
            }
            let node = method.default_body_node?;
            let body = graph.trait_default_method_body(node)?;
            Some(build_method(
                &actual_module,
                identity_namespace,
                impl_metadata,
                owner_origin,
                MethodBuildInput {
                    target_type: target_type.clone(),
                    name: method.name.clone(),
                    name_origin: origin(method.name_span),
                    signature: &method.signature,
                    body,
                    module: declaration_metadata.module,
                    signature_module: trait_metadata.module,
                },
            ))
        }));
    }
    methods
}

fn build_method(
    actual_module: &ModulePath,
    identity_namespace: Option<&str>,
    impl_metadata: &ImplMetadata,
    owner_origin: HirSourceOrigin,
    input: MethodBuildInput<'_>,
) -> ScriptMethod {
    let identity = match &impl_metadata.kind {
        ImplMetadataKind::Inherent => ScriptMethodIdentity::Inherent {
            owner: target_owner_name(
                Some(actual_module),
                identity_namespace,
                &impl_metadata.target_path,
            ),
        },
        ImplMetadataKind::Trait { trait_path } => ScriptMethodIdentity::Trait {
            trait_name: trait_method_owner_name(
                Some(actual_module),
                identity_namespace,
                trait_path,
            ),
            source_trait_path: trait_path.join("::"),
        },
    };
    ScriptMethod {
        owner: ScriptMethodOwner {
            target_type: input.target_type,
            actual_module: actual_module.clone(),
            identity,
        },
        node: match input.body.owner {
            crate::body::HirBodyOwner::TraitDefaultMethod(node)
            | crate::body::HirBodyOwner::ImplMethod(node) => node,
            _ => unreachable!("script methods own method HIR bodies"),
        },
        name: input.name,
        body: input.body.id,
        signature: input.signature.clone(),
        parameter_default_bodies: input
            .body
            .params
            .iter()
            .map(|parameter| parameter.default_body)
            .collect(),
        module: input.module,
        signature_module: input.signature_module,
        origin: input.body.origin,
        name_origin: input.name_origin,
        owner_origin,
    }
}

fn trait_declaration(
    graph: &ModuleGraph,
    owner_module: ModuleId,
    path: &[String],
) -> Option<HirDeclId> {
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

fn target_owner_name(
    module_path: Option<&ModulePath>,
    identity_namespace: Option<&str>,
    target_path: &[String],
) -> String {
    if target_path.len() != 1 {
        return target_path.join("::");
    }
    if let Some(module) = identity_namespace {
        return format!("{module}::{}", target_path[0]);
    }
    module_target_name(module_path, target_path)
}

fn trait_method_owner_name(
    module_path: Option<&ModulePath>,
    identity_namespace: Option<&str>,
    trait_path: &[String],
) -> String {
    if is_builtin_operator_trait(trait_path) || trait_path.len() != 1 {
        return trait_path.join("::");
    }
    if let Some(module) = identity_namespace {
        return format!("{module}::{}", trait_path[0]);
    }
    module_target_name(module_path, trait_path)
}

fn is_builtin_operator_trait(path: &[String]) -> bool {
    matches!(path, [name] if matches!(name.as_str(), "PartialEq" | "Eq" | "PartialOrd" | "Ord"))
}

fn origin(span: vela_common::Span) -> HirSourceOrigin {
    HirSourceOrigin {
        source: span.source,
        span,
    }
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
