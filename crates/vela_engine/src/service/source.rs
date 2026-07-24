//! Schema-linked sparse service declarations from Vela HIR.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use vela_bytecode::{ExecutableGenerationId, LinkedArtifact};
use vela_common::{CallableAsyncness, ServiceGenerationId, Span};
use vela_def::{FunctionId, script_function_id};
use vela_hir::ids::{HirBodyId, HirDeclId, HirNodeId, ModuleId};
use vela_hir::module_graph::ModuleGraph;
use vela_hir::service_impl::{ServiceImplCatalog, ServiceImplCatalogError};
use vela_hir::type_hint::FunctionSignature;
use vela_vm::error::VmResult;

use super::{
    ServiceMethodKey, ServiceMethodSelection, ServiceMethodUpdate, ServiceSchema,
    ServiceSelectionError, ServiceSelectionTable, ServiceSetSchema,
};
use crate::native::EffectSet;
use crate::runtime::{CallArgs, CallOptions, Runtime, RuntimeBuildError, VelaValue};

/// One Vela method body resolved against an imported Rust service schema.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VelaServiceMethod {
    implementation: String,
    declaration: HirDeclId,
    node: HirNodeId,
    body: HirBodyId,
    module: ModuleId,
    signature: FunctionSignature,
    effect_ceiling: EffectSet,
    function: FunctionId,
    symbol: String,
    span: Span,
}

impl VelaServiceMethod {
    #[must_use]
    pub fn implementation(&self) -> &str {
        &self.implementation
    }

    #[must_use]
    pub const fn declaration(&self) -> HirDeclId {
        self.declaration
    }

    #[must_use]
    pub const fn node(&self) -> HirNodeId {
        self.node
    }

    #[must_use]
    pub const fn body(&self) -> HirBodyId {
        self.body
    }

    #[must_use]
    pub const fn module(&self) -> ModuleId {
        self.module
    }

    #[must_use]
    pub const fn signature(&self) -> &FunctionSignature {
        &self.signature
    }

    #[must_use]
    pub const fn effect_ceiling(&self) -> EffectSet {
        self.effect_ceiling
    }

    #[must_use]
    pub const fn function(&self) -> FunctionId {
        self.function
    }

    #[must_use]
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    #[must_use]
    pub const fn span(&self) -> Span {
        self.span
    }

    pub fn call<'host>(
        &self,
        runtime: &mut Runtime,
        args: CallArgs<'host>,
        options: CallOptions,
    ) -> VmResult<VelaValue> {
        runtime.call_stable_function(self.function, self.symbol.clone(), args, options)
    }
}

/// One schema-linked Vela method retained with the exact artifact that
/// satisfied its stable target identity and compiled signature.
#[derive(Clone, Debug)]
pub struct LinkedVelaServiceMethod {
    method: VelaServiceMethod,
    artifact: Arc<LinkedArtifact>,
}

impl LinkedVelaServiceMethod {
    #[must_use]
    pub const fn method(&self) -> &VelaServiceMethod {
        &self.method
    }

    #[must_use]
    pub fn artifact(&self) -> &Arc<LinkedArtifact> {
        &self.artifact
    }

    pub fn with_runtime<C, R>(
        &self,
        context: &mut C,
        invoke: impl FnOnce(&mut Runtime, &mut C) -> R,
    ) -> Result<R, RuntimeBuildError>
    where
        C: super::ServiceRuntimeAuthority,
    {
        context.with_service_runtime(&self.artifact, invoke)
    }

    fn rebind(&self, artifact: Arc<LinkedArtifact>) -> Result<Self, ServiceSourceError> {
        validate_compiled_target(&self.method, &artifact)?;
        Ok(Self {
            method: self.method.clone(),
            artifact,
        })
    }
}

impl PartialEq for LinkedVelaServiceMethod {
    fn eq(&self, other: &Self) -> bool {
        self.method == other.method && self.artifact.generation() == other.artifact.generation()
    }
}

