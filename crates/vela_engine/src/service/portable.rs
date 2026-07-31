//! Source-independent service Snapshot/Delta transport.

use std::fmt;

use bincode::Options;
use serde::{Deserialize, Serialize};
use vela_bytecode::{ArtifactChecksum, PortableArtifactError, PortableProgramArtifact};
use vela_common::{
    ServiceGenerationId, ServiceSetAbiFingerprint, ServiceSetId, TypeBindingRegistryChecksum,
};

use super::source::PortableServiceSourceManifest;
use super::{
    ServiceBundleError, ServiceMethodSelection, ServiceSetSchema, ServiceSourceError,
    ServiceSourceManifest, ServiceUpdateBundle, ServiceUpdateMode,
};
use crate::engine::Engine;
use crate::native::TypeHint;

const MAGIC: &[u8; 8] = b"VELASVC\0";
// Version 1 may contain bytecode produced under implicit Host-borrow release
// semantics. Reject it at the transport boundary instead of interpreting it.
const FORMAT_VERSION: u32 = 2;
const HEADER_LEN: usize = MAGIC.len() + size_of::<u32>() + size_of::<u64>() + 32;
const MAX_PAYLOAD_BYTES: u64 = 128 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PortableServiceBundleChecksum([u8; 32]);

impl PortableServiceBundleChecksum {
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for PortableServiceBundleChecksum {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PortableDiagnosticSource {
    path: String,
    text: String,
}

impl PortableDiagnosticSource {
    #[must_use]
    pub fn new(path: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            text: text.into(),
        }
    }

    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PortableServiceUpdateBundle {
    payload: PortableServicePayload,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct PortableServicePayload {
    mode: PortableServiceMode,
    service_set_id: u128,
    service_set_abi: u64,
    type_binding_checksum: u64,
    service_manifest_checksum: [u8; 32],
    host_schema_hash: u64,
    artifact_checksum: [u8; 32],
    artifact: Vec<u8>,
    update: PortableServiceSourceManifest,
    update_count: u32,
    diagnostics: Vec<PortableDiagnosticSource>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum PortableServiceMode {
    Snapshot,
    Delta {
        base_generation_id: u64,
        base_artifact_checksum: [u8; 32],
    },
}

impl PortableServiceUpdateBundle {
    pub fn snapshot(
        schema: &ServiceSetSchema,
        artifact: PortableProgramArtifact,
        update: &ServiceSourceManifest,
        host_schema_hash: u64,
        diagnostics: impl IntoIterator<Item = PortableDiagnosticSource>,
    ) -> Result<Self, PortableServiceBundleError> {
        Self::build(
            PortableServiceMode::Snapshot,
            schema,
            artifact,
            update,
            host_schema_hash,
            diagnostics,
        )
    }

    pub fn delta(
        schema: &ServiceSetSchema,
        base_generation_id: ServiceGenerationId,
        base_artifact_checksum: ArtifactChecksum,
        artifact: PortableProgramArtifact,
        update: &ServiceSourceManifest,
        host_schema_hash: u64,
        diagnostics: impl IntoIterator<Item = PortableDiagnosticSource>,
    ) -> Result<Self, PortableServiceBundleError> {
        Self::build(
            PortableServiceMode::Delta {
                base_generation_id: base_generation_id.get(),
                base_artifact_checksum: *base_artifact_checksum.as_bytes(),
            },
            schema,
            artifact,
            update,
            host_schema_hash,
            diagnostics,
        )
    }

    fn build(
        mode: PortableServiceMode,
        schema: &ServiceSetSchema,
        artifact: PortableProgramArtifact,
        update: &ServiceSourceManifest,
        host_schema_hash: u64,
        diagnostics: impl IntoIterator<Item = PortableDiagnosticSource>,
    ) -> Result<Self, PortableServiceBundleError> {
        update
            .clone()
            .into_snapshot(schema)
            .map_err(ServiceBundleError::from)?;
        validate_interpreter_service_parameters(schema, &artifact, update)?;
        let artifact_checksum = *artifact.checksum().as_bytes();
        let artifact = artifact.encode()?;
        let update_count =
            u32::try_from(update.len()).map_err(|_| PortableServiceBundleError::UpdateLimit)?;
        Ok(Self {
            payload: PortableServicePayload {
                mode,
                service_set_id: schema.id().get(),
                service_set_abi: schema.abi_fingerprint().get(),
                type_binding_checksum: schema.type_binding_checksum().get(),
                service_manifest_checksum: checksum_debug(schema),
                host_schema_hash,
                artifact_checksum,
                artifact,
                update: update.to_portable(),
                update_count,
                diagnostics: diagnostics.into_iter().collect(),
            },
        })
    }

    pub fn encode(&self) -> Result<Vec<u8>, PortableServiceBundleError> {
        let payload = codec()
            .serialize(&self.payload)
            .map_err(|error| PortableServiceBundleError::Encode(error.to_string()))?;
        if payload.len() as u64 > MAX_PAYLOAD_BYTES {
            return Err(PortableServiceBundleError::PayloadTooLarge {
                maximum: MAX_PAYLOAD_BYTES,
                actual: payload.len() as u64,
            });
        }
        let checksum = blake3::hash(&payload);
        let mut encoded = Vec::with_capacity(HEADER_LEN + payload.len());
        encoded.extend_from_slice(MAGIC);
        encoded.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
        encoded.extend_from_slice(&(payload.len() as u64).to_le_bytes());
        encoded.extend_from_slice(checksum.as_bytes());
        encoded.extend_from_slice(&payload);
        Ok(encoded)
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, PortableServiceBundleError> {
        if encoded.len() < HEADER_LEN {
            return Err(PortableServiceBundleError::Truncated);
        }
        if &encoded[..MAGIC.len()] != MAGIC {
            return Err(PortableServiceBundleError::InvalidMagic);
        }
        let mut cursor = MAGIC.len();
        let version = read_u32(encoded, &mut cursor);
        if version != FORMAT_VERSION {
            return Err(PortableServiceBundleError::UnsupportedFormat {
                expected: FORMAT_VERSION,
                actual: version,
            });
        }
        let payload_len = read_u64(encoded, &mut cursor);
        if payload_len > MAX_PAYLOAD_BYTES {
            return Err(PortableServiceBundleError::PayloadTooLarge {
                maximum: MAX_PAYLOAD_BYTES,
                actual: payload_len,
            });
        }
        let payload_len = usize::try_from(payload_len).map_err(|_| {
            PortableServiceBundleError::PayloadTooLarge {
                maximum: MAX_PAYLOAD_BYTES,
                actual: payload_len,
            }
        })?;
        let expected_len = HEADER_LEN.checked_add(payload_len).ok_or(
            PortableServiceBundleError::PayloadTooLarge {
                maximum: MAX_PAYLOAD_BYTES,
                actual: payload_len as u64,
            },
        )?;
        if encoded.len() != expected_len {
            return Err(PortableServiceBundleError::LengthMismatch {
                expected: expected_len,
                actual: encoded.len(),
            });
        }
        let checksum = &encoded[cursor..cursor + 32];
        cursor += 32;
        let payload = &encoded[cursor..];
        if blake3::hash(payload).as_bytes() != checksum {
            return Err(PortableServiceBundleError::ChecksumMismatch);
        }
        let payload = codec()
            .with_limit(MAX_PAYLOAD_BYTES)
            .deserialize(payload)
            .map_err(|error| PortableServiceBundleError::Decode(error.to_string()))?;
        Ok(Self { payload })
    }

    #[must_use]
    pub fn checksum(&self) -> PortableServiceBundleChecksum {
        let payload = codec()
            .serialize(&self.payload)
            .expect("validated service payload is always encodable");
        PortableServiceBundleChecksum::new(*blake3::hash(&payload).as_bytes())
    }

    #[must_use]
    pub const fn host_schema_hash(&self) -> u64 {
        self.payload.host_schema_hash
    }

    #[must_use]
    pub const fn artifact_checksum(&self) -> ArtifactChecksum {
        ArtifactChecksum::new(self.payload.artifact_checksum)
    }

    #[must_use]
    pub const fn mode(&self) -> ServiceUpdateMode {
        match self.payload.mode {
            PortableServiceMode::Snapshot => ServiceUpdateMode::Snapshot,
            PortableServiceMode::Delta {
                base_generation_id,
                base_artifact_checksum,
            } => ServiceUpdateMode::Delta {
                base_generation_id: ServiceGenerationId::new(base_generation_id),
                base_artifact_checksum: ArtifactChecksum::new(base_artifact_checksum),
            },
        }
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[PortableDiagnosticSource] {
        &self.payload.diagnostics
    }

    /// Validates schema/build identities, binds stable bytecode identities to
    /// the receiving Engine, and returns the ordinary in-memory staging unit.
    pub fn load(
        self,
        engine: &Engine,
        schema: &ServiceSetSchema,
        expected_host_schema_hash: u64,
    ) -> Result<ServiceUpdateBundle, PortableServiceBundleError> {
        self.validate_schema(schema, expected_host_schema_hash)?;
        if self.payload.update_count as usize != self.payload.update.len() {
            return Err(PortableServiceBundleError::UpdateCountMismatch);
        }
        let artifact = PortableProgramArtifact::decode(&self.payload.artifact)?;
        if artifact.checksum().as_bytes() != &self.payload.artifact_checksum {
            return Err(PortableServiceBundleError::ArtifactChecksumMismatch);
        }
        let artifact = engine
            .link_portable_program(artifact.into_compiled())
            .map_err(|error| PortableServiceBundleError::Link(error.to_string()))?;
        let update = ServiceSourceManifest::from_portable(self.payload.update)?
            .bind_artifact(artifact.clone())?;
        match self.payload.mode {
            PortableServiceMode::Snapshot => {
                ServiceUpdateBundle::snapshot(schema, artifact, update).map_err(Into::into)
            }
            PortableServiceMode::Delta {
                base_generation_id,
                base_artifact_checksum,
            } => ServiceUpdateBundle::delta(
                schema,
                ServiceGenerationId::new(base_generation_id),
                ArtifactChecksum::new(base_artifact_checksum),
                artifact,
                update,
            )
            .map_err(Into::into),
        }
    }

    fn validate_schema(
        &self,
        schema: &ServiceSetSchema,
        expected_host_schema_hash: u64,
    ) -> Result<(), PortableServiceBundleError> {
        if self.payload.service_set_id != schema.id().get() {
            return Err(PortableServiceBundleError::ForeignServiceSet {
                expected: schema.id(),
                actual: ServiceSetId::new(self.payload.service_set_id),
            });
        }
        if self.payload.service_set_abi != schema.abi_fingerprint().get() {
            return Err(PortableServiceBundleError::IncompatibleServiceSetSchema {
                expected: schema.abi_fingerprint(),
                actual: ServiceSetAbiFingerprint::new(self.payload.service_set_abi),
            });
        }
        if self.payload.type_binding_checksum != schema.type_binding_checksum().get() {
            return Err(PortableServiceBundleError::TypeBindingChecksumMismatch {
                expected: schema.type_binding_checksum(),
                actual: TypeBindingRegistryChecksum::new(self.payload.type_binding_checksum),
            });
        }
        if self.payload.service_manifest_checksum != checksum_debug(schema) {
            return Err(PortableServiceBundleError::ServiceManifestChecksumMismatch);
        }
        if self.payload.host_schema_hash != expected_host_schema_hash {
            return Err(PortableServiceBundleError::HostSchemaHashMismatch {
                expected: expected_host_schema_hash,
                actual: self.payload.host_schema_hash,
            });
        }
        Ok(())
    }
}

fn validate_interpreter_service_parameters(
    schema: &ServiceSetSchema,
    artifact: &PortableProgramArtifact,
    update: &ServiceSourceManifest,
) -> Result<(), PortableServiceBundleError> {
    for update in update.updates() {
        let ServiceMethodSelection::Vela(target) = update.selection() else {
            continue;
        };
        let service = schema
            .services()
            .iter()
            .find(|service| service.id() == update.key().service_id)
            .expect("schema-linked service update retains its service");
        let method = service
            .methods()
            .iter()
            .find(|method| method.id == update.key().method_id)
            .expect("schema-linked service update retains its method");
        let function = artifact.function_by_id(target.function()).ok_or_else(|| {
            PortableServiceBundleError::MissingProgramTarget {
                symbol: target.symbol().to_owned(),
            }
        })?;
        for (parameter, contract) in method.callable.parameters.iter().enumerate() {
            if !matches!(contract.ty, TypeHint::Host(_)) {
                continue;
            }
            let guarded = function.param_guards.iter().any(|guard| {
                usize::from(guard.parameter) == parameter
                    && matches!(
                        guard.guard.plan,
                        vela_bytecode::UnlinkedTypeGuardPlan::HostType { .. }
                    )
            });
            if !guarded {
                return Err(PortableServiceBundleError::UntypedHostParameter {
                    symbol: target.symbol().to_owned(),
                    parameter: contract.name.clone(),
                });
            }
        }
    }
    Ok(())
}

fn codec() -> impl Options {
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .with_little_endian()
        .reject_trailing_bytes()
}

fn checksum_debug(value: &impl fmt::Debug) -> [u8; 32] {
    *blake3::hash(format!("{value:?}").as_bytes()).as_bytes()
}

fn read_u32(input: &[u8], cursor: &mut usize) -> u32 {
    let bytes = input[*cursor..*cursor + size_of::<u32>()]
        .try_into()
        .expect("header length checked");
    *cursor += size_of::<u32>();
    u32::from_le_bytes(bytes)
}

fn read_u64(input: &[u8], cursor: &mut usize) -> u64 {
    let bytes = input[*cursor..*cursor + size_of::<u64>()]
        .try_into()
        .expect("header length checked");
    *cursor += size_of::<u64>();
    u64::from_le_bytes(bytes)
}

#[derive(Debug)]
pub enum PortableServiceBundleError {
    InvalidMagic,
    Truncated,
    UnsupportedFormat {
        expected: u32,
        actual: u32,
    },
    PayloadTooLarge {
        maximum: u64,
        actual: u64,
    },
    LengthMismatch {
        expected: usize,
        actual: usize,
    },
    ChecksumMismatch,
    Encode(String),
    Decode(String),
    UpdateLimit,
    UpdateCountMismatch,
    ArtifactChecksumMismatch,
    MissingProgramTarget {
        symbol: String,
    },
    UntypedHostParameter {
        symbol: String,
        parameter: String,
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
    HostSchemaHashMismatch {
        expected: u64,
        actual: u64,
    },
    Program(PortableArtifactError),
    Link(String),
    Source(ServiceSourceError),
    Service(ServiceBundleError),
}

impl fmt::Display for PortableServiceBundleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMagic => formatter.write_str("portable service bundle magic is invalid"),
            Self::Truncated => formatter.write_str("portable service bundle header is truncated"),
            Self::UnsupportedFormat { expected, actual } => write!(
                formatter,
                "portable service bundle format {actual} is unsupported; expected {expected}"
            ),
            Self::PayloadTooLarge { maximum, actual } => write!(
                formatter,
                "portable service bundle payload is {actual} bytes; maximum is {maximum}"
            ),
            Self::LengthMismatch { expected, actual } => write!(
                formatter,
                "portable service bundle length is {actual} bytes; expected {expected}"
            ),
            Self::ChecksumMismatch => {
                formatter.write_str("portable service bundle checksum does not match its payload")
            }
            Self::Encode(message) => {
                write!(
                    formatter,
                    "portable service bundle encode failed: {message}"
                )
            }
            Self::Decode(message) => {
                write!(
                    formatter,
                    "portable service bundle decode failed: {message}"
                )
            }
            Self::UpdateLimit => {
                formatter.write_str("portable service bundle has more than u32::MAX updates")
            }
            Self::UpdateCountMismatch => {
                formatter.write_str("portable service bundle update count is inconsistent")
            }
            Self::ArtifactChecksumMismatch => formatter
                .write_str("portable service bundle artifact checksum does not match its payload"),
            Self::MissingProgramTarget { symbol } => write!(
                formatter,
                "portable service bundle has no compiled target for `{symbol}`"
            ),
            Self::UntypedHostParameter { symbol, parameter } => write!(
                formatter,
                "portable interpreter service `{symbol}` must declare the Host parameter \
                 `{parameter}` with its registered Vela type"
            ),
            Self::ForeignServiceSet { expected, actual } => write!(
                formatter,
                "portable service bundle expects set {}, found {}",
                expected.get(),
                actual.get()
            ),
            Self::IncompatibleServiceSetSchema { expected, actual } => write!(
                formatter,
                "portable service bundle expects set ABI {:016x}, found {:016x}",
                expected.get(),
                actual.get()
            ),
            Self::TypeBindingChecksumMismatch { expected, actual } => write!(
                formatter,
                "portable service bundle expects TypeBinding checksum {:016x}, found {:016x}",
                expected.get(),
                actual.get()
            ),
            Self::ServiceManifestChecksumMismatch => formatter.write_str(
                "portable service bundle manifest checksum does not match the host schema",
            ),
            Self::HostSchemaHashMismatch { expected, actual } => write!(
                formatter,
                "portable service bundle expects host schema hash {expected:016x}, found {actual:016x}"
            ),
            Self::Program(error) => error.fmt(formatter),
            Self::Link(message) => write!(
                formatter,
                "portable service artifact link failed: {message}"
            ),
            Self::Source(error) => error.fmt(formatter),
            Self::Service(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for PortableServiceBundleError {}

impl From<PortableArtifactError> for PortableServiceBundleError {
    fn from(error: PortableArtifactError) -> Self {
        Self::Program(error)
    }
}

impl From<ServiceSourceError> for PortableServiceBundleError {
    fn from(error: ServiceSourceError) -> Self {
        Self::Source(error)
    }
}

impl From<ServiceBundleError> for PortableServiceBundleError {
    fn from(error: ServiceBundleError) -> Self {
        Self::Service(error)
    }
}
