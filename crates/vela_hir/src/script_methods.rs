//! Backend-neutral catalog of executable script methods.
//!
//! The catalog expands trait defaults and owns only HIR identities, signatures,
//! source origins, and stable method-identity inputs. Backends may derive their
//! own lowering context from the graph without placing bindings or backend
//! constants in this shared boundary.

use std::collections::BTreeSet;
use std::fmt;

use vela_def::{MethodId, script_inherent_method_id, script_trait_method_id};

use crate::body::{HirBody, HirSourceOrigin};
use crate::ids::{HirBodyId, HirDeclId, HirNodeId, ModuleId};
use crate::module_graph::{Declaration, DeclarationKind, ModuleGraph, ModulePath};
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
    pub fn from_graph(
        graph: &ModuleGraph,
        mode: ScriptMethodCatalogMode,
    ) -> Result<Self, ScriptMethodCatalogError> {
        let (declarations, qualification, identity_namespace) = match mode {
            ScriptMethodCatalogMode::SingleSource {
                module,
                identity_namespace,
            } => (
                graph
                    .declarations()
                    .filter(|declaration| declaration.module == module)
                    .filter(|declaration| declaration.kind == DeclarationKind::Impl)
                    .collect::<Vec<_>>(),
                TargetQualification::Local,
                Some(identity_namespace),
            ),
            ScriptMethodCatalogMode::ModuleGraph => (
                graph
                    .declarations()
                    .filter(|declaration| declaration.kind == DeclarationKind::Impl)
                    .collect::<Vec<_>>(),
                TargetQualification::Module,
                None,
            ),
        };
        let mut methods = Vec::new();
        for declaration in declarations {
            methods.extend(collect_impl_methods(
                graph,
                declaration,
                qualification,
                identity_namespace.as_deref(),
            )?);
        }
        Ok(Self { methods })
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
pub struct ScriptMethodCatalogError {
    declaration: HirDeclId,
    node: Option<HirNodeId>,
    origin: HirSourceOrigin,
    message: String,
}

impl ScriptMethodCatalogError {
    #[must_use]
    pub const fn declaration(&self) -> HirDeclId {
        self.declaration
    }

    #[must_use]
    pub const fn node(&self) -> Option<HirNodeId> {
        self.node
    }

    #[must_use]
    pub const fn origin(&self) -> HirSourceOrigin {
        self.origin
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ScriptMethodCatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "script method catalog inconsistency for declaration {:?}",
            self.declaration
        )?;
        if let Some(node) = self.node {
            write!(formatter, ", node {node:?}")?;
        }
        write!(formatter, ": {}", self.message)
    }
}

