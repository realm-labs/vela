//! Immutable service deployment bundles and dry-run validation.

use std::fmt;
use std::sync::Arc;

use vela_bytecode::{ArtifactChecksum, LinkedArtifact};
use vela_common::{
    ServiceGenerationId, ServiceSetAbiFingerprint, ServiceSetId, TypeBindingRegistryChecksum,
};

use super::{
    LinkedServiceSourceManifest, LinkedVelaServiceMethod, ServiceMethodSelection,
    ServiceSelectionTable, ServiceSetSchema, ServiceStagingError,
};

// Version 1 metadata identifies generations built under the old implicit
// Host-borrow release contract and is deliberately not loadable.
const SERVICE_BUNDLE_FORMAT_VERSION: u32 = 2;

/// Checksum used for service manifests, sparse operations, and package metadata.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ServiceBundleChecksum([u8; 32]);

impl ServiceBundleChecksum {
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for ServiceBundleChecksum {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// Snapshot versus exact-base Delta omission semantics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceUpdateMode {
    Snapshot,
    Delta {
        base_generation_id: ServiceGenerationId,
        base_artifact_checksum: ArtifactChecksum,
    },
}

/// Package identity copied into immutable deployment metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServicePackageIdentity {
    id: String,
    version: String,
}

impl ServicePackageIdentity {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }
}

/// Detached metadata persisted alongside a service deployment artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceUpdateMetadata {
    format_version: u32,
    mode: ServiceUpdateMode,
    service_set_id: ServiceSetId,
    service_set_abi: ServiceSetAbiFingerprint,
    type_binding_checksum: TypeBindingRegistryChecksum,
    service_manifest_checksum: ServiceBundleChecksum,
    update_checksum: ServiceBundleChecksum,
    artifact_checksum: ArtifactChecksum,
    package_checksum: Option<ServiceBundleChecksum>,
    packages: Vec<ServicePackageIdentity>,
    update_count: usize,
}

impl ServiceUpdateMetadata {
    #[must_use]
    pub const fn format_version(&self) -> u32 {
        self.format_version
    }

    #[must_use]
    pub const fn mode(&self) -> ServiceUpdateMode {
        self.mode
    }

    #[must_use]
    pub const fn service_set_id(&self) -> ServiceSetId {
        self.service_set_id
    }

    #[must_use]
    pub const fn service_set_abi(&self) -> ServiceSetAbiFingerprint {
        self.service_set_abi
    }

    #[must_use]
    pub const fn type_binding_checksum(&self) -> TypeBindingRegistryChecksum {
        self.type_binding_checksum
    }

    #[must_use]
    pub const fn service_manifest_checksum(&self) -> ServiceBundleChecksum {
        self.service_manifest_checksum
    }

    #[must_use]
    pub const fn update_checksum(&self) -> ServiceBundleChecksum {
        self.update_checksum
    }

    #[must_use]
    pub const fn artifact_checksum(&self) -> ArtifactChecksum {
        self.artifact_checksum
    }

    #[must_use]
    pub const fn package_checksum(&self) -> Option<ServiceBundleChecksum> {
        self.package_checksum
    }

    #[must_use]
    pub fn packages(&self) -> &[ServicePackageIdentity] {
        &self.packages
    }

    #[must_use]
    pub const fn update_count(&self) -> usize {
        self.update_count
    }
}

/// One immutable loaded Snapshot or exact-base Delta.
#[derive(Clone, Debug)]
pub struct ServiceUpdateBundle {
    metadata: ServiceUpdateMetadata,
    artifact: Arc<LinkedArtifact>,
    update: LinkedServiceSourceManifest,
}

impl ServiceUpdateBundle {
    pub fn snapshot(
        schema: &ServiceSetSchema,
        artifact: Arc<LinkedArtifact>,
        update: LinkedServiceSourceManifest,
    ) -> Result<Self, ServiceBundleError> {
        Self::build(ServiceUpdateMode::Snapshot, schema, artifact, update)
    }

    pub fn delta(
        schema: &ServiceSetSchema,
        base_generation_id: ServiceGenerationId,
        base_artifact_checksum: ArtifactChecksum,
        artifact: Arc<LinkedArtifact>,
        update: LinkedServiceSourceManifest,
    ) -> Result<Self, ServiceBundleError> {
        Self::build(
            ServiceUpdateMode::Delta {
                base_generation_id,
                base_artifact_checksum,
            },
            schema,
            artifact,
            update,
        )
    }

