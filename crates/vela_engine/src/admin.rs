//! Source-independent administrative/Actor-operation script transport.

use std::fmt;
use std::sync::Arc;

use bincode::Options;
use serde::{Deserialize, Serialize};
use vela_bytecode::{LinkedArtifact, PortableArtifactError, PortableProgramArtifact};
use vela_common::{CallableAsyncness, CapabilitySet};
use vela_def::FunctionId;

use crate::engine::Engine;

const MAGIC: &[u8; 8] = b"VELAADM\0";
const FORMAT_VERSION: u32 = 1;
const HEADER_LEN: usize = MAGIC.len() + size_of::<u32>() + size_of::<u64>() + 32;
const MAX_PAYLOAD_BYTES: u64 = 128 * 1024 * 1024;

/// Fixed, application-owned ABI for one administrative script entry.
///
/// `boundary_fingerprint` seals parameter/return semantics that are more
/// specific than Vela's runtime arity. The embedding application computes it
/// from its generated operation protocol and requires the exact value on load.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminScriptAbi {
    boundary_fingerprint: u64,
    entry: FunctionId,
    symbol: String,
    asyncness: CallableAsyncness,
    parameter_count: u32,
    capability_ceiling: CapabilitySet,
}

impl AdminScriptAbi {
    #[must_use]
    pub fn new(
        boundary_fingerprint: u64,
        entry: FunctionId,
        symbol: impl Into<String>,
        asyncness: CallableAsyncness,
        parameter_count: u32,
        capability_ceiling: CapabilitySet,
    ) -> Self {
        Self {
            boundary_fingerprint,
            entry,
            symbol: symbol.into(),
            asyncness,
            parameter_count,
            capability_ceiling,
        }
    }

    #[must_use]
    pub const fn boundary_fingerprint(&self) -> u64 {
        self.boundary_fingerprint
    }

    #[must_use]
    pub const fn entry(&self) -> FunctionId {
        self.entry
    }

    #[must_use]
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    #[must_use]
    pub const fn asyncness(&self) -> CallableAsyncness {
        self.asyncness
    }

    #[must_use]
    pub const fn parameter_count(&self) -> u32 {
        self.parameter_count
    }

    #[must_use]
    pub const fn capability_ceiling(&self) -> CapabilitySet {
        self.capability_ceiling
    }

