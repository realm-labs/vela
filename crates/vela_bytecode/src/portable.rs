use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use bincode::Options;
use serde::{Deserialize, Serialize};

use crate::compiler::CompiledProgram;
use crate::{
    LinkedArtifact, NominalTypeDescriptor, RustBindingSchema, ScriptMethodTable, StateDescriptor,
    UnlinkedCodeObject, UnlinkedProgram,
};
use vela_def::FunctionId;

const MAGIC: &[u8; 8] = b"VELAPRG\0";
// Version 1 bytecode was compiled under implicit Host-borrow release semantics.
// The explicit-release hard switch intentionally has no compatibility loader.
const FORMAT_VERSION: u32 = 2;
const HEADER_LEN: usize = MAGIC.len() + size_of::<u32>() + size_of::<u64>() + 32;
const MAX_PAYLOAD_BYTES: u64 = 64 * 1024 * 1024;

/// Checksum of the canonical encoded portable program payload.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PortableArtifactChecksum([u8; 32]);

impl PortableArtifactChecksum {
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for PortableArtifactChecksum {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// A source-independent, interpreter-ready compile result.
///
/// Loading still binds stable native/type identities against the receiving
/// Engine and runs the bytecode verifier. MIR and process-local executable
/// generations are deliberately not portable in format version 2.
#[derive(Debug)]
pub struct PortableCompiledProgram {
    pub(crate) bytecode: UnlinkedProgram,
    pub(crate) binding_schema: Arc<RustBindingSchema>,
}

/// Versioned portable bytecode artifact before host-registry binding.
#[derive(Clone, Debug, PartialEq)]
pub struct PortableProgramArtifact {
    payload: PortableProgramPayload,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct PortableProgramPayload {
    functions: Vec<UnlinkedCodeObject>,
    states: Vec<StateDescriptor>,
    nominal_types: Vec<NominalTypeDescriptor>,
    script_methods: ScriptMethodTable,
    binding_schema: RustBindingSchema,
}

impl PortableProgramArtifact {
    /// Converts a production compiler result into its portable interpreter
    /// representation without retaining source text or process-local state.
    pub fn from_compiled(program: CompiledProgram) -> Result<Self, PortableArtifactError> {
        let parts = program.into_linker_parts();
        if parts.package_metadata.is_some() {
            return Err(PortableArtifactError::UnsupportedPackageMetadata);
        }
        Ok(Self {
            payload: PortableProgramPayload {
                functions: parts
                    .bytecode
                    .functions
                    .into_iter()
                    .map(canonical_portable_code)
                    .collect(),
                states: parts.bytecode.states,
                nominal_types: parts.bytecode.nominal_types,
                script_methods: parts.bytecode.script_methods,
                binding_schema: Arc::unwrap_or_clone(parts.binding_schema),
            },
        })
    }

    /// Re-encodes an already linked program into stable-ID bytecode suitable
    /// for rebinding in another process.
    pub fn from_linked(artifact: &LinkedArtifact) -> Result<Self, PortableArtifactError> {
        if artifact.package_metadata().is_some() {
            return Err(PortableArtifactError::UnsupportedPackageMetadata);
        }
        Ok(Self {
            payload: PortableProgramPayload {
                functions: artifact
                    .image()
                    .functions()
                    .map(|(_, code)| canonical_portable_code(code.clone()))
                    .collect(),
                states: artifact.image().states().to_vec(),
                nominal_types: artifact.image().nominal_types().to_vec(),
                script_methods: artifact.image().script_methods().clone(),
                binding_schema: artifact.binding_schema().as_ref().clone(),
            },
        })
    }

    /// Encodes a deterministic, versioned and checksummed binary artifact.
    pub fn encode(&self) -> Result<Vec<u8>, PortableArtifactError> {
        let payload = codec()
            .serialize(&self.payload)
            .map_err(|error| PortableArtifactError::Encode(error.to_string()))?;
        if payload.len() as u64 > MAX_PAYLOAD_BYTES {
            return Err(PortableArtifactError::PayloadTooLarge {
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

    /// Decodes and verifies one complete artifact before exposing bytecode.
    pub fn decode(encoded: &[u8]) -> Result<Self, PortableArtifactError> {
        if encoded.len() < HEADER_LEN {
            return Err(PortableArtifactError::Truncated);
        }
        if &encoded[..MAGIC.len()] != MAGIC {
            return Err(PortableArtifactError::InvalidMagic);
        }
        let mut cursor = MAGIC.len();
        let version = read_u32(encoded, &mut cursor);
        if version != FORMAT_VERSION {
            return Err(PortableArtifactError::UnsupportedFormat {
                expected: FORMAT_VERSION,
                actual: version,
            });
        }
        let payload_len = read_u64(encoded, &mut cursor);
        if payload_len > MAX_PAYLOAD_BYTES {
            return Err(PortableArtifactError::PayloadTooLarge {
                maximum: MAX_PAYLOAD_BYTES,
                actual: payload_len,
            });
        }
        let expected_len = HEADER_LEN
            .checked_add(usize::try_from(payload_len).map_err(|_| {
                PortableArtifactError::PayloadTooLarge {
                    maximum: MAX_PAYLOAD_BYTES,
                    actual: payload_len,
                }
            })?)
            .ok_or(PortableArtifactError::PayloadTooLarge {
                maximum: MAX_PAYLOAD_BYTES,
                actual: payload_len,
            })?;
        if encoded.len() != expected_len {
            return Err(PortableArtifactError::LengthMismatch {
                expected: expected_len,
                actual: encoded.len(),
            });
        }
        let checksum = &encoded[cursor..cursor + 32];
        cursor += 32;
        let payload = &encoded[cursor..];
        if blake3::hash(payload).as_bytes() != checksum {
            return Err(PortableArtifactError::ChecksumMismatch);
        }
        let payload = codec()
            .with_limit(MAX_PAYLOAD_BYTES)
            .deserialize(payload)
            .map_err(|error| PortableArtifactError::Decode(error.to_string()))?;
        Ok(Self { payload })
    }

    #[must_use]
    pub fn checksum(&self) -> PortableArtifactChecksum {
        let payload = codec()
            .serialize(&self.payload)
            .expect("validated portable payload is always encodable");
        PortableArtifactChecksum::new(*blake3::hash(&payload).as_bytes())
    }

    /// Looks up a stable top-level executable before the artifact is consumed
    /// by a receiving Engine.
    #[must_use]
    pub fn function_by_id(&self, function: FunctionId) -> Option<&UnlinkedCodeObject> {
        self.payload
            .functions
            .iter()
            .find(|code| code.stable_function == Some(function))
    }

    /// Materializes decoded compiler data for one receiving Engine to bind.
    #[must_use]
    pub fn into_compiled(self) -> PortableCompiledProgram {
        let mut bytecode = UnlinkedProgram {
            functions: self.payload.functions,
            function_by_name: BTreeMap::new(),
            function_by_id: BTreeMap::new(),
            states: self.payload.states,
            state_slots_by_name: BTreeMap::new(),
            state_slots_by_id: BTreeMap::new(),
            nominal_types: self.payload.nominal_types,
            script_methods: self.payload.script_methods,
            script_metadata: None,
        };
        bytecode.rebuild_function_index();
        bytecode.rebuild_state_index();
        PortableCompiledProgram {
            bytecode,
            binding_schema: Arc::new(self.payload.binding_schema),
        }
    }
}

fn canonical_portable_code(mut code: UnlinkedCodeObject) -> UnlinkedCodeObject {
    code.compiled_mir = None;
    for instruction in &mut code.instructions {
        instruction.mir_origin = None;
        instruction.mir_budget_charges = Box::new([]);
    }
    code.nested_functions = code
        .nested_functions
        .into_iter()
        .map(canonical_portable_code)
        .collect();
    code
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PortableArtifactError {
    InvalidMagic,
    Truncated,
    UnsupportedFormat { expected: u32, actual: u32 },
    PayloadTooLarge { maximum: u64, actual: u64 },
    LengthMismatch { expected: usize, actual: usize },
    ChecksumMismatch,
    Encode(String),
    Decode(String),
    UnsupportedPackageMetadata,
}

impl fmt::Display for PortableArtifactError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMagic => formatter.write_str("portable artifact magic is invalid"),
            Self::Truncated => formatter.write_str("portable artifact header is truncated"),
            Self::UnsupportedFormat { expected, actual } => write!(
                formatter,
                "portable artifact format {actual} is unsupported; expected {expected}"
            ),
            Self::PayloadTooLarge { maximum, actual } => write!(
                formatter,
                "portable artifact payload is {actual} bytes; maximum is {maximum}"
            ),
            Self::LengthMismatch { expected, actual } => write!(
                formatter,
                "portable artifact length is {actual} bytes; expected {expected}"
            ),
            Self::ChecksumMismatch => {
                formatter.write_str("portable artifact checksum does not match its payload")
            }
            Self::Encode(message) => {
                write!(formatter, "portable artifact encode failed: {message}")
            }
            Self::Decode(message) => {
                write!(formatter, "portable artifact decode failed: {message}")
            }
            Self::UnsupportedPackageMetadata => formatter.write_str(
                "portable artifact format 2 does not yet encode package/provider runtime metadata",
            ),
        }
    }
}

impl std::error::Error for PortableArtifactError {}

#[cfg(test)]
mod tests {
    use vela_common::SourceId;

    use super::*;

    #[test]
    fn portable_program_is_deterministic_and_links_without_source_compilation() {
        let compiled =
            crate::compiler::compile_test_program(SourceId::new(1), "fn main() { return 42; }")
                .expect("compile program");
        let artifact = PortableProgramArtifact::from_compiled(compiled).expect("portable artifact");
        let first = artifact.encode().expect("first encoding");
        let second = artifact.encode().expect("second encoding");
        assert_eq!(first, second);

        let decoded = PortableProgramArtifact::decode(&first).expect("decode artifact");
        assert_eq!(decoded, artifact);
        assert_eq!(decoded.checksum(), artifact.checksum());

        let linked = crate::Linker::new()
            .link_portable_program(decoded.into_compiled())
            .expect("link decoded artifact");
        assert!(linked.program().entry_point_by_name("main").is_some());
        let stable_main =
            vela_def::script_function_id(vela_package::PackageId::anonymous().as_str(), "main");
        assert!(linked.program().entry_point_by_id(stable_main).is_some());
        assert!(linked.verified_mir().roots().next().is_none());
        linked.verify().expect("verify linked artifact");
    }

    #[test]
    fn portable_program_rejects_corruption_and_old_implicit_release_format() {
        let compiled =
            crate::compiler::compile_test_program(SourceId::new(2), "fn main() { return 7; }")
                .expect("compile program");
        let artifact = PortableProgramArtifact::from_compiled(compiled).expect("portable artifact");
        let mut corrupted = artifact.encode().expect("encode artifact");
        let last = corrupted.last_mut().expect("payload byte");
        *last ^= 0x40;
        assert_eq!(
            PortableProgramArtifact::decode(&corrupted),
            Err(PortableArtifactError::ChecksumMismatch)
        );

        let mut old_implicit_release = artifact.encode().expect("encode artifact");
        old_implicit_release[MAGIC.len()..MAGIC.len() + size_of::<u32>()]
            .copy_from_slice(&1_u32.to_le_bytes());
        assert_eq!(
            PortableProgramArtifact::decode(&old_implicit_release),
            Err(PortableArtifactError::UnsupportedFormat {
                expected: FORMAT_VERSION,
                actual: 1,
            })
        );
    }
}