    /// Loads detached metadata with its already linked artifact and sparse
    /// operations, rejecting any checksum or schema substitution.
    pub fn load(
        metadata: ServiceUpdateMetadata,
        schema: &ServiceSetSchema,
        artifact: Arc<LinkedArtifact>,
        update: LinkedServiceSourceManifest,
    ) -> Result<Self, ServiceBundleError> {
        if metadata.format_version != SERVICE_BUNDLE_FORMAT_VERSION {
            return Err(ServiceBundleError::UnsupportedFormat {
                expected: SERVICE_BUNDLE_FORMAT_VERSION,
                actual: metadata.format_version,
            });
        }
        let rebuilt = Self::build(metadata.mode, schema, artifact, update)?;
        if rebuilt.metadata != metadata {
            return Err(ServiceBundleError::MetadataChecksumMismatch);
        }
        Ok(rebuilt)
    }

    fn build(
        mode: ServiceUpdateMode,
        schema: &ServiceSetSchema,
        artifact: Arc<LinkedArtifact>,
        update: LinkedServiceSourceManifest,
    ) -> Result<Self, ServiceBundleError> {
        let artifact_checksum = artifact.checksum();
        if let Some(bound) = update.artifact_checksum()
            && bound != artifact_checksum
        {
            return Err(ServiceBundleError::ArtifactChecksumMismatch {
                expected: artifact_checksum,
                actual: bound,
            });
        }
        let (packages, package_checksum) = package_metadata(&artifact);
        let metadata = ServiceUpdateMetadata {
            format_version: SERVICE_BUNDLE_FORMAT_VERSION,
            mode,
            service_set_id: schema.id(),
            service_set_abi: schema.abi_fingerprint(),
            type_binding_checksum: schema.type_binding_checksum(),
            service_manifest_checksum: checksum_debug(schema),
            update_checksum: update_checksum(&update, artifact_checksum),
            artifact_checksum,
            package_checksum,
            packages,
            update_count: update.len(),
        };
        Ok(Self {
            metadata,
            artifact,
            update,
        })
    }

    #[must_use]
    pub const fn metadata(&self) -> &ServiceUpdateMetadata {
        &self.metadata
    }

    #[must_use]
    pub fn artifact(&self) -> &Arc<LinkedArtifact> {
        &self.artifact
    }

    #[must_use]
    pub fn update(&self) -> &LinkedServiceSourceManifest {
        &self.update
    }

    pub fn dry_run(
        &self,
        schema: &ServiceSetSchema,
        active_generation: ServiceGenerationId,
        active_artifact_checksum: Option<ArtifactChecksum>,
        base: &ServiceSelectionTable<LinkedVelaServiceMethod>,
    ) -> ServiceDryRunReport {
        let outcome = match self.validate(schema, active_generation, active_artifact_checksum) {
            Ok(()) => self
                .compose(schema, active_generation, base)
                .map(|selections| ServiceSelectionSummary::from_table(&selections)),
            Err(error) => Err(ServiceStagingError::from(error)),
        };
        ServiceDryRunReport {
            metadata: self.metadata.clone(),
            active_generation,
            active_artifact_checksum,
            outcome,
        }
    }

    #[doc(hidden)]
    pub fn into_selection(
        self,
        schema: &ServiceSetSchema,
        active_generation: ServiceGenerationId,
        active_artifact_checksum: Option<ArtifactChecksum>,
        base: &ServiceSelectionTable<LinkedVelaServiceMethod>,
    ) -> Result<
        (
            ServiceSelectionTable<LinkedVelaServiceMethod>,
            ServiceUpdateMetadata,
        ),
        ServiceStagingError,
    > {
        self.validate(schema, active_generation, active_artifact_checksum)?;
        let selections = self.compose(schema, active_generation, base)?;
        Ok((selections, self.metadata))
    }

