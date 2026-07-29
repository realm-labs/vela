//! Whole-generation publication for generated Rust/Vela service domains.

mod deployment;
#[cfg(feature = "artifact-codec")]
mod portable;
mod runtime;
mod schema;
mod selection;
mod source;
mod workspace;

pub use deployment::{
    ServiceBundleChecksum, ServiceBundleError, ServiceDryRunReport, ServicePackageIdentity,
    ServiceSelectionSummary, ServiceUpdateBundle, ServiceUpdateMetadata, ServiceUpdateMode,
};
#[cfg(feature = "artifact-codec")]
pub use portable::{
    PortableDiagnosticSource, PortableServiceBundleChecksum, PortableServiceBundleError,
    PortableServiceUpdateBundle,
};
pub use runtime::{
    ServiceCallDispatcher, ServiceCallTarget, ServiceFuture, ServiceInvocationError,
    ServiceRuntimeAuthority, ServiceRuntimeBinding, ServiceRuntimeLease, ServiceRuntimeSlot,
};
pub use schema::{
    ServiceMethodDescriptor, ServiceSchema, ServiceSchemaError, ServiceSetSchema,
    ServiceSetSchemaFactory, ServiceTypeRequirement,
};
pub use selection::{
    ServiceMethodKey, ServiceMethodSelection, ServiceMethodUpdate, ServiceSelectionError,
    ServiceSelectionTable,
};
pub use source::{
    LinkedServiceSourceManifest, LinkedVelaServiceMethod, ServiceSourceError,
    ServiceSourceErrorKind, ServiceSourceManifest, VelaServiceMethod,
};
pub use vela_bytecode::{ArtifactChecksum, LinkedArtifact};
pub use workspace::{
    PatchEdit, PatchRevision, PatchRevisionChecksum, PatchSources, ServicePatch, ServicePatchState,
    ServicePatchStateRollback, ServicePatchWorkspaceError,
};

use std::fmt;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use arc_swap::ArcSwap;
use vela_common::{ServiceGenerationId, ServiceSetId};

static NEXT_CONTROLLER_ID: AtomicU64 = AtomicU64::new(1);

/// Declaration-only marker used by `#[vela_macros::service_domain]`.
///
/// A service domain struct is macro input rather than a runtime value with
/// fields. `Service<dyn Trait>` makes that schema role explicit in source.
pub struct Service<T: ?Sized>(PhantomData<fn() -> T>);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServiceDomainBuildError {
    MissingDefault {
        domain: &'static str,
        service: &'static str,
    },
    ContextTypeMismatch {
        expected: &'static str,
        actual: &'static str,
    },
    Engine(crate::error::EngineError),
    Schema(ServiceSchemaError),
}

impl fmt::Display for ServiceDomainBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingDefault { domain, service } => {
                write!(
                    formatter,
                    "service domain `{domain}` is missing Rust default `{service}`"
                )
            }
            Self::ContextTypeMismatch { expected, actual } => {
                write!(
                    formatter,
                    "service domain expects Runtime context `{expected}`, found `{actual}`"
                )
            }
            Self::Engine(error) => write!(formatter, "service domain engine failed: {error}"),
            Self::Schema(error) => write!(formatter, "service domain schema failed: {error}"),
        }
    }
}

impl std::error::Error for ServiceDomainBuildError {}

impl From<crate::error::EngineError> for ServiceDomainBuildError {
    fn from(error: crate::error::EngineError) -> Self {
        Self::Engine(error)
    }
}

impl From<ServiceSchemaError> for ServiceDomainBuildError {
    fn from(error: ServiceSchemaError) -> Self {
        Self::Schema(error)
    }
}