impl Eq for LinkedVelaServiceMethod {}

/// Sparse source claims resolved to stable service and method IDs.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ServiceSourceManifest {
    updates: Vec<ServiceMethodUpdate<VelaServiceMethod>>,
}

impl ServiceSourceManifest {
    pub fn link(
        graph: &ModuleGraph,
        schema: &ServiceSetSchema,
    ) -> Result<Self, ServiceSourceError> {
        let catalog = ServiceImplCatalog::from_graph(graph).map_err(ServiceSourceError::catalog)?;
        let services = schema
            .services()
            .iter()
            .map(|service| (service.path(), service))
            .collect::<BTreeMap<_, _>>();
        let mut claims = BTreeMap::new();
        let mut updates = Vec::new();

        for implementation in catalog.implementations() {
            let service_path = implementation.service_path_text();
            let service = services.get(service_path.as_str()).ok_or_else(|| {
                ServiceSourceError::new(
                    implementation.span(),
                    ServiceSourceErrorKind::UnknownService {
                        service: service_path.clone(),
                    },
                )
            })?;
            if implementation.methods().len() == 0 {
                return Err(ServiceSourceError::new(
                    implementation.span(),
                    ServiceSourceErrorKind::EmptyImplementation {
                        service: service_path,
                    },
                ));
            }
            let implementation_path = implementation.implementation_path().join("::");
            let package = graph
                .module_package(implementation.module())
                .expect("catalogued service impl module has a package");
            for method in implementation.methods() {
                let descriptor = find_method(service, method.name()).ok_or_else(|| {
                    ServiceSourceError::new(
                        method.name_span(),
                        ServiceSourceErrorKind::UnknownMethod {
                            service: service.path().to_owned(),
                            method: method.name().to_owned(),
                        },
                    )
                })?;
                validate_signature(service, descriptor, method.signature(), method.name_span())?;
                let key = ServiceMethodKey::new(service.id(), descriptor.id);
                if let Some(previous) = claims.insert(key, method.name_span()) {
                    return Err(ServiceSourceError::new(
                        method.name_span(),
                        ServiceSourceErrorKind::DuplicateMethodClaim {
                            service: service.path().to_owned(),
                            method: method.name().to_owned(),
                            previous,
                        },
                    ));
                }
                let symbol = implementation.method_symbol(method);
                updates.push(ServiceMethodUpdate::vela(
                    service.id(),
                    descriptor.id,
                    service.abi_fingerprint(),
                    VelaServiceMethod {
                        implementation: implementation_path.clone(),
                        declaration: implementation.declaration(),
                        node: method.node(),
                        body: method.body(),
                        module: method.module(),
                        signature: method.signature().clone(),
                        effect_ceiling: descriptor.callable.effects,
                        function: script_function_id(package.as_str(), &symbol),
                        symbol,
                        span: method.origin().span,
                    },
                ));
            }
        }
        Ok(Self { updates })
    }

    pub fn updates(
        &self,
    ) -> impl ExactSizeIterator<Item = &ServiceMethodUpdate<VelaServiceMethod>> {
        self.updates.iter()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.updates.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.updates.is_empty()
    }

    pub fn into_snapshot(
        self,
        schema: &ServiceSetSchema,
    ) -> Result<ServiceSelectionTable<VelaServiceMethod>, ServiceSelectionError> {
        ServiceSelectionTable::snapshot(schema, self.updates)
    }

    /// Proves that every selected Vela method is present with the linked
    /// signature expected by this source manifest.
    ///
    /// Candidate construction must call this before retaining the artifact.
    /// A stable [`FunctionId`] alone is not sufficient because the caller may
    /// accidentally pair a manifest with an unrelated compile generation.
    pub fn validate_artifact(&self, artifact: &LinkedArtifact) -> Result<(), ServiceSourceError> {
        for update in &self.updates {
            let ServiceMethodSelection::Vela(target) = update.selection() else {
                continue;
            };
            validate_compiled_target(target, artifact)?;
        }
        Ok(())
    }

