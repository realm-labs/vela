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
// Version 5 adds portable physical coverage for verifier-selected interpreter
// units. The hard switch intentionally has no compatibility loader or rewrite.
const FORMAT_VERSION: u32 = 5;
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
/// generations are deliberately not portable in format version 5.
#[derive(Debug)]
pub struct PortableCompiledProgram {
    pub(crate) bytecode: UnlinkedProgram,
    pub(crate) binding_schema: Arc<RustBindingSchema>,
    pub(crate) required_features: crate::ArtifactFeatureSet,
    pub(crate) task_targets: Box<[crate::ArtifactTaskTarget]>,
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
    required_features: crate::ArtifactFeatureSet,
    task_targets: Box<[crate::ArtifactTaskTarget]>,
}

impl PortableProgramArtifact {
    /// Converts a production compiler result into its portable interpreter
    /// representation without retaining source text or process-local state.
    pub fn from_compiled(program: CompiledProgram) -> Result<Self, PortableArtifactError> {
        let parts = program.into_linker_parts();
        if parts.package_metadata.is_some() {
            return Err(PortableArtifactError::UnsupportedPackageMetadata);
        }
        let task_targets = crate::artifact::collect_compiled_task_targets(
            &parts.verified_mir,
            &parts.mir_executables,
        )
        .map_err(|error| PortableArtifactError::TaskMetadata(error.to_string()))?;
        let required_features = if task_targets.is_empty() {
            crate::ArtifactFeatureSet::empty()
        } else {
            crate::ArtifactFeatureSet::host_scoped_tasks()
        };
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
                required_features,
                task_targets,
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
                required_features: artifact.required_features(),
                task_targets: artifact.task_targets().to_vec().into_boxed_slice(),
            },
        })
    }

    /// Encodes a deterministic, versioned and checksummed binary artifact.
    pub fn encode(&self) -> Result<Vec<u8>, PortableArtifactError> {
        validate_task_metadata(&self.payload)?;
        validate_selected_plans(&self.payload)?;
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
        let payload: PortableProgramPayload = codec()
            .with_limit(MAX_PAYLOAD_BYTES)
            .deserialize(payload)
            .map_err(|error| PortableArtifactError::Decode(error.to_string()))?;
        validate_task_metadata(&payload)?;
        validate_selected_plans(&payload)?;
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
            required_features: self.payload.required_features,
            task_targets: self.payload.task_targets,
        }
    }
}

fn canonical_portable_code(mut code: UnlinkedCodeObject) -> UnlinkedCodeObject {
    code.compiled_mir = None;
    for slot in &mut code.frame.slots {
        slot.local = None;
    }
    for instruction in &mut code.instructions {
        instruction.mir_origin = None;
        instruction.mir_budget_charges = Box::new([]);
    }
    for selected in &mut code.selected_units {
        selected.mir_statement = None;
        selected.mir_terminator = None;
    }
    code.nested_functions = code
        .nested_functions
        .into_iter()
        .map(canonical_portable_code)
        .collect();
    code
}

fn validate_task_metadata(payload: &PortableProgramPayload) -> Result<(), PortableArtifactError> {
    if payload.required_features.has_unknown() {
        return Err(PortableArtifactError::UnsupportedFeatures {
            required: payload.required_features.bits(),
            supported: crate::ArtifactFeatureSet::SUPPORTED.bits(),
        });
    }
    let task_feature = payload
        .required_features
        .contains(crate::ArtifactFeatureSet::host_scoped_tasks());
    if task_feature != !payload.task_targets.is_empty() {
        return Err(PortableArtifactError::TaskMetadata(
            "host-scoped task feature bit disagrees with the target table".to_owned(),
        ));
    }
    Ok(())
}