#[derive(Debug)]
pub enum ServicePatchError {
    MissingRuntimeAuthority {
        domain: &'static str,
    },
    Workspace(ServicePatchWorkspaceError),
    SourceIngestion(String),
    Compile(crate::source::EngineSourceError),
    Link(vela_bytecode::LinkError),
    Source(ServiceSourceError),
    Bundle(ServiceBundleError),
    #[cfg(feature = "artifact-codec")]
    Portable(PortableServiceBundleError),
    Staging(ServiceStagingError),
    Publication(ServicePublicationError),
}

impl fmt::Display for ServicePatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingRuntimeAuthority { domain } => {
                write!(
                    formatter,
                    "service domain `{domain}` has no actor Runtime authority"
                )
            }
            Self::Workspace(error) => write!(formatter, "service patch workspace failed: {error}"),
            Self::SourceIngestion(message) => {
                write!(
                    formatter,
                    "service patch source ingestion failed: {message}"
                )
            }
            Self::Compile(error) => write!(formatter, "service patch compilation failed: {error}"),
            Self::Link(error) => write!(formatter, "service patch linking failed: {error}"),
            Self::Source(error) => write!(formatter, "service patch manifest failed: {error}"),
            Self::Bundle(error) => write!(formatter, "service patch bundle failed: {error}"),
            #[cfg(feature = "artifact-codec")]
            Self::Portable(error) => {
                write!(formatter, "portable service patch bundle failed: {error}")
            }
            Self::Staging(error) => write!(formatter, "service patch staging failed: {error}"),
            Self::Publication(error) => {
                write!(formatter, "service patch publication failed: {error}")
            }
        }
    }
}

impl std::error::Error for ServicePatchError {}

impl From<ServicePatchWorkspaceError> for ServicePatchError {
    fn from(error: ServicePatchWorkspaceError) -> Self {
        Self::Workspace(error)
    }
}

impl From<crate::source::EngineSourceError> for ServicePatchError {
    fn from(error: crate::source::EngineSourceError) -> Self {
        Self::Compile(error)
    }
}

impl From<vela_bytecode::LinkError> for ServicePatchError {
    fn from(error: vela_bytecode::LinkError) -> Self {
        Self::Link(error)
    }
}

impl From<ServiceSourceError> for ServicePatchError {
    fn from(error: ServiceSourceError) -> Self {
        Self::Source(error)
    }
}

impl From<ServiceBundleError> for ServicePatchError {
    fn from(error: ServiceBundleError) -> Self {
        Self::Bundle(error)
    }
}

#[cfg(feature = "artifact-codec")]
impl From<PortableServiceBundleError> for ServicePatchError {
    fn from(error: PortableServiceBundleError) -> Self {
        Self::Portable(error)
    }
}

#[cfg(feature = "artifact-codec")]
impl From<vela_bytecode::PortableArtifactError> for ServicePatchError {
    fn from(error: vela_bytecode::PortableArtifactError) -> Self {
        Self::Portable(PortableServiceBundleError::from(error))
    }
}

impl From<ServiceStagingError> for ServicePatchError {
    fn from(error: ServiceStagingError) -> Self {
        Self::Staging(error)
    }
}

impl From<ServicePublicationError> for ServicePatchError {
    fn from(error: ServicePublicationError) -> Self {
        Self::Publication(error)
    }
}

impl crate::engine::Engine {
    /// Compiles and links one complete virtual service patch workspace.
    pub fn compile_service_patch(
        &self,
        schema: &ServiceSetSchema,
        revision: &PatchRevision,
    ) -> Result<ServiceUpdateBundle, ServicePatchError> {
        let (manifest, compiled) = self.compile_service_patch_sources(schema, revision)?;
        let artifact = self.link_compiled_program(compiled)?;
        let update = manifest.bind_artifact(Arc::clone(&artifact))?;
        ServiceUpdateBundle::snapshot(schema, artifact, update).map_err(Into::into)
    }

