#![no_main]

use std::sync::OnceLock;

use bincode::Options;
use libfuzzer_sys::fuzz_target;
use serde::{Deserialize, Serialize};
use vela_bytecode::script_methods::ScriptMethodTable;
use vela_bytecode::{
    ArtifactFeatureSet, ArtifactTaskTarget, InstructionOffset, NominalTypeDescriptor,
    PortableProgramArtifact, Register, RustBindingSchema, ScalarBlockPlanId, ScalarSourcePointId,
    StateDescriptor, UnlinkedCodeObject, UnlinkedInstructionKind,
};
use vela_engine::engine::Engine;

const MAGIC: &[u8; 8] = b"VELAPRG\0";
const FORMAT_VERSION: u32 = 5;
const HEADER_LEN: usize = MAGIC.len() + size_of::<u32>() + size_of::<u64>() + 32;
const MAX_PAYLOAD_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PortableProgramPayload {
    functions: Vec<UnlinkedCodeObject>,
    states: Vec<StateDescriptor>,
    nominal_types: Vec<NominalTypeDescriptor>,
    script_methods: ScriptMethodTable,
    binding_schema: RustBindingSchema,
    required_features: ArtifactFeatureSet,
    task_targets: Box<[ArtifactTaskTarget]>,
}

fuzz_target!(|input: &[u8]| {
    let _ = PortableProgramArtifact::decode(input);
    let Some((&kind, bytes)) = input.split_first() else {
        return;
    };
    let base = base_artifact();
    if kind == b'p' {
        let mut oversized = base.clone();
        oversized[MAGIC.len() + size_of::<u32>()..MAGIC.len() + size_of::<u32>() + 8]
            .copy_from_slice(&(MAX_PAYLOAD_BYTES + 1).to_le_bytes());
        let _ = PortableProgramArtifact::decode(&oversized);
        return;
    }

    let mut payload: PortableProgramPayload = codec()
        .deserialize(&base[HEADER_LEN..])
        .expect("the static fuzz seed is a valid v5 payload");
    let value = fuzz_value(bytes);

    match kind {
        b'h' => {
            let code = scalar_code(&mut payload);
            let instruction = code
                .instructions
                .iter_mut()
                .find(|instruction| {
                    matches!(
                        instruction.kind,
                        UnlinkedInstructionKind::RunScalarBlock { .. }
                    )
                })
                .expect("the static fuzz seed contains a scalar block entry");
            instruction.kind = UnlinkedInstructionKind::RunScalarBlock {
                plan: invalid_plan_id(value),
            };
        }
        b'o' => {
            scalar_range(scalar_code(&mut payload)).cursor = Register(value as u16);
        }
        b'c' => {
            payload
                .functions
                .iter_mut()
                .find_map(|code| code.selected_units.first_mut())
                .expect("the static fuzz seed contains a coverage manifest")
                .set_covered_operations_for_test(value as u16);
        }
        b'e' => {
            scalar_range(scalar_code(&mut payload)).done_target.target = InstructionOffset(value);
        }
        b's' => {
            scalar_range(scalar_code(&mut payload)).header_source = invalid_source_point_id(value);
        }
        _ => return,
    }

    let encoded = encode_unchecked(&payload);
    let _ = PortableProgramArtifact::decode(&encoded);
});

fn scalar_code(payload: &mut PortableProgramPayload) -> &mut UnlinkedCodeObject {
    payload
        .functions
        .iter_mut()
        .find(|code| {
            code.scalar_blocks
                .iter()
                .any(|plan| plan.range_loop.is_some())
        })
        .expect("the static fuzz seed contains a scalar range loop")
}

fn scalar_range(code: &mut UnlinkedCodeObject) -> &mut vela_bytecode::ScalarRangeLoop {
    code.scalar_blocks
        .iter_mut()
        .find_map(|plan| plan.range_loop.as_mut())
        .expect("the static fuzz seed contains a scalar range loop")
}

fn fuzz_value(bytes: &[u8]) -> usize {
    let mut raw = [0_u8; size_of::<usize>()];
    let copied = raw.len().min(bytes.len());
    raw[..copied].copy_from_slice(&bytes[..copied]);
    usize::from_le_bytes(raw)
}

fn invalid_plan_id(value: usize) -> ScalarBlockPlanId {
    let index = (value % u32::MAX as usize).max(1);
    ScalarBlockPlanId::new(index)
}

fn invalid_source_point_id(value: usize) -> ScalarSourcePointId {
    let index = (value % u16::MAX as usize).max(64);
    ScalarSourcePointId::new(index)
}

fn base_artifact() -> &'static Vec<u8> {
    static BASE: OnceLock<Vec<u8>> = OnceLock::new();
    BASE.get_or_init(|| {
        let engine = Engine::builder().build().expect("fuzz seed engine");
        let compiled = engine
            .compile_source(
                "fn branch(value: i64) -> i64 { if value < 10 { return value + 1; } return value; } fn main(limit: i64) -> i64 { let total = 0; for item in 0..limit { total += item + 1 - 1; } return branch(total); }",
            )
            .expect("fuzz seed source");
        PortableProgramArtifact::from_compiled(compiled)
            .expect("portable fuzz seed")
            .encode()
            .expect("encoded fuzz seed")
    })
}

fn encode_unchecked(payload: &PortableProgramPayload) -> Vec<u8> {
    let payload = codec()
        .serialize(payload)
        .expect("fuzz payload remains serializable");
    let mut encoded = Vec::with_capacity(HEADER_LEN + payload.len());
    encoded.extend_from_slice(MAGIC);
    encoded.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    encoded.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    encoded.extend_from_slice(blake3::hash(&payload).as_bytes());
    encoded.extend_from_slice(&payload);
    encoded
}

fn codec() -> impl Options {
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .with_little_endian()
        .reject_trailing_bytes()
}