fn validate_selected_plans(payload: &PortableProgramPayload) -> Result<(), PortableArtifactError> {
    fn validate(code: &UnlinkedCodeObject) -> Result<(), PortableArtifactError> {
        crate::selected_plan::verify_selected_physical_units(code)
            .map_err(|detail| PortableArtifactError::SelectedPlan(detail.to_owned()))?;
        for nested in &code.nested_functions {
            validate(nested)?;
        }
        Ok(())
    }

    for code in &payload.functions {
        validate(code)?;
    }
    Ok(())
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
    TaskMetadata(String),
    SelectedPlan(String),
    UnsupportedFeatures { required: u64, supported: u64 },
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
                "portable artifact format 5 does not yet encode package/provider runtime metadata",
            ),
            Self::TaskMetadata(message) => {
                write!(formatter, "portable task metadata is invalid: {message}")
            }
            Self::SelectedPlan(message) => {
                write!(formatter, "portable selected plan is invalid: {message}")
            }
            Self::UnsupportedFeatures {
                required,
                supported,
            } => write!(
                formatter,
                "portable artifact requires feature bits {required:#x}; supported bits are {supported:#x}"
            ),
        }
    }
}

impl std::error::Error for PortableArtifactError {}

#[cfg(test)]
mod tests {
    use vela_common::SourceId;

    use super::*;
    use crate::InstructionOffset;

    fn portable_task_artifact() -> PortableProgramArtifact {
        let compiled = crate::compiler::compile_test_program(
            SourceId::new(3),
            r#"
state current: i64 = 0;

async fn worker(value: Any) -> Any {
    let snapshot = current;
    return value;
}

fn continuation(result: Result<Any, task::Error>, turn: i64 = 7) {
    current = turn;
}

fn main(value: Any) {
    task::spawn_scoped_then(worker(value), continuation);
}
"#,
        )
        .expect("compile real task program");
        PortableProgramArtifact::from_compiled(compiled).expect("portable task artifact")
    }

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
    fn portable_v5_round_trips_selected_physical_coverage_from_source_and_linked_artifacts() {
        let compiled = crate::compiler::compile_test_program(
            SourceId::new(77),
            "fn main(value: i64) -> i64 { if value > 4 { return 1; } return 0; }",
        )
        .expect("compile selected branch");
        let artifact = PortableProgramArtifact::from_compiled(compiled)
            .expect("portable selected-plan artifact");
        let main = artifact
            .payload
            .functions
            .iter()
            .find(|code| code.name == "main")
            .expect("main function");
        assert_eq!(main.selected_units.len(), 1);
        assert_eq!(main.selected_units[0].source_points().len(), 2);
        assert_eq!(main.selected_units[0].exits().len(), 2);

        let encoded = artifact.encode().expect("encode selected plan");
        let decoded = PortableProgramArtifact::decode(&encoded).expect("decode selected plan");
        assert_eq!(decoded, artifact);
        let linked = crate::Linker::new()
            .link_portable_program(decoded.into_compiled())
            .expect("link selected plan");
        assert!(
            linked
                .program()
                .functions()
                .any(|(_, code)| code.selected_units.len() == 1)
        );
        let reencoded =
            PortableProgramArtifact::from_linked(&linked).expect("re-encode linked selected plan");
        assert_eq!(reencoded, artifact);
        assert_eq!(reencoded.checksum(), artifact.checksum());
    }

    #[test]
    fn portable_v5_rejects_checksum_valid_invalid_selected_coverage() {
        let compiled = crate::compiler::compile_test_program(
            SourceId::new(78),
            "fn main(value: i64) -> i64 { if value > 4 { return 1; } return 0; }",
        )
        .expect("compile selected branch");
        let mut artifact = PortableProgramArtifact::from_compiled(compiled)
            .expect("portable selected-plan artifact");
        let main = artifact
            .payload
            .functions
            .iter_mut()
            .find(|code| code.name == "main")
            .expect("main function");
        main.selected_units[0].exits[1] = InstructionOffset(usize::MAX);

        let payload = codec()
            .serialize(&artifact.payload)
            .expect("serialize malformed payload");
        let mut encoded = Vec::new();
        encoded.extend_from_slice(MAGIC);
        encoded.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
        encoded.extend_from_slice(&(payload.len() as u64).to_le_bytes());
        encoded.extend_from_slice(blake3::hash(&payload).as_bytes());
        encoded.extend_from_slice(&payload);
        assert!(matches!(
            PortableProgramArtifact::decode(&encoded),
            Err(PortableArtifactError::SelectedPlan(_))
        ));
    }