    /// Compiles one complete virtual workspace into a source-independent
    /// transport bundle for another Engine with the same sealed schema.
    #[cfg(feature = "artifact-codec")]
    pub fn compile_portable_service_patch(
        &self,
        schema: &ServiceSetSchema,
        revision: &PatchRevision,
        host_schema_hash: u64,
    ) -> Result<PortableServiceUpdateBundle, ServicePatchError> {
        let (manifest, compiled) = self.compile_service_patch_sources(schema, revision)?;
        let artifact = vela_bytecode::PortableProgramArtifact::from_compiled(compiled)?;
        let diagnostics = revision
            .sources()
            .iter()
            .map(|(path, source)| PortableDiagnosticSource::new(path, source));
        PortableServiceUpdateBundle::snapshot(
            schema,
            artifact,
            &manifest,
            host_schema_hash,
            diagnostics,
        )
        .map_err(Into::into)
    }

    fn compile_service_patch_sources(
        &self,
        schema: &ServiceSetSchema,
        revision: &PatchRevision,
    ) -> Result<
        (
            ServiceSourceManifest,
            vela_bytecode::compiler::CompiledProgram,
        ),
        ServicePatchError,
    > {
        let module_sources = revision.sources().module_sources()?;
        let sources = vela_hir::source_ingestion::build_module_source_set(&module_sources)
            .map_err(|error| ServicePatchError::SourceIngestion(format!("{error:?}")))?;
        let manifest = ServiceSourceManifest::link(sources.graph(), schema)?;
        let compiled = self.compile_source_set(&sources)?;
        Ok((manifest, compiled))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ServiceControllerId(u64);

impl ServiceControllerId {
    fn next() -> Self {
        Self(NEXT_CONTROLLER_ID.fetch_add(1, Ordering::Relaxed))
    }
}

/// One complete immutable service-domain generation.
pub struct ServiceGeneration<T> {
    controller_id: ServiceControllerId,
    service_set_id: ServiceSetId,
    generation_id: ServiceGenerationId,
    services: T,
}

impl<T> ServiceGeneration<T> {
    #[must_use]
    pub const fn service_set_id(&self) -> ServiceSetId {
        self.service_set_id
    }

    #[must_use]
    pub fn generation_id(&self) -> ServiceGenerationId {
        self.generation_id
    }

    #[must_use]
    pub const fn services(&self) -> &T {
        &self.services
    }
}

impl<T> Deref for ServiceGeneration<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.services()
    }
}

/// A root-scoped pin of one exact service generation.
pub struct ServiceRoot<T> {
    generation: Arc<ServiceGeneration<T>>,
}

impl<T> Clone for ServiceRoot<T> {
    fn clone(&self) -> Self {
        Self {
            generation: Arc::clone(&self.generation),
        }
    }
}

impl<T> ServiceRoot<T> {
    #[must_use]
    pub fn generation_id(&self) -> ServiceGenerationId {
        self.generation.generation_id()
    }

    #[must_use]
    pub fn service_set_id(&self) -> ServiceSetId {
        self.generation.service_set_id()
    }

    #[must_use]
    pub fn generation(&self) -> &Arc<ServiceGeneration<T>> {
        &self.generation
    }
}

impl<T> Deref for ServiceRoot<T> {
    type Target = ServiceGeneration<T>;

    fn deref(&self) -> &Self::Target {
        &self.generation
    }
}

/// A complete generation staged against one exact pinned base.
pub struct ServiceGenerationCandidate<T> {
    controller_id: ServiceControllerId,
    service_set_id: ServiceSetId,
    expected: Arc<ServiceGeneration<T>>,
    generation: Arc<ServiceGeneration<T>>,
}

impl<T> ServiceGenerationCandidate<T> {
    #[must_use]
    pub fn generation_id(&self) -> ServiceGenerationId {
        self.generation.generation_id()
    }

    #[must_use]
    pub fn base_generation_id(&self) -> ServiceGenerationId {
        self.expected.generation_id()
    }

    #[must_use]
    pub fn generation(&self) -> &Arc<ServiceGeneration<T>> {
        &self.generation
    }
}