    /// Retains the exact validated artifact on every Vela target.
    pub fn bind_artifact(
        self,
        artifact: Arc<LinkedArtifact>,
    ) -> Result<LinkedServiceSourceManifest, ServiceSourceError> {
        self.validate_artifact(&artifact)?;
        let updates = self
            .updates
            .into_iter()
            .map(|update| {
                let selection = match update.selection() {
                    ServiceMethodSelection::RustDefault => ServiceMethodSelection::RustDefault,
                    ServiceMethodSelection::Vela(method) => {
                        ServiceMethodSelection::Vela(LinkedVelaServiceMethod {
                            method: method.clone(),
                            artifact: Arc::clone(&artifact),
                        })
                    }
                };
                ServiceMethodUpdate::new(update.key(), update.expected_service_abi(), selection)
            })
            .collect::<Vec<_>>();
        LinkedServiceSourceManifest::from_updates(updates)
    }

    pub fn into_updates(self) -> Vec<ServiceMethodUpdate<VelaServiceMethod>> {
        self.updates
    }
}

/// Sparse service updates whose Vela targets retain one validated linked
/// artifact.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LinkedServiceSourceManifest {
    updates: Vec<ServiceMethodUpdate<LinkedVelaServiceMethod>>,
}

impl LinkedServiceSourceManifest {
    pub fn from_updates(
        updates: impl IntoIterator<Item = ServiceMethodUpdate<LinkedVelaServiceMethod>>,
    ) -> Result<Self, ServiceSourceError> {
        let updates = updates.into_iter().collect::<Vec<_>>();
        validate_linked_artifacts(&updates)?;
        Ok(Self { updates })
    }

    pub fn updates(
        &self,
    ) -> impl ExactSizeIterator<Item = &ServiceMethodUpdate<LinkedVelaServiceMethod>> {
        self.updates.iter()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.updates.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.updates.is_empty()
    }

    pub fn into_snapshot(
        self,
        schema: &ServiceSetSchema,
    ) -> Result<ServiceSelectionTable<LinkedVelaServiceMethod>, ServiceSelectionError> {
        ServiceSelectionTable::snapshot(schema, self.updates)
    }

    pub fn into_delta(
        self,
        schema: &ServiceSetSchema,
        expected_base_generation: ServiceGenerationId,
        actual_base_generation: ServiceGenerationId,
        base: &ServiceSelectionTable<LinkedVelaServiceMethod>,
    ) -> Result<ServiceSelectionTable<LinkedVelaServiceMethod>, super::ServiceStagingError> {
        let artifact = linked_manifest_artifact(&self.updates).cloned();
        let rebound = if let Some(artifact) = artifact {
            base.try_map_vela(|target| target.rebind(Arc::clone(&artifact)))?
        } else {
            base.clone()
        };
        ServiceSelectionTable::delta(
            schema,
            expected_base_generation,
            actual_base_generation,
            &rebound,
            self.updates,
        )
        .map_err(super::ServiceStagingError::from)
    }