    fn to_portable(&self) -> PortableAdminScriptAbi {
        PortableAdminScriptAbi {
            boundary_fingerprint: self.boundary_fingerprint,
            entry: self.entry.get(),
            symbol: self.symbol.clone(),
            asyncness: self.asyncness,
            parameter_count: self.parameter_count,
            capability_ceiling: self.capability_ceiling,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct PortableAdminScriptAbi {
    boundary_fingerprint: u64,
    entry: u128,
    symbol: String,
    asyncness: CallableAsyncness,
    parameter_count: u32,
    capability_ceiling: CapabilitySet,
}

impl PortableAdminScriptAbi {
    fn materialize(&self) -> AdminScriptAbi {
        AdminScriptAbi::new(
            self.boundary_fingerprint,
            FunctionId::new(self.entry),
            self.symbol.clone(),
            self.asyncness,
            self.parameter_count,
            self.capability_ceiling,
        )
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PortableAdminDiagnosticSource {
    path: String,
    text: String,
}

impl PortableAdminDiagnosticSource {
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

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PortableAdminBundleChecksum([u8; 32]);

impl PortableAdminBundleChecksum {
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for PortableAdminBundleChecksum {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// Versioned operation bundle that contains no source compiler dependency.
#[derive(Clone, Debug, PartialEq)]
pub struct PortableAdminScriptBundle {
    payload: PortableAdminPayload,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct PortableAdminPayload {
    host_schema_hash: u64,
    abi: PortableAdminScriptAbi,
    observed_capabilities: CapabilitySet,
    artifact_checksum: [u8; 32],
    artifact: Vec<u8>,
    diagnostics: Vec<PortableAdminDiagnosticSource>,
}

impl PortableAdminScriptBundle {
    pub fn build(
        host_schema_hash: u64,
        abi: &AdminScriptAbi,
        artifact: PortableProgramArtifact,
        diagnostics: impl IntoIterator<Item = PortableAdminDiagnosticSource>,
    ) -> Result<Self, PortableAdminBundleError> {
        let observed_capabilities = validate_program_contract(&artifact, abi)?;
        let artifact_checksum = *artifact.checksum().as_bytes();
        let artifact = artifact.encode()?;
        Ok(Self {
            payload: PortableAdminPayload {
                host_schema_hash,
                abi: abi.to_portable(),
                observed_capabilities,
                artifact_checksum,
                artifact,
                diagnostics: diagnostics.into_iter().collect(),
            },
        })
    }

    pub fn encode(&self) -> Result<Vec<u8>, PortableAdminBundleError> {
        let payload = codec()
            .serialize(&self.payload)
            .map_err(|error| PortableAdminBundleError::Encode(error.to_string()))?;
        if payload.len() as u64 > MAX_PAYLOAD_BYTES {
            return Err(PortableAdminBundleError::PayloadTooLarge {
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

    pub fn decode(encoded: &[u8]) -> Result<Self, PortableAdminBundleError> {
        if encoded.len() < HEADER_LEN {
            return Err(PortableAdminBundleError::Truncated);
        }
        if &encoded[..MAGIC.len()] != MAGIC {
            return Err(PortableAdminBundleError::InvalidMagic);
        }
        let mut cursor = MAGIC.len();
        let version = read_u32(encoded, &mut cursor);
        if version != FORMAT_VERSION {
            return Err(PortableAdminBundleError::UnsupportedFormat {
                expected: FORMAT_VERSION,
                actual: version,
            });
        }
        let payload_len = read_u64(encoded, &mut cursor);
        if payload_len > MAX_PAYLOAD_BYTES {
            return Err(PortableAdminBundleError::PayloadTooLarge {
                maximum: MAX_PAYLOAD_BYTES,
                actual: payload_len,
            });
        }
        let payload_len = usize::try_from(payload_len).map_err(|_| {
            PortableAdminBundleError::PayloadTooLarge {
                maximum: MAX_PAYLOAD_BYTES,
                actual: payload_len,
            }
        })?;
        let expected_len = HEADER_LEN.checked_add(payload_len).ok_or(
            PortableAdminBundleError::PayloadTooLarge {
                maximum: MAX_PAYLOAD_BYTES,
                actual: payload_len as u64,
            },
        )?;
        if encoded.len() != expected_len {
            return Err(PortableAdminBundleError::LengthMismatch {
                expected: expected_len,
                actual: encoded.len(),
            });
        }
        let checksum = &encoded[cursor..cursor + 32];
        cursor += 32;
        let payload = &encoded[cursor..];
        if blake3::hash(payload).as_bytes() != checksum {
            return Err(PortableAdminBundleError::ChecksumMismatch);
        }
        let payload = codec()
            .with_limit(MAX_PAYLOAD_BYTES)
            .deserialize(payload)
            .map_err(|error| PortableAdminBundleError::Decode(error.to_string()))?;
        Ok(Self { payload })
    }

    #[must_use]
    pub fn checksum(&self) -> PortableAdminBundleChecksum {
        let payload = codec()
            .serialize(&self.payload)
            .expect("validated admin payload is always encodable");
        PortableAdminBundleChecksum::new(*blake3::hash(&payload).as_bytes())
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[PortableAdminDiagnosticSource] {
        &self.payload.diagnostics
    }

    pub fn load(
        self,
        engine: &Engine,
        expected_host_schema_hash: u64,
        expected_abi: &AdminScriptAbi,
    ) -> Result<LoadedAdminScript, PortableAdminBundleError> {
        if self.payload.host_schema_hash != expected_host_schema_hash {
            return Err(PortableAdminBundleError::HostSchemaHashMismatch {
                expected: expected_host_schema_hash,
                actual: self.payload.host_schema_hash,
            });
        }
        let actual_abi = self.payload.abi.materialize();
        if &actual_abi != expected_abi {
            return Err(PortableAdminBundleError::AbiMismatch {
                expected: expected_abi.clone(),
                actual: actual_abi,
            });
        }
        if !engine
            .capabilities()
            .contains_all(self.payload.observed_capabilities)
        {
            return Err(PortableAdminBundleError::MissingCapabilities {
                available: engine.capabilities(),
                required: self.payload.observed_capabilities,
            });
        }
        let artifact = PortableProgramArtifact::decode(&self.payload.artifact)?;
        if artifact.checksum().as_bytes() != &self.payload.artifact_checksum {
            return Err(PortableAdminBundleError::ArtifactChecksumMismatch);
        }
        let observed = validate_program_contract(&artifact, expected_abi)?;
        if observed != self.payload.observed_capabilities {
            return Err(PortableAdminBundleError::CapabilityProofMismatch {
                expected: self.payload.observed_capabilities,
                actual: observed,
            });
        }
        let artifact = engine
            .link_portable_program(artifact.into_compiled())
            .map_err(|error| PortableAdminBundleError::Link(error.to_string()))?;
        Ok(LoadedAdminScript {
            artifact,
            abi: expected_abi.clone(),
            observed_capabilities: observed,
            diagnostics: self.payload.diagnostics,
        })
    }
}

fn validate_program_contract(
    artifact: &PortableProgramArtifact,
    abi: &AdminScriptAbi,
) -> Result<CapabilitySet, PortableAdminBundleError> {
    let function = artifact.function_by_id(abi.entry).ok_or_else(|| {
        PortableAdminBundleError::MissingEntry {
            entry: abi.entry,
            symbol: abi.symbol.clone(),
        }
    })?;
    if function.name != abi.symbol {
        return Err(PortableAdminBundleError::EntrySymbolMismatch {
            expected: abi.symbol.clone(),
            actual: function.name.clone(),
        });
    }
    if function.asyncness != abi.asyncness {
        return Err(PortableAdminBundleError::EntryAsyncnessMismatch {
            expected: abi.asyncness,
            actual: function.asyncness,
        });
    }
    let actual_parameters = u32::try_from(function.params.len()).map_err(|_| {
        PortableAdminBundleError::EntryParameterCountOverflow {
            actual: function.params.len(),
        }
    })?;
    if actual_parameters != abi.parameter_count {
        return Err(PortableAdminBundleError::EntryParameterCountMismatch {
            expected: abi.parameter_count,
            actual: actual_parameters,
        });
    }
    let observed = function.verified_capabilities().ok_or_else(|| {
        PortableAdminBundleError::MissingCapabilityProof {
            symbol: abi.symbol.clone(),
        }
    })?;
    if !abi.capability_ceiling.contains_all(observed) {
        return Err(PortableAdminBundleError::CapabilityCeilingExceeded {
            ceiling: abi.capability_ceiling,
            observed,
        });
    }
    Ok(observed)
}

#[derive(Clone, Debug)]
pub struct LoadedAdminScript {
    artifact: Arc<LinkedArtifact>,
    abi: AdminScriptAbi,
    observed_capabilities: CapabilitySet,
    diagnostics: Vec<PortableAdminDiagnosticSource>,
}

impl LoadedAdminScript {
    #[must_use]
    pub fn artifact(&self) -> &Arc<LinkedArtifact> {
        &self.artifact
    }

    #[must_use]
    pub const fn abi(&self) -> &AdminScriptAbi {
        &self.abi
    }

    #[must_use]
    pub const fn observed_capabilities(&self) -> CapabilitySet {
        self.observed_capabilities
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[PortableAdminDiagnosticSource] {
        &self.diagnostics
    }

    #[must_use]
    pub fn into_parts(self) -> (Arc<LinkedArtifact>, AdminScriptAbi) {
        (self.artifact, self.abi)
    }
}

fn codec() -> impl Options {
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .with_little_endian()
        .reject_trailing_bytes()
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
pub enum PortableAdminBundleError {
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
    HostSchemaHashMismatch {
        expected: u64,
        actual: u64,
    },
    AbiMismatch {
        expected: AdminScriptAbi,
        actual: AdminScriptAbi,
    },
    MissingEntry {
        entry: FunctionId,
        symbol: String,
    },
    EntrySymbolMismatch {
        expected: String,
        actual: String,
    },
    EntryAsyncnessMismatch {
        expected: CallableAsyncness,
        actual: CallableAsyncness,
    },
    EntryParameterCountOverflow {
        actual: usize,
    },
    EntryParameterCountMismatch {
        expected: u32,
        actual: u32,
    },
    MissingCapabilityProof {
        symbol: String,
    },
    CapabilityCeilingExceeded {
        ceiling: CapabilitySet,
        observed: CapabilitySet,
    },
    MissingCapabilities {
        available: CapabilitySet,
        required: CapabilitySet,
    },
    CapabilityProofMismatch {
        expected: CapabilitySet,
        actual: CapabilitySet,
    },
    ArtifactChecksumMismatch,
    Program(PortableArtifactError),
    Link(String),
}

impl fmt::Display for PortableAdminBundleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMagic => formatter.write_str("portable admin bundle magic is invalid"),
            Self::Truncated => formatter.write_str("portable admin bundle header is truncated"),
            Self::UnsupportedFormat { expected, actual } => write!(
                formatter,
                "portable admin bundle format {actual} is unsupported; expected {expected}"
            ),
            Self::PayloadTooLarge { maximum, actual } => write!(
                formatter,
                "portable admin bundle payload is {actual} bytes; maximum is {maximum}"
            ),
            Self::LengthMismatch { expected, actual } => write!(
                formatter,
                "portable admin bundle length is {actual} bytes; expected {expected}"
            ),
            Self::ChecksumMismatch => {
                formatter.write_str("portable admin bundle checksum does not match its payload")
            }
            Self::Encode(message) => {
                write!(formatter, "portable admin bundle encode failed: {message}")
            }
            Self::Decode(message) => {
                write!(formatter, "portable admin bundle decode failed: {message}")
            }
            Self::HostSchemaHashMismatch { expected, actual } => write!(
                formatter,
                "portable admin bundle expects host schema hash {expected:016x}, found {actual:016x}"
            ),
            Self::AbiMismatch { expected, actual } => write!(
                formatter,
                "portable admin bundle ABI mismatch: expected {expected:?}, found {actual:?}"
            ),
            Self::MissingEntry { entry, symbol } => write!(
                formatter,
                "portable admin bundle entry `{symbol}` ({}) is absent",
                entry.get()
            ),
            Self::EntrySymbolMismatch { expected, actual } => write!(
                formatter,
                "portable admin entry symbol is `{actual}`; expected `{expected}`"
            ),
            Self::EntryAsyncnessMismatch { expected, actual } => write!(
                formatter,
                "portable admin entry asyncness is {actual:?}; expected {expected:?}"
            ),
            Self::EntryParameterCountOverflow { actual } => write!(
                formatter,
                "portable admin entry has {actual} parameters, exceeding u32::MAX"
            ),
            Self::EntryParameterCountMismatch { expected, actual } => write!(
                formatter,
                "portable admin entry has {actual} parameters; expected {expected}"
            ),
            Self::MissingCapabilityProof { symbol } => write!(
                formatter,
                "portable admin entry `{symbol}` has no compiler capability proof"
            ),
            Self::CapabilityCeilingExceeded { ceiling, observed } => write!(
                formatter,
                "portable admin entry capabilities {observed:?} exceed ceiling {ceiling:?}"
            ),
            Self::MissingCapabilities {
                available,
                required,
            } => write!(
                formatter,
                "receiving Engine capabilities {available:?} do not satisfy {required:?}"
            ),
            Self::CapabilityProofMismatch { expected, actual } => write!(
                formatter,
                "portable admin capability proof is {actual:?}; expected {expected:?}"
            ),
            Self::ArtifactChecksumMismatch => {
                formatter.write_str("portable admin artifact checksum does not match its payload")
            }
            Self::Program(error) => error.fmt(formatter),
            Self::Link(message) => {
                write!(formatter, "portable admin artifact link failed: {message}")
            }
        }
    }
}

impl std::error::Error for PortableAdminBundleError {}

impl From<PortableArtifactError> for PortableAdminBundleError {
    fn from(error: PortableArtifactError) -> Self {
        Self::Program(error)
    }
}