/// Conditional rollback authority for one successful activation.
pub struct ServiceRollbackToken<T> {
    controller_id: ServiceControllerId,
    service_set_id: ServiceSetId,
    replaced: Arc<ServiceGeneration<T>>,
    installed: Arc<ServiceGeneration<T>>,
}

impl<T> ServiceRollbackToken<T> {
    #[must_use]
    pub fn replaced_generation_id(&self) -> ServiceGenerationId {
        self.replaced.generation_id()
    }

    #[must_use]
    pub fn installed_generation_id(&self) -> ServiceGenerationId {
        self.installed.generation_id()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServicePublicationError {
    ForeignController,
    ForeignServiceSet {
        expected: ServiceSetId,
        actual: ServiceSetId,
    },
    StaleBaseGeneration {
        expected: ServiceGenerationId,
        active: ServiceGenerationId,
    },
    StaleRollback {
        expected: ServiceGenerationId,
        active: ServiceGenerationId,
    },
    GenerationIdExhausted,
}

impl fmt::Display for ServicePublicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignController => {
                formatter.write_str("service generation belongs to another controller")
            }
            Self::ForeignServiceSet { expected, actual } => write!(
                formatter,
                "service-set identity mismatch: expected {}, found {}",
                expected.get(),
                actual.get()
            ),
            Self::StaleBaseGeneration { expected, active } => write!(
                formatter,
                "stale service base generation: expected {}, active {}",
                expected.get(),
                active.get()
            ),
            Self::StaleRollback { expected, active } => write!(
                formatter,
                "stale service rollback: expected {}, active {}",
                expected.get(),
                active.get()
            ),
            Self::GenerationIdExhausted => formatter.write_str("service generation IDs exhausted"),
        }
    }
}

impl std::error::Error for ServicePublicationError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServiceStagingError {
    ContextTypeMismatch {
        expected: &'static str,
        actual: &'static str,
    },
    Source(ServiceSourceError),
    Selection(ServiceSelectionError),
    Deployment(ServiceBundleError),
    Publication(ServicePublicationError),
}

impl fmt::Display for ServiceStagingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContextTypeMismatch { expected, actual } => write!(
                formatter,
                "service set expects Runtime context `{expected}`, found `{actual}`"
            ),
            Self::Source(error) => write!(formatter, "service source failed: {error}"),
            Self::Selection(error) => write!(formatter, "service selection failed: {error}"),
            Self::Deployment(error) => write!(formatter, "service deployment failed: {error}"),
            Self::Publication(error) => write!(formatter, "service staging failed: {error}"),
        }
    }
}

impl std::error::Error for ServiceStagingError {}

impl From<ServiceSourceError> for ServiceStagingError {
    fn from(error: ServiceSourceError) -> Self {
        Self::Source(error)
    }
}

impl From<ServiceSelectionError> for ServiceStagingError {
    fn from(error: ServiceSelectionError) -> Self {
        Self::Selection(error)
    }
}

impl From<ServiceBundleError> for ServiceStagingError {
    fn from(error: ServiceBundleError) -> Self {
        Self::Deployment(error)
    }
}

impl From<ServicePublicationError> for ServiceStagingError {
    fn from(error: ServicePublicationError) -> Self {
        Self::Publication(error)
    }
}

/// Atomic publication owner for one generated service set.
pub struct ServiceController<T> {
    controller_id: ServiceControllerId,
    service_set_id: ServiceSetId,
    next_generation_id: AtomicU64,
    active: ArcSwap<ServiceGeneration<T>>,
}

impl<T> ServiceController<T> {
    #[must_use]
    pub fn new(service_set_id: ServiceSetId, defaults: T) -> Self {
        let controller_id = ServiceControllerId::next();
        let initial = Arc::new(ServiceGeneration {
            controller_id,
            service_set_id,
            generation_id: ServiceGenerationId::new(1),
            services: defaults,
        });
        Self {
            controller_id,
            service_set_id,
            next_generation_id: AtomicU64::new(2),
            active: ArcSwap::from(initial),
        }
    }