    #[test]
    fn portable_v5_round_trips_sealed_task_metadata_and_feature_bits() {
        let artifact = portable_task_artifact();
        let [target] = artifact.payload.task_targets.as_ref() else {
            panic!("real task program should seal one target");
        };
        assert_eq!(
            target.operation,
            crate::ArtifactTaskOperation::SpawnScopedThen
        );
        assert_eq!(target.worker_debug_name, "worker");
        assert_eq!(
            target.worker_signature.parameter_detachability.as_ref(),
            [vela_common::Detachability::RuntimeChecked]
        );
        assert_eq!(
            target.worker_signature.result_detachability,
            vela_common::Detachability::RuntimeChecked
        );
        assert!(target.worker_signature.effects.state_read);
        let continuation = target
            .continuation
            .as_ref()
            .expect("spawn_scoped_then continuation");
        assert_eq!(continuation.debug_name, "continuation");
        assert_eq!(continuation.resume_parameters.len(), 1);
        assert_eq!(
            continuation.resume_parameters[0].contract,
            Some(vela_mir::MirTypeContract::Primitive(
                vela_common::PrimitiveTag::I64,
            ))
        );
        assert!(continuation.resume_parameters[0].has_default);
        assert!(continuation.effects.state_write);

        let encoded = artifact.encode().expect("encode v5 task metadata");
        let decoded = PortableProgramArtifact::decode(&encoded).expect("decode v5 task metadata");
        assert_eq!(
            decoded.payload.required_features,
            artifact.payload.required_features
        );
        assert_eq!(decoded.payload.task_targets, artifact.payload.task_targets);
        let linked = crate::Linker::new()
            .link_portable_program(decoded.into_compiled())
            .expect("link v5 task metadata");
        assert!(
            linked
                .required_features()
                .contains(crate::ArtifactFeatureSet::host_scoped_tasks())
        );
        assert_eq!(
            linked.task_targets(),
            artifact.payload.task_targets.as_ref()
        );

        let mut artifact = artifact;
        artifact.payload.required_features = crate::ArtifactFeatureSet::from_bits(1 << 63);
        assert!(matches!(
            artifact.encode(),
            Err(PortableArtifactError::UnsupportedFeatures { .. })
        ));
    }

    #[test]
    fn portable_v5_rejects_corrupted_task_slots_and_continuation_shape() {
        fn assert_link_rejects(
            mut artifact: PortableProgramArtifact,
            corrupt: impl FnOnce(&mut crate::ArtifactTaskTarget),
        ) {
            corrupt(&mut artifact.payload.task_targets[0]);
            let encoded = artifact
                .encode()
                .expect("structurally encodable corrupted task artifact");
            let decoded = PortableProgramArtifact::decode(&encoded)
                .expect("checksum-valid corrupted task artifact");
            assert!(matches!(
                crate::Linker::new().link_portable_program(decoded.into_compiled()),
                Err(crate::LinkError::InvalidTaskMetadata(_))
            ));
        }

        assert_link_rejects(portable_task_artifact(), |target| {
            target.caller_target = u32::MAX;
        });
        assert_link_rejects(portable_task_artifact(), |target| {
            target.worker_target = u32::MAX;
        });
        assert_link_rejects(portable_task_artifact(), |target| {
            target.operation = crate::ArtifactTaskOperation::SpawnScoped;
        });
        assert_link_rejects(portable_task_artifact(), |target| {
            target.continuation.as_mut().expect("continuation").target = u32::MAX;
        });
        assert_link_rejects(portable_task_artifact(), |target| {
            target.worker_signature.asyncness = vela_common::CallableAsyncness::Sync;
        });

        let mut feature_mismatch = portable_task_artifact();
        feature_mismatch.payload.required_features = crate::ArtifactFeatureSet::empty();
        assert!(matches!(
            feature_mismatch.encode(),
            Err(PortableArtifactError::TaskMetadata(_))
        ));
    }

    #[test]
    fn portable_program_rejects_corruption_and_pre_task_metadata_formats() {
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

        for old_version in [1_u32, 2, 3, 4] {
            let mut old = artifact.encode().expect("encode artifact");
            old[MAGIC.len()..MAGIC.len() + size_of::<u32>()]
                .copy_from_slice(&old_version.to_le_bytes());
            assert_eq!(
                PortableProgramArtifact::decode(&old),
                Err(PortableArtifactError::UnsupportedFormat {
                    expected: FORMAT_VERSION,
                    actual: old_version,
                })
            );
        }
    }
}