    pub fn into_updates(self) -> Vec<ServiceMethodUpdate<LinkedVelaServiceMethod>> {
        self.updates
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceSourceError {
    span: Span,
    kind: ServiceSourceErrorKind,
}

impl ServiceSourceError {
    const fn new(span: Span, kind: ServiceSourceErrorKind) -> Self {
        Self { span, kind }
    }

    fn catalog(error: ServiceImplCatalogError) -> Self {
        Self {
            span: error.span(),
            kind: ServiceSourceErrorKind::InvalidDeclaration(error.to_string()),
        }
    }

    #[must_use]
    pub const fn span(&self) -> Span {
        self.span
    }

    #[must_use]
    pub const fn kind(&self) -> &ServiceSourceErrorKind {
        &self.kind
    }

    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self.kind {
            ServiceSourceErrorKind::InvalidDeclaration(_) => "service.source.invalid_declaration",
            ServiceSourceErrorKind::UnknownService { .. } => "service.source.unknown_service",
            ServiceSourceErrorKind::EmptyImplementation { .. } => {
                "service.source.empty_implementation"
            }
            ServiceSourceErrorKind::UnknownMethod { .. } => "service.source.unknown_method",
            ServiceSourceErrorKind::DuplicateMethodClaim { .. } => {
                "service.source.duplicate_method"
            }
            ServiceSourceErrorKind::AsyncnessMismatch { .. } => "service.source.asyncness_mismatch",
            ServiceSourceErrorKind::ParameterCountMismatch { .. } => {
                "service.source.parameter_count"
            }
            ServiceSourceErrorKind::ParameterDefaultUnsupported { .. } => {
                "service.source.parameter_default"
            }
            ServiceSourceErrorKind::MissingCompiledTarget { .. } => {
                "service.source.missing_compiled_target"
            }
            ServiceSourceErrorKind::CompiledAsyncnessMismatch { .. } => {
                "service.source.compiled_asyncness_mismatch"
            }
            ServiceSourceErrorKind::CompiledParameterCountMismatch { .. } => {
                "service.source.compiled_parameter_count"
            }
            ServiceSourceErrorKind::MixedLinkedArtifacts { .. } => {
                "service.source.mixed_linked_artifacts"
            }
            ServiceSourceErrorKind::EffectCeilingExceeded { .. } => "service.source.effect_ceiling",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServiceSourceErrorKind {
    InvalidDeclaration(String),
    UnknownService {
        service: String,
    },
    EmptyImplementation {
        service: String,
    },
    UnknownMethod {
        service: String,
        method: String,
    },
    DuplicateMethodClaim {
        service: String,
        method: String,
        previous: Span,
    },
    AsyncnessMismatch {
        service: String,
        method: String,
        expected: CallableAsyncness,
        actual: CallableAsyncness,
    },
    ParameterCountMismatch {
        service: String,
        method: String,
        expected: usize,
        actual: usize,
    },
    ParameterDefaultUnsupported {
        service: String,
        method: String,
        parameter: String,
    },
    MissingCompiledTarget {
        symbol: String,
        function: FunctionId,
    },
    CompiledAsyncnessMismatch {
        symbol: String,
        expected: CallableAsyncness,
        actual: CallableAsyncness,
    },
    CompiledParameterCountMismatch {
        symbol: String,
        expected: usize,
        actual: usize,
    },
    MixedLinkedArtifacts {
        expected: ExecutableGenerationId,
        actual: ExecutableGenerationId,
    },
    EffectCeilingExceeded {
        symbol: String,
        allowed: EffectSet,
        observed: EffectSet,
    },
}

impl fmt::Display for ServiceSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ServiceSourceErrorKind::InvalidDeclaration(reason) => {
                write!(formatter, "invalid service implementation: {reason}")
            }
            ServiceSourceErrorKind::UnknownService { service } => {
                write!(formatter, "unknown imported Rust service `{service}`")
            }
            ServiceSourceErrorKind::EmptyImplementation { service } => {
                write!(
                    formatter,
                    "service implementation for `{service}` has no methods"
                )
            }
            ServiceSourceErrorKind::UnknownMethod { service, method } => {
                write!(formatter, "service `{service}` has no method `{method}`")
            }
            ServiceSourceErrorKind::DuplicateMethodClaim {
                service, method, ..
            } => write!(
                formatter,
                "service method `{service}::{method}` is implemented more than once"
            ),
            ServiceSourceErrorKind::AsyncnessMismatch {
                service,
                method,
                expected,
                actual,
            } => write!(
                formatter,
                "service method `{service}::{method}` expects {expected:?}, found {actual:?}"
            ),
            ServiceSourceErrorKind::ParameterCountMismatch {
                service,
                method,
                expected,
                actual,
            } => write!(
                formatter,
                "service method `{service}::{method}` expects {expected} parameters, found {actual}"
            ),
            ServiceSourceErrorKind::ParameterDefaultUnsupported {
                service,
                method,
                parameter,
            } => write!(
                formatter,
                "service method `{service}::{method}` parameter `{parameter}` cannot declare a default"
            ),
            ServiceSourceErrorKind::MissingCompiledTarget { symbol, function } => write!(
                formatter,
                "linked artifact does not contain service target `{symbol}` ({function:?})"
            ),
            ServiceSourceErrorKind::CompiledAsyncnessMismatch {
                symbol,
                expected,
                actual,
            } => write!(
                formatter,
                "compiled service target `{symbol}` expects {expected:?}, found {actual:?}"
            ),
            ServiceSourceErrorKind::CompiledParameterCountMismatch {
                symbol,
                expected,
                actual,
            } => write!(
                formatter,
                "compiled service target `{symbol}` expects {expected} parameters, found {actual}"
            ),
            ServiceSourceErrorKind::MixedLinkedArtifacts { expected, actual } => write!(
                formatter,
                "one service update mixes linked artifact generations {} and {}",
                expected.get(),
                actual.get()
            ),
            ServiceSourceErrorKind::EffectCeilingExceeded {
                symbol,
                allowed,
                observed,
            } => write!(
                formatter,
                "compiled service target `{symbol}` effects 0x{:x} exceed Rust ceiling 0x{:x}",
                observed.bits(),
                allowed.bits()
            ),
        }
    }
}

impl std::error::Error for ServiceSourceError {}

fn find_method<'schema>(
    service: &'schema ServiceSchema,
    method_name: &str,
) -> Option<&'schema super::ServiceMethodDescriptor> {
    service
        .methods()
        .iter()
        .find(|method| method.path.rsplit("::").next() == Some(method_name))
}