    #[must_use]
    pub const fn service_set_id(&self) -> ServiceSetId {
        self.service_set_id
    }

    #[must_use]
    pub fn pin(&self) -> ServiceRoot<T> {
        ServiceRoot {
            generation: self.active.load_full(),
        }
    }

    pub fn stage(
        &self,
        base: &ServiceRoot<T>,
        services: T,
    ) -> Result<ServiceGenerationCandidate<T>, ServicePublicationError> {
        self.validate_generation(base.generation())?;
        let generation_id = self.next_generation_id()?;
        Ok(ServiceGenerationCandidate {
            controller_id: self.controller_id,
            service_set_id: self.service_set_id,
            expected: Arc::clone(base.generation()),
            generation: Arc::new(ServiceGeneration {
                controller_id: self.controller_id,
                service_set_id: self.service_set_id,
                generation_id,
                services,
            }),
        })
    }

    pub fn activate_if_current(
        &self,
        candidate: ServiceGenerationCandidate<T>,
    ) -> Result<ServiceRollbackToken<T>, ServicePublicationError> {
        self.validate_candidate(&candidate)?;
        let previous = self
            .active
            .compare_and_swap(&candidate.expected, Arc::clone(&candidate.generation));
        if !Arc::ptr_eq(&previous, &candidate.expected) {
            return Err(ServicePublicationError::StaleBaseGeneration {
                expected: candidate.expected.generation_id(),
                active: previous.generation_id(),
            });
        }
        Ok(ServiceRollbackToken {
            controller_id: self.controller_id,
            service_set_id: self.service_set_id,
            replaced: candidate.expected,
            installed: candidate.generation,
        })
    }

    pub fn rollback_if_current(
        &self,
        token: ServiceRollbackToken<T>,
    ) -> Result<ServiceRoot<T>, ServicePublicationError> {
        self.validate_rollback(&token)?;
        let previous = self
            .active
            .compare_and_swap(&token.installed, Arc::clone(&token.replaced));
        if !Arc::ptr_eq(&previous, &token.installed) {
            return Err(ServicePublicationError::StaleRollback {
                expected: token.installed.generation_id(),
                active: previous.generation_id(),
            });
        }
        Ok(ServiceRoot {
            generation: token.replaced,
        })
    }

    fn next_generation_id(&self) -> Result<ServiceGenerationId, ServicePublicationError> {
        self.next_generation_id
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map(ServiceGenerationId::new)
            .map_err(|_| ServicePublicationError::GenerationIdExhausted)
    }

    fn validate_generation(
        &self,
        generation: &Arc<ServiceGeneration<T>>,
    ) -> Result<(), ServicePublicationError> {
        if generation.service_set_id != self.service_set_id {
            return Err(ServicePublicationError::ForeignServiceSet {
                expected: self.service_set_id,
                actual: generation.service_set_id,
            });
        }
        if generation.controller_id != self.controller_id {
            return Err(ServicePublicationError::ForeignController);
        }
        Ok(())
    }

    fn validate_candidate(
        &self,
        candidate: &ServiceGenerationCandidate<T>,
    ) -> Result<(), ServicePublicationError> {
        if candidate.service_set_id != self.service_set_id {
            return Err(ServicePublicationError::ForeignServiceSet {
                expected: self.service_set_id,
                actual: candidate.service_set_id,
            });
        }
        if candidate.controller_id != self.controller_id {
            return Err(ServicePublicationError::ForeignController);
        }
        self.validate_generation(&candidate.expected)?;
        self.validate_generation(&candidate.generation)
    }