impl std::error::Error for ScriptMethodCatalogError {}

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
    node: HirNodeId,
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
    declaration: &Declaration,
    qualification: TargetQualification,
    identity_namespace: Option<&str>,
) -> Result<Vec<ScriptMethod>, ScriptMethodCatalogError> {
    let impl_metadata = graph.impl_metadata(declaration.id).ok_or_else(|| {
        catalog_error(
            declaration.id,
            None,
            origin(declaration.span),
            "impl declaration has no HIR metadata",
        )
    })?;
    let actual_module = graph
        .module_path(declaration.module)
        .cloned()
        .unwrap_or_else(ModulePath::root);
    let target_type = match qualification {
        TargetQualification::Local => impl_metadata.target_path.join("::"),
        TargetQualification::Module => {
            module_target_name(Some(&actual_module), &impl_metadata.target_path)
        }
    };
    let owner_origin = origin(declaration.span);
    let mut methods = Vec::new();
    for method in &impl_metadata.methods {
        let body = graph.impl_method_body(method.node).ok_or_else(|| {
            catalog_error(
                declaration.id,
                Some(method.node),
                origin(method.span),
                "impl method has no owning HIR body",
            )
        })?;
        methods.push(build_method(
            graph,
            declaration.id,
            &actual_module,
            identity_namespace,
            impl_metadata,
            owner_origin,
            MethodBuildInput {
                node: method.node,
                target_type: target_type.clone(),
                name: method.name.clone(),
                name_origin: origin(method.name_span),
                signature: &method.signature,
                body,
                module: declaration.module,
                signature_module: declaration.module,
            },
        )?);
    }

    let explicit = impl_metadata
        .methods
        .iter()
        .map(|method| method.name.as_str())
        .collect::<BTreeSet<_>>();
    if let Some(trait_path) = impl_metadata.trait_path() {
        let trait_declaration = trait_declaration(graph, declaration.module, trait_path)
            .ok_or_else(|| {
                catalog_error(
                    declaration.id,
                    None,
                    owner_origin,
                    format!("trait `{}` has no declaration", trait_path.join("::")),
                )
            })?;
        let trait_metadata = graph.declaration(trait_declaration).ok_or_else(|| {
            catalog_error(
                declaration.id,
                None,
                owner_origin,
                "resolved trait has no declaration metadata",
            )
        })?;
        let shape = graph.trait_shape(trait_declaration).ok_or_else(|| {
            catalog_error(
                declaration.id,
                None,
                owner_origin,
                "resolved trait has no method shape",
            )
        })?;
        for method in &shape.methods {
            if explicit.contains(method.name.as_str()) {
                continue;
            }
            if !method.has_default {
                continue;
            }
            let node = method.default_body_node.ok_or_else(|| {
                catalog_error(
                    declaration.id,
                    None,
                    origin(method.span),
                    format!("default method `{}` has no body node", method.name),
                )
            })?;
            let body = graph.trait_default_method_body(node).ok_or_else(|| {
                catalog_error(
                    declaration.id,
                    Some(node),
                    origin(method.default_body_span.unwrap_or(method.span)),
                    format!("default method `{}` has no owning HIR body", method.name),
                )
            })?;
            methods.push(build_method(
                graph,
                declaration.id,
                &actual_module,
                identity_namespace,
                impl_metadata,
                owner_origin,
                MethodBuildInput {
                    node,
                    target_type: target_type.clone(),
                    name: method.name.clone(),
                    name_origin: origin(method.name_span),
                    signature: &method.signature,
                    body,
                    module: declaration.module,
                    signature_module: trait_metadata.module,
                },
            )?);
        }
    }
    Ok(methods)
}

fn build_method(
    graph: &ModuleGraph,
    declaration: HirDeclId,
    actual_module: &ModulePath,
    identity_namespace: Option<&str>,
    impl_metadata: &ImplMetadata,
    owner_origin: HirSourceOrigin,
    input: MethodBuildInput<'_>,
) -> Result<ScriptMethod, ScriptMethodCatalogError> {
    match input.body.owner {
        crate::body::HirBodyOwner::TraitDefaultMethod(node)
        | crate::body::HirBodyOwner::ImplMethod(node)
            if node == input.node => {}
        _ => {
            return Err(catalog_error(
                declaration,
                Some(input.node),
                input.body.origin,
                "method body owner does not match its method node",
            ));
        }
    }
    if input.body.params.len() != input.signature.params.len() {
        return Err(catalog_error(
            declaration,
            Some(input.node),
            input.body.origin,
            format!(
                "method signature has {} parameters but its body has {}",
                input.signature.params.len(),
                input.body.params.len()
            ),
        ));
    }
    let parameter_default_bodies = input
        .body
        .params
        .iter()
        .map(|parameter| parameter.default_body)
        .collect::<Vec<_>>();
    for default_body in parameter_default_bodies.iter().flatten() {
        let default = graph.body(*default_body).ok_or_else(|| {
            catalog_error(
                declaration,
                Some(input.node),
                input.body.origin,
                format!("parameter default body {default_body:?} is missing"),
            )
        })?;
        if !matches!(
            default.owner,
            crate::body::HirBodyOwner::ParameterDefault { parent, .. }
                if parent == input.body.id
        ) {
            return Err(catalog_error(
                declaration,
                Some(input.node),
                default.origin,
                format!("parameter default body {default_body:?} has the wrong owner"),
            ));
        }
    }
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
    Ok(ScriptMethod {
        owner: ScriptMethodOwner {
            target_type: input.target_type,
            actual_module: actual_module.clone(),
            identity,
        },
        node: input.node,
        name: input.name,
        body: input.body.id,
        signature: input.signature.clone(),
        parameter_default_bodies,
        module: input.module,
        signature_module: input.signature_module,
        origin: input.body.origin,
        name_origin: input.name_origin,
        owner_origin,
    })
}

fn catalog_error(
    declaration: HirDeclId,
    node: Option<HirNodeId>,
    origin: HirSourceOrigin,
    message: impl Into<String>,
) -> ScriptMethodCatalogError {
    ScriptMethodCatalogError {
        declaration,
        node,
        origin,
        message: message.into(),
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