fn validate_signature(
    service: &ServiceSchema,
    method: &super::ServiceMethodDescriptor,
    actual: &FunctionSignature,
    span: Span,
) -> Result<(), ServiceSourceError> {
    let expected = &method.callable;
    if actual.asyncness != expected.asyncness {
        return Err(ServiceSourceError::new(
            span,
            ServiceSourceErrorKind::AsyncnessMismatch {
                service: service.path().to_owned(),
                method: method.path.clone(),
                expected: expected.asyncness,
                actual: actual.asyncness,
            },
        ));
    }
    if actual.params.len() != expected.parameters.len() {
        return Err(ServiceSourceError::new(
            span,
            ServiceSourceErrorKind::ParameterCountMismatch {
                service: service.path().to_owned(),
                method: method.path.clone(),
                expected: expected.parameters.len(),
                actual: actual.params.len(),
            },
        ));
    }
    if let Some(parameter) = actual
        .params
        .iter()
        .find(|parameter| parameter.default_value_span.is_some())
    {
        return Err(ServiceSourceError::new(
            parameter.default_value_span.unwrap_or(parameter.span),
            ServiceSourceErrorKind::ParameterDefaultUnsupported {
                service: service.path().to_owned(),
                method: method.path.clone(),
                parameter: parameter.name.clone(),
            },
        ));
    }
    Ok(())
}