    fn validate(
        &self,
        schema: &ServiceSetSchema,
        active_generation: ServiceGenerationId,
        active_artifact_checksum: Option<ArtifactChecksum>,
    ) -> Result<(), ServiceBundleError> {
        if self.metadata.service_set_id != schema.id() {
            return Err(ServiceBundleError::ForeignServiceSet {
                expected: schema.id(),
                actual: self.metadata.service_set_id,
            });
        }
        if self.metadata.service_set_abi != schema.abi_fingerprint() {
            return Err(ServiceBundleError::IncompatibleServiceSetSchema {
                expected: schema.abi_fingerprint(),
                actual: self.metadata.service_set_abi,
            });
        }
        if self.metadata.type_binding_checksum != schema.type_binding_checksum() {
            return Err(ServiceBundleError::TypeBindingChecksumMismatch {
                expected: schema.type_binding_checksum(),
                actual: self.metadata.type_binding_checksum,
            });
        }
        if self.metadata.service_manifest_checksum != checksum_debug(schema) {
            return Err(ServiceBundleError::ServiceManifestChecksumMismatch);
        }
        if self.metadata.artifact_checksum != self.artifact.checksum() {
            return Err(ServiceBundleError::ArtifactChecksumMismatch {
                expected: self.metadata.artifact_checksum,
                actual: self.artifact.checksum(),
            });
        }
        if self.metadata.update_checksum
            != update_checksum(&self.update, self.metadata.artifact_checksum)
        {
            return Err(ServiceBundleError::UpdateChecksumMismatch);
        }
        if let ServiceUpdateMode::Delta {
            base_generation_id,
            base_artifact_checksum,
        } = self.metadata.mode
        {
            if base_generation_id != active_generation {
                return Err(ServiceBundleError::BaseGenerationMismatch {
                    expected: base_generation_id,
                    actual: active_generation,
                });
            }
            if Some(base_artifact_checksum) != active_artifact_checksum {
                return Err(ServiceBundleError::BaseArtifactChecksumMismatch {
                    expected: base_artifact_checksum,
                    actual: active_artifact_checksum,
                });
            }
        }
        Ok(())
    }

    fn compose(
        &self,
        schema: &ServiceSetSchema,
        active_generation: ServiceGenerationId,
        base: &ServiceSelectionTable<LinkedVelaServiceMethod>,
    ) -> Result<ServiceSelectionTable<LinkedVelaServiceMethod>, ServiceStagingError> {
        match self.metadata.mode {
            ServiceUpdateMode::Snapshot => self
                .update
                .clone()
                .into_snapshot(schema)
                .map_err(Into::into),
            ServiceUpdateMode::Delta {
                base_generation_id, ..
            } => {
                self.update
                    .clone()
                    .into_delta(schema, base_generation_id, active_generation, base)
            }
        }
    }
}

/// Counts from a successfully flattened dry-run selection table.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ServiceSelectionSummary {
    method_count: usize,
    rust_default_count: usize,
    vela_count: usize,
}

impl ServiceSelectionSummary {
    fn from_table(table: &ServiceSelectionTable<LinkedVelaServiceMethod>) -> Self {
        let mut summary = Self {
            method_count: table.len(),
            ..Self::default()
        };
        for (_, selection) in table.iter() {
            match selection {
                ServiceMethodSelection::RustDefault => summary.rust_default_count += 1,
                ServiceMethodSelection::Vela(_) => summary.vela_count += 1,
            }
        }
        summary
    }

    #[must_use]
    pub const fn method_count(self) -> usize {
        self.method_count
    }

    #[must_use]
    pub const fn rust_default_count(self) -> usize {
        self.rust_default_count
    }

    #[must_use]
    pub const fn vela_count(self) -> usize {
        self.vela_count
    }
}

/// Read-only result of validating and flattening a bundle against one root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceDryRunReport {
    metadata: ServiceUpdateMetadata,
    active_generation: ServiceGenerationId,
    active_artifact_checksum: Option<ArtifactChecksum>,
    outcome: Result<ServiceSelectionSummary, ServiceStagingError>,
}

impl ServiceDryRunReport {
    #[must_use]
    pub const fn metadata(&self) -> &ServiceUpdateMetadata {
        &self.metadata
    }

    #[must_use]
    pub const fn active_generation(&self) -> ServiceGenerationId {
        self.active_generation
    }

    #[must_use]
    pub const fn active_artifact_checksum(&self) -> Option<ArtifactChecksum> {
        self.active_artifact_checksum
    }

    pub const fn outcome(&self) -> &Result<ServiceSelectionSummary, ServiceStagingError> {
        &self.outcome
    }

