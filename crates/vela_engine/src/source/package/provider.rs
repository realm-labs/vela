use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::PathBuf;

use vela_analysis::facts::AnalysisFacts;
use vela_analysis::registry::{RegistryEffectFact, RegistryFacts};
use vela_common::{CallableAsyncness, Capability, CapabilitySet};
use vela_def::{MethodId, TraitId, TypeId};
use vela_hir::provider::{ProviderKey, discover_providers};

use super::{
    EnginePackageError, EnginePackageErrorKind, PackageCompilationSnapshot,
    PackageCompilationSnapshotId, package_error,
};
use crate::engine::Engine;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderSourceLocation {
    path: PathBuf,
    start: u32,
    end: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderMethodDescriptor {
    id: MethodId,
    name: String,
    asyncness: CallableAsyncness,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderDescriptor {
    key: ProviderKey,
    provider_type: TypeId,
    methods: Box<[ProviderMethodDescriptor]>,
    package_declared_capabilities: CapabilitySet,
    package_statically_observed_capabilities: CapabilitySet,
    source: ProviderSourceLocation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderCatalog {
    snapshot: PackageCompilationSnapshotId,
    providers: Box<[ProviderDescriptor]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderSelection {
    snapshot: PackageCompilationSnapshotId,
    providers: BTreeSet<ProviderKey>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderCatalogError {
    UnknownProvider {
        key: ProviderKey,
    },
    SnapshotMismatch {
        expected: PackageCompilationSnapshotId,
        actual: PackageCompilationSnapshotId,
    },
}

impl ProviderSourceLocation {
    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    #[must_use]
    pub const fn start(&self) -> u32 {
        self.start
    }

    #[must_use]
    pub const fn end(&self) -> u32 {
        self.end
    }
}

impl ProviderMethodDescriptor {
    #[must_use]
    pub const fn id(&self) -> MethodId {
        self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn asyncness(&self) -> CallableAsyncness {
        self.asyncness
    }
}

impl ProviderDescriptor {
    #[must_use]
    pub const fn key(&self) -> &ProviderKey {
        &self.key
    }

    #[must_use]
    pub const fn provider_type(&self) -> TypeId {
        self.provider_type
    }

    #[must_use]
    pub fn methods(&self) -> &[ProviderMethodDescriptor] {
        &self.methods
    }

    #[must_use]
    pub const fn package_declared_capabilities(&self) -> CapabilitySet {
        self.package_declared_capabilities
    }

    #[must_use]
    pub const fn package_statically_observed_capabilities(&self) -> CapabilitySet {
        self.package_statically_observed_capabilities
    }

    #[must_use]
    pub const fn source(&self) -> &ProviderSourceLocation {
        &self.source
    }
}

impl ProviderCatalog {
    #[must_use]
    pub const fn snapshot(&self) -> PackageCompilationSnapshotId {
        self.snapshot
    }

    #[must_use]
    pub fn providers(&self) -> &[ProviderDescriptor] {
        &self.providers
    }

    #[must_use]
    pub fn providers_for(&self, service: TraitId) -> Vec<&ProviderDescriptor> {
        self.providers
            .iter()
            .filter(|provider| provider.key.service() == service)
            .collect()
    }

    pub fn select(
        &self,
        providers: impl IntoIterator<Item = ProviderKey>,
    ) -> Result<ProviderSelection, ProviderCatalogError> {
        let providers = providers.into_iter().collect::<BTreeSet<_>>();
        if let Some(key) = providers
            .iter()
            .find(|key| !self.providers.iter().any(|provider| provider.key() == *key))
        {
            return Err(ProviderCatalogError::UnknownProvider { key: key.clone() });
        }
        Ok(ProviderSelection {
            snapshot: self.snapshot,
            providers,
        })
    }

    pub fn validate_selection(
        &self,
        selection: &ProviderSelection,
    ) -> Result<(), ProviderCatalogError> {
        if selection.snapshot != self.snapshot {
            return Err(ProviderCatalogError::SnapshotMismatch {
                expected: self.snapshot,
                actual: selection.snapshot,
            });
        }
        if let Some(key) = selection
            .providers
            .iter()
            .find(|key| !self.providers.iter().any(|provider| provider.key() == *key))
        {
            return Err(ProviderCatalogError::UnknownProvider { key: key.clone() });
        }
        Ok(())
    }
}

impl ProviderSelection {
    pub(super) fn for_snapshot(
        snapshot: PackageCompilationSnapshotId,
        providers: impl IntoIterator<Item = ProviderKey>,
    ) -> Self {
        Self {
            snapshot,
            providers: providers.into_iter().collect(),
        }
    }

    #[must_use]
    pub const fn snapshot(&self) -> PackageCompilationSnapshotId {
        self.snapshot
    }

    #[must_use]
    pub const fn providers(&self) -> &BTreeSet<ProviderKey> {
        &self.providers
    }
}

impl fmt::Display for ProviderCatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownProvider { key } => write!(
                formatter,
                "unknown provider `{}` for service {:?} in package `{}`",
                key.provider(),
                key.service(),
                key.package()
            ),
            Self::SnapshotMismatch { expected, actual } => write!(
                formatter,
                "provider selection belongs to snapshot {} but catalog {} was supplied",
                actual.get(),
                expected.get()
            ),
        }
    }
}

impl std::error::Error for ProviderCatalogError {}

impl Engine {
    pub fn discover_providers(
        &self,
        snapshot: &PackageCompilationSnapshot,
    ) -> Result<ProviderCatalog, EnginePackageError> {
        let discovered = discover_providers(snapshot.sources.graph())
            .map_err(|error| package_error(EnginePackageErrorKind::ProviderDiscovery(error)))?;
        let observed = observed_capabilities(self, snapshot)?;
        for provider in &discovered {
            let package = snapshot
                .package_graph
                .packages()
                .get(provider.key.package())
                .expect("discovered provider belongs to a loaded package");
            let package_observed = observed
                .get(provider.key.package())
                .copied()
                .unwrap_or_default();
            let missing = package_observed.difference(package.required_capabilities);
            if !missing.is_empty() {
                return Err(package_error(
                    EnginePackageErrorKind::UndeclaredCapabilities {
                        package: provider.key.package().clone(),
                        missing,
                    },
                ));
            }
        }
        let sources = snapshot.package_graph.sources().sources();
        let providers = discovered
            .into_iter()
            .map(|provider| {
                let package = snapshot
                    .package_graph
                    .packages()
                    .get(provider.key.package())
                    .expect("discovered provider belongs to a loaded package");
                let source_index = usize::try_from(provider.source.source.get() - 1)
                    .expect("source ID fits usize");
                let source = sources
                    .get(source_index)
                    .expect("provider source belongs to the package snapshot");
                let package_observed = observed
                    .get(provider.key.package())
                    .copied()
                    .unwrap_or_default();
                ProviderDescriptor {
                    key: provider.key,
                    provider_type: provider.provider_type,
                    methods: provider
                        .methods
                        .into_iter()
                        .map(|method| ProviderMethodDescriptor {
                            id: method.id,
                            name: method.name,
                            asyncness: method.asyncness,
                        })
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                    package_declared_capabilities: package.required_capabilities,
                    package_statically_observed_capabilities: package_observed,
                    source: ProviderSourceLocation {
                        path: source.path.clone(),
                        start: provider.source.start,
                        end: provider.source.end,
                    },
                }
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Ok(ProviderCatalog {
            snapshot: snapshot.id,
            providers,
        })
    }
}

fn observed_capabilities(
    engine: &Engine,
    snapshot: &PackageCompilationSnapshot,
) -> Result<BTreeMap<vela_package::PackageId, CapabilitySet>, EnginePackageError> {
    let graph = snapshot.sources.graph();
    let schema = RegistryFacts::from_compile_view(engine.compiler_registry()).map_err(|error| {
        package_error(EnginePackageErrorKind::ProviderAnalysis(format!(
            "provider analysis registry is invalid: {error}"
        )))
    })?;
    let facts = AnalysisFacts::from_module_graph_and_schema(graph, &schema);
    let mut observed = BTreeMap::<vela_package::PackageId, CapabilitySet>::new();
    for body in graph.bodies() {
        let Some(package) = graph.source_package(body.origin.source) else {
            continue;
        };
        let capabilities = observed.entry(package.clone()).or_default();
        for expression in body.expressions.values() {
            if let Some(effect) = facts.effect(expression.id) {
                insert_effect_capabilities(capabilities, effect);
            }
            if facts.host_path_target(expression.id).is_some() {
                capabilities.insert(Capability::HostRead);
            }
            if let vela_hir::body::HirExprKind::Assign {
                target: Some(target),
                ..
            } = &expression.kind
                && facts.host_path_target(*target).is_some()
            {
                capabilities.insert(Capability::HostWrite);
            }
        }
        for path in body.paths.values() {
            let [namespace, operation, ..] = path.path.as_slice() else {
                continue;
            };
            let capability = match namespace.as_str() {
                "time" => Some(Capability::Time),
                "random" => Some(Capability::Random),
                "event" => Some(Capability::EventEmit),
                "io" if operation.starts_with("write") => Some(Capability::IoWrite),
                "io" => Some(Capability::IoRead),
                "reflect" if operation == "set" => Some(Capability::ReflectionWrite),
                "reflect" if operation == "call" => Some(Capability::ReflectionCall),
                "reflect" => Some(Capability::ReflectionRead),
                _ => None,
            };
            if let Some(capability) = capability {
                capabilities.insert(capability);
            }
        }
    }
    Ok(observed)
}

fn insert_effect_capabilities(capabilities: &mut CapabilitySet, effect: &RegistryEffectFact) {
    for (present, capability) in [
        (effect.reads_host, Capability::HostRead),
        (effect.writes_host, Capability::HostWrite),
        (effect.emits_events, Capability::EventEmit),
        (effect.reads_time, Capability::Time),
        (effect.uses_random, Capability::Random),
        (effect.reads_io, Capability::IoRead),
        (effect.writes_io, Capability::IoWrite),
        (effect.reads_reflection, Capability::ReflectionRead),
        (effect.writes_reflection, Capability::ReflectionWrite),
        (effect.calls_reflection, Capability::ReflectionCall),
    ] {
        if present {
            capabilities.insert(capability);
        }
    }
}