fn validate_compiled_target(
    target: &VelaServiceMethod,
    artifact: &LinkedArtifact,
) -> Result<(), ServiceSourceError> {
    let Some(_handle) = artifact.program().entry_point_by_id(target.function()) else {
        return Err(missing_compiled_target(target));
    };
    let Some(verified) = artifact.verified_mir().root(target.function()) else {
        return Err(missing_compiled_target(target));
    };
    let Some(function_id) = verified.program().function_by_id(target.function()) else {
        return Err(missing_compiled_target(target));
    };
    let Some(function) = verified.program().function(function_id) else {
        return Err(missing_compiled_target(target));
    };
    if function.asyncness() != target.signature().asyncness {
        return Err(ServiceSourceError::new(
            target.span(),
            ServiceSourceErrorKind::CompiledAsyncnessMismatch {
                symbol: target.symbol().to_owned(),
                expected: target.signature().asyncness,
                actual: function.asyncness(),
            },
        ));
    }
    if function.parameters().len() != target.signature().params.len() {
        return Err(ServiceSourceError::new(
            target.span(),
            ServiceSourceErrorKind::CompiledParameterCountMismatch {
                symbol: target.symbol().to_owned(),
                expected: target.signature().params.len(),
                actual: function.parameters().len(),
            },
        ));
    }
    let observed = service_effects(function);
    if !target.effect_ceiling().contains_all(observed) {
        return Err(ServiceSourceError::new(
            target.span(),
            ServiceSourceErrorKind::EffectCeilingExceeded {
                symbol: target.symbol().to_owned(),
                allowed: target.effect_ceiling(),
                observed,
            },
        ));
    }
    Ok(())
}

fn missing_compiled_target(target: &VelaServiceMethod) -> ServiceSourceError {
    ServiceSourceError::new(
        target.span(),
        ServiceSourceErrorKind::MissingCompiledTarget {
            symbol: target.symbol().to_owned(),
            function: target.function(),
        },
    )
}

fn validate_linked_artifacts(
    updates: &[ServiceMethodUpdate<LinkedVelaServiceMethod>],
) -> Result<(), ServiceSourceError> {
    let Some(expected) = linked_manifest_artifact(updates) else {
        return Ok(());
    };
    for update in updates {
        let ServiceMethodSelection::Vela(target) = update.selection() else {
            continue;
        };
        if target.artifact().generation() != expected.generation() {
            return Err(ServiceSourceError::new(
                target.method().span(),
                ServiceSourceErrorKind::MixedLinkedArtifacts {
                    expected: expected.generation(),
                    actual: target.artifact().generation(),
                },
            ));
        }
    }
    Ok(())
}

fn linked_manifest_artifact(
    updates: &[ServiceMethodUpdate<LinkedVelaServiceMethod>],
) -> Option<&Arc<LinkedArtifact>> {
    updates.iter().find_map(|update| match update.selection() {
        ServiceMethodSelection::RustDefault => None,
        ServiceMethodSelection::Vela(target) => Some(target.artifact()),
    })
}

fn service_effects(function: &vela_mir::MirFunction) -> EffectSet {
    let mut observed = vela_mir::MirEffect::PURE;
    for (_, statement) in function.statements() {
        observed = observed.union(statement.effect);
    }
    for (_, block) in function.blocks() {
        if let Some(terminator) = block.terminator() {
            observed = observed.union(terminator.effect);
        }
    }

    let mut effects = EffectSet::pure();
    effects = if observed.host_write {
        effects.union(EffectSet::host_write())
    } else if observed.host_read {
        effects.union(EffectSet::host_read())
    } else {
        effects
    };
    if observed.emits_event {
        effects = effects.union(EffectSet::event_emit());
    }
    if observed.reads_time {
        effects = effects.union(EffectSet::time());
    }
    if observed.uses_random {
        effects = effects.union(EffectSet::random());
    }
    if observed.reads_io {
        effects = effects.union(EffectSet::io_read());
    }
    if observed.writes_io {
        effects = effects.union(EffectSet::io_write());
    }
    if observed.reflection_read {
        effects = effects.union(EffectSet::reflection_read());
    }
    if observed.reflection_write {
        effects = effects.union(EffectSet::reflection_write());
    }
    if observed.reflection_call {
        effects = effects.union(EffectSet::reflection_call());
    }
    effects
}