    #[must_use]
    pub const fn accepted(&self) -> bool {
        self.outcome.is_ok()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServiceBundleError {
    UnsupportedFormat {
        expected: u32,
        actual: u32,
    },
    ForeignServiceSet {
        expected: ServiceSetId,
        actual: ServiceSetId,
    },
    IncompatibleServiceSetSchema {
        expected: ServiceSetAbiFingerprint,
        actual: ServiceSetAbiFingerprint,
    },
    TypeBindingChecksumMismatch {
        expected: TypeBindingRegistryChecksum,
        actual: TypeBindingRegistryChecksum,
    },
    ServiceManifestChecksumMismatch,
    UpdateChecksumMismatch,
    MetadataChecksumMismatch,
    ArtifactChecksumMismatch {
        expected: ArtifactChecksum,
        actual: ArtifactChecksum,
    },
    BaseGenerationMismatch {
        expected: ServiceGenerationId,
        actual: ServiceGenerationId,
    },
    BaseArtifactChecksumMismatch {
        expected: ArtifactChecksum,
        actual: Option<ArtifactChecksum>,
    },
    Selection(super::ServiceSelectionError),
}

impl fmt::Display for ServiceBundleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedFormat { expected, actual } => {
                write!(
                    formatter,
                    "service bundle format {actual} is unsupported; expected {expected}"
                )
            }
            Self::ForeignServiceSet { expected, actual } => write!(
                formatter,
                "service bundle expects set {}, found {}",
                expected.get(),
                actual.get()
            ),
            Self::IncompatibleServiceSetSchema { expected, actual } => write!(
                formatter,
                "service bundle expects set ABI {:016x}, found {:016x}",
                expected.get(),
                actual.get()
            ),
            Self::TypeBindingChecksumMismatch { expected, actual } => write!(
                formatter,
                "service bundle expects TypeBinding checksum {:016x}, found {:016x}",
                expected.get(),
                actual.get()
            ),
            Self::ServiceManifestChecksumMismatch => formatter
                .write_str("service bundle manifest checksum does not match the host schema"),
            Self::UpdateChecksumMismatch => {
                formatter.write_str("service bundle sparse update checksum is invalid")
            }
            Self::MetadataChecksumMismatch => {
                formatter.write_str("loaded service bundle metadata does not match its contents")
            }
            Self::ArtifactChecksumMismatch { expected, actual } => write!(
                formatter,
                "service bundle expects artifact checksum {expected}, found {actual}"
            ),
            Self::BaseGenerationMismatch { expected, actual } => write!(
                formatter,
                "service Delta expects base generation {}, found {}",
                expected.get(),
                actual.get()
            ),
            Self::BaseArtifactChecksumMismatch { expected, actual } => match actual {
                Some(actual) => write!(
                    formatter,
                    "service Delta expects base artifact checksum {expected}, found {actual}"
                ),
                None => write!(
                    formatter,
                    "service Delta expects base artifact checksum {expected}, but the active generation has no Vela artifact"
                ),
            },
            Self::Selection(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ServiceBundleError {}

impl From<super::ServiceSelectionError> for ServiceBundleError {
    fn from(error: super::ServiceSelectionError) -> Self {
        Self::Selection(error)
    }
}

fn checksum_debug(value: &impl fmt::Debug) -> ServiceBundleChecksum {
    let mut hasher = blake3::Hasher::new();
    hasher.update(format!("{value:?}").as_bytes());
    ServiceBundleChecksum::new(*hasher.finalize().as_bytes())
}

fn update_checksum(
    update: &LinkedServiceSourceManifest,
    artifact_checksum: ArtifactChecksum,
) -> ServiceBundleChecksum {
    let mut hasher = blake3::Hasher::new();
    hasher.update(artifact_checksum.as_bytes());
    for operation in update.updates() {
        hasher.update(&operation.key().service_id.get().to_le_bytes());
        hasher.update(&operation.key().method_id.get().to_le_bytes());
        hasher.update(&operation.expected_service_abi().get().to_le_bytes());
        match operation.selection() {
            ServiceMethodSelection::RustDefault => {
                hasher.update(b"rust-default");
            }
            ServiceMethodSelection::Vela(target) => {
                hasher.update(b"vela");
                hasher.update(format!("{:?}", target.method()).as_bytes());
            }
        }
    }
    ServiceBundleChecksum::new(*hasher.finalize().as_bytes())
}

fn package_metadata(
    artifact: &LinkedArtifact,
) -> (Vec<ServicePackageIdentity>, Option<ServiceBundleChecksum>) {
    let Some(metadata) = artifact.package_metadata() else {
        return (Vec::new(), None);
    };
    let packages = metadata
        .packages()
        .iter()
        .map(|package| ServicePackageIdentity {
            id: package.id().as_str().to_owned(),
            version: package.version().as_str().to_owned(),
        })
        .collect::<Vec<_>>();
    let mut hasher = blake3::Hasher::new();
    for package in metadata.packages() {
        hasher.update(package.id().as_str().as_bytes());
        hasher.update(&[0]);
        hasher.update(package.version().as_str().as_bytes());
        hasher.update(&[0]);
        hasher.update(format!("{:?}", package.declared_capabilities()).as_bytes());
        hasher.update(format!("{:?}", package.observed_capabilities()).as_bytes());
    }
    (
        packages,
        Some(ServiceBundleChecksum::new(*hasher.finalize().as_bytes())),
    )
}