    fn validate_rollback(
        &self,
        token: &ServiceRollbackToken<T>,
    ) -> Result<(), ServicePublicationError> {
        if token.service_set_id != self.service_set_id {
            return Err(ServicePublicationError::ForeignServiceSet {
                expected: self.service_set_id,
                actual: token.service_set_id,
            });
        }
        if token.controller_id != self.controller_id {
            return Err(ServicePublicationError::ForeignController);
        }
        self.validate_generation(&token.replaced)?;
        self.validate_generation(&token.installed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct Services {
        inventory: u64,
        reward: u64,
    }

    const SERVICE_SET: ServiceSetId = ServiceSetId::new(7);

    #[test]
    fn roots_pin_one_complete_generation_across_activation_and_rollback() {
        let controller = ServiceController::new(
            SERVICE_SET,
            Services {
                inventory: 1,
                reward: 10,
            },
        );
        let old = controller.pin();
        let candidate = controller
            .stage(
                &old,
                Services {
                    inventory: 2,
                    reward: 20,
                },
            )
            .expect("candidate should stage");
        let rollback = controller
            .activate_if_current(candidate)
            .expect("candidate should activate");
        let new = controller.pin();

        assert_eq!((old.inventory, old.reward), (1, 10));
        assert_eq!((new.inventory, new.reward), (2, 20));
        assert_ne!(old.generation_id(), new.generation_id());

        let restored = controller
            .rollback_if_current(rollback)
            .expect("rollback should restore the complete prior generation");
        assert_eq!(restored.generation_id(), old.generation_id());
        assert_eq!(
            controller.pin().generation_id(),
            old.generation_id(),
            "rollback must republish the exact prior Arc"
        );
    }

    #[test]
    fn stale_candidate_cannot_overwrite_a_newer_generation() {
        let controller = ServiceController::new(
            SERVICE_SET,
            Services {
                inventory: 1,
                reward: 10,
            },
        );
        let base = controller.pin();
        let first = controller
            .stage(
                &base,
                Services {
                    inventory: 2,
                    reward: 20,
                },
            )
            .expect("first candidate");
        let stale = controller
            .stage(
                &base,
                Services {
                    inventory: 3,
                    reward: 30,
                },
            )
            .expect("stale candidate");
        controller
            .activate_if_current(first)
            .expect("first activation");

        assert!(matches!(
            controller.activate_if_current(stale),
            Err(ServicePublicationError::StaleBaseGeneration { .. })
        ));
        assert_eq!(
            (controller.pin().inventory, controller.pin().reward),
            (2, 20)
        );
    }

    #[test]
    fn rollback_token_cannot_overwrite_a_later_activation() {
        let controller = ServiceController::new(
            SERVICE_SET,
            Services {
                inventory: 1,
                reward: 10,
            },
        );
        let base = controller.pin();
        let first = controller
            .stage(
                &base,
                Services {
                    inventory: 2,
                    reward: 20,
                },
            )
            .and_then(|candidate| controller.activate_if_current(candidate))
            .expect("first activation");
        let active = controller.pin();
        let second = controller
            .stage(
                &active,
                Services {
                    inventory: 3,
                    reward: 30,
                },
            )
            .and_then(|candidate| controller.activate_if_current(candidate))
            .expect("second activation");

        assert!(matches!(
            controller.rollback_if_current(first),
            Err(ServicePublicationError::StaleRollback { .. })
        ));
        assert_eq!(
            (controller.pin().inventory, controller.pin().reward),
            (3, 30)
        );
        controller
            .rollback_if_current(second)
            .expect("latest rollback remains valid");
    }

    #[test]
    fn candidates_and_rollbacks_are_controller_bound() {
        let first = ServiceController::new(
            SERVICE_SET,
            Services {
                inventory: 1,
                reward: 10,
            },
        );
        let second = ServiceController::new(
            SERVICE_SET,
            Services {
                inventory: 1,
                reward: 10,
            },
        );
        let candidate = first
            .stage(
                &first.pin(),
                Services {
                    inventory: 2,
                    reward: 20,
                },
            )
            .expect("candidate");
        assert!(matches!(
            second.activate_if_current(candidate),
            Err(ServicePublicationError::ForeignController)
        ));
    }
}
