#[allow(dead_code)]
#[path = "../benches/external_compare/config.rs"]
mod config;
#[path = "../benches/external_compare/version.rs"]
mod version;
#[path = "../benches/external_compare/workloads.rs"]
mod workloads;

use std::collections::{BTreeMap, BTreeSet};

use mlua::{Function, Lua};
use rhai::{Engine, Scope};
use vela_bytecode::compiler::compile_program_source_with_registry;
use vela_bytecode::linked::InstructionKind;
use vela_bytecode::{BinaryLiteralOp, BinaryLiteralSide};
use vela_bytecode::{Linker, UnlinkedProgram};
use vela_common::ScalarValue;
use vela_common::SourceId;
use vela_vm::Vm;
use vela_vm::owned_value::OwnedValue;

#[test]
fn external_compare_workload_names_are_unique() {
    let mut names = BTreeSet::new();
    for workload in workloads::all_workloads() {
        assert!(
            names.insert(workload.name),
            "duplicate external compare workload name: {}",
            workload.name
        );
    }
}

#[test]
fn external_compare_workloads_have_all_runtime_sources() {
    for workload in workloads::all_workloads() {
        assert!(
            !workload.vela.trim().is_empty(),
            "{} missing vela",
            workload.name
        );
        assert!(
            !workload.lua.trim().is_empty(),
            "{} missing lua",
            workload.name
        );
        assert!(
            !workload.rhai.trim().is_empty(),
            "{} missing rhai",
            workload.name
        );
        assert!(
            !workload.node.trim().is_empty(),
            "{} missing node",
            workload.name
        );
        assert!(
            !workload.python.trim().is_empty(),
            "{} missing python",
            workload.name
        );
    }
}

#[test]
fn external_compare_vela_sources_compile_and_link() {
    let vm = Vm::new().with_standard_natives();
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    for workload in workloads::all_workloads() {
        let program = compile_program_source_with_registry(
            SourceId::new(1),
            workload.vela,
            registry.compile_view(),
        )
        .unwrap_or_else(|error| panic!("{} should compile: {error:?}", workload.name));
        link_program_for_vm(&vm, &program)
            .unwrap_or_else(|error| panic!("{} should link: {error}", workload.name));
    }
}

#[test]
fn scalar_workloads_have_reproducible_opcode_count_reports() {
    let vm = Vm::new().with_standard_natives();
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");

    let scalar = opcode_count_report(&vm, registry.compile_view(), "scalar_branch_loop");
    assert_has_opcode(&scalar, "I64RemImm");
    assert_has_opcode(&scalar, "I64MulImm");
    assert_has_opcode(&scalar, "I64CmpImmJumpIfFalse");
    assert_has_opcode(&scalar, "I64Add");
    assert_has_opcode(&scalar, "Jump");
    assert_has_opcode(&scalar, "I64RangeNext");

    let range = opcode_count_report(&vm, registry.compile_view(), "range_iteration");
    assert_has_opcode(&range, "I64RangeNext");
    assert_has_opcode(&range, "I64Add");
    assert_has_opcode(&range, "I64Sub");

    let function_calls = opcode_count_report(&vm, registry.compile_view(), "function_calls");
    assert_has_opcode(&function_calls, "CallFunction");

    let float_math = opcode_count_report(&vm, registry.compile_view(), "float_math_loop");
    assert_has_opcode(&float_math, "BinaryFloatLiteral::Add::Right");
    assert_has_opcode(&float_math, "Mul");
    assert_has_opcode(&float_math, "Div");
}

#[test]
fn embedded_lua_sources_load_and_run() {
    for workload in workloads::all_workloads() {
        let lua = Lua::new();
        lua.load(workload.lua)
            .exec()
            .unwrap_or_else(|error| panic!("{} lua should load: {error}", workload.name));
        let run = lua
            .globals()
            .get::<Function>("run")
            .unwrap_or_else(|error| panic!("{} lua run function missing: {error}", workload.name));
        let _checksum: i64 = run
            .call(2_i64)
            .unwrap_or_else(|error| panic!("{} lua should run: {error}", workload.name));
    }
}

#[test]
fn embedded_rhai_sources_compile_and_run() {
    let mut engine = Engine::new();
    engine.set_max_call_levels(256);
    engine.set_max_expr_depths(256, 256);
    for workload in workloads::all_workloads() {
        let ast = engine
            .compile(workload.rhai)
            .unwrap_or_else(|error| panic!("{} rhai should compile: {error}", workload.name));
        let mut scope = Scope::new();
        let _checksum = engine
            .call_fn::<i64>(&mut scope, &ast, "run", (2_i64,))
            .unwrap_or_else(|error| panic!("{} rhai should run: {error}", workload.name));
    }
}

#[test]
fn embedded_workload_checksums_match() {
    let iterations = 2_i64;
    let vm = Vm::new().with_standard_natives();
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    let mut rhai_engine = Engine::new();
    rhai_engine.set_max_call_levels(256);
    rhai_engine.set_max_expr_depths(256, 256);

    for workload in workloads::all_workloads() {
        let vela_checksum = run_vela_workload(&vm, registry.compile_view(), workload, iterations)
            .unwrap_or_else(|error| panic!("{} vela should run: {error}", workload.name));
        let lua_checksum = run_lua_workload(workload, iterations)
            .unwrap_or_else(|error| panic!("{} lua should run: {error}", workload.name));
        let rhai_checksum = run_rhai_workload(&rhai_engine, workload, iterations)
            .unwrap_or_else(|error| panic!("{} rhai should run: {error}", workload.name));

        assert_eq!(
            vela_checksum, lua_checksum,
            "{} Vela/Lua checksum mismatch",
            workload.name
        );
        assert_eq!(
            vela_checksum, rhai_checksum,
            "{} Vela/Rhai checksum mismatch",
            workload.name
        );
    }
}

#[test]
fn python_version_parser_accepts_only_python3_versions() {
    assert_eq!(
        version::python_major_from_version_text("Python 3.14.5"),
        Some(3)
    );
    assert_eq!(
        version::python_major_from_version_text("Python 2.7.18"),
        Some(2)
    );
    assert_eq!(version::python_major_from_version_text("not python"), None);
}

#[test]
fn external_compare_config_accepts_profile_parameters() {
    let config = config::BenchConfig::from_iter(
        [
            "--quick",
            "--runtime",
            "vela,lua54",
            "--iterations=12345",
            "--repeats",
            "4",
            "--warmup",
            "2",
            "scalar",
        ]
        .into_iter()
        .map(str::to_owned),
    );

    assert_eq!(config.params.iterations, 12_345);
    assert_eq!(config.params.repeats, 4);
    assert_eq!(config.params.warmup, 2);
    assert!(config.should_run("scalar_branch_loop"));
    assert!(!config.should_run("range_iteration"));
    assert!(config.should_run_runtime("vela"));
    assert!(config.should_run_runtime("lua54"));
    assert!(!config.should_run_runtime("rhai"));
    assert_eq!(config.runtimes_label(), "vela,lua54");
}

fn run_vela_workload(
    vm: &Vm,
    registry: vela_registry::RegistryCompileView<'_>,
    workload: &workloads::Workload,
    iterations: i64,
) -> Result<i64, Box<dyn std::error::Error>> {
    let program = compile_program_source_with_registry(SourceId::new(1), workload.vela, registry)
        .map_err(|error| format!("{error:?}"))?;
    let linked = link_program_for_vm(vm, &program)?;
    let value = vm.run_linked_program(
        &linked,
        "main",
        &[OwnedValue::Scalar(ScalarValue::I64(iterations))],
    )?;
    match value {
        OwnedValue::Scalar(ScalarValue::I64(value)) => Ok(value),
        other => Err(format!("expected i64 checksum, got {other:?}").into()),
    }
}

fn run_lua_workload(
    workload: &workloads::Workload,
    iterations: i64,
) -> Result<i64, Box<dyn std::error::Error>> {
    let lua = Lua::new();
    lua.load(workload.lua).exec()?;
    let run = lua.globals().get::<Function>("run")?;
    Ok(run.call(iterations)?)
}

fn run_rhai_workload(
    engine: &Engine,
    workload: &workloads::Workload,
    iterations: i64,
) -> Result<i64, Box<dyn std::error::Error>> {
    let ast = engine.compile(workload.rhai)?;
    let mut scope = Scope::new();
    Ok(engine.call_fn::<i64>(&mut scope, &ast, "run", (iterations,))?)
}

fn link_program_for_vm(
    vm: &Vm,
    program: &UnlinkedProgram,
) -> Result<vela_bytecode::LinkedProgram, Box<dyn std::error::Error>> {
    let mut linker = Linker::new();
    for id in vm.native_implementation_ids() {
        linker.add_native_implementation(id);
    }
    linker
        .link_program(program)
        .map_err(|error| format!("{error:?}").into())
}

fn opcode_count_report(
    vm: &Vm,
    registry: vela_registry::RegistryCompileView<'_>,
    workload_name: &str,
) -> BTreeMap<&'static str, usize> {
    let workload = workloads::all_workloads()
        .find(|workload| workload.name == workload_name)
        .unwrap_or_else(|| panic!("{workload_name} workload should exist"));
    let program = compile_program_source_with_registry(SourceId::new(1), workload.vela, registry)
        .unwrap_or_else(|error| panic!("{workload_name} should compile: {error:?}"));
    let linked = link_program_for_vm(vm, &program)
        .unwrap_or_else(|error| panic!("{workload_name} should link: {error}"));
    let mut counts = BTreeMap::new();
    for (_, function) in linked.functions() {
        for instruction in &function.instructions {
            *counts.entry(opcode_label(&instruction.kind)).or_insert(0) += 1;
        }
    }
    counts
}

fn assert_has_opcode(counts: &BTreeMap<&'static str, usize>, opcode: &'static str) {
    assert!(
        counts.get(opcode).copied().unwrap_or_default() > 0,
        "expected {opcode} in opcode count report: {counts:#?}"
    );
}

fn opcode_label(kind: &InstructionKind) -> &'static str {
    match kind {
        InstructionKind::LoadConst { .. } => "LoadConst",
        InstructionKind::Move { .. } => "Move",
        InstructionKind::Not { .. } => "Not",
        InstructionKind::Truthy { .. } => "Truthy",
        InstructionKind::Negate { .. } => "Negate",
        InstructionKind::Add { .. } => "Add",
        InstructionKind::Sub { .. } => "Sub",
        InstructionKind::Mul { .. } => "Mul",
        InstructionKind::Div { .. } => "Div",
        InstructionKind::Rem { .. } => "Rem",
        InstructionKind::Equal { .. } => "Equal",
        InstructionKind::NotEqual { .. } => "NotEqual",
        InstructionKind::Less { .. } => "Less",
        InstructionKind::LessEqual { .. } => "LessEqual",
        InstructionKind::Greater { .. } => "Greater",
        InstructionKind::GreaterEqual { .. } => "GreaterEqual",
        InstructionKind::I64Add { .. } => "I64Add",
        InstructionKind::I64Sub { .. } => "I64Sub",
        InstructionKind::I64Mul { .. } => "I64Mul",
        InstructionKind::I64Rem { .. } => "I64Rem",
        InstructionKind::I64AddImm { .. } => "I64AddImm",
        InstructionKind::I64SubImm { .. } => "I64SubImm",
        InstructionKind::I64MulImm { .. } => "I64MulImm",
        InstructionKind::I64RemImm { .. } => "I64RemImm",
        InstructionKind::I64CmpImm { .. } => "I64CmpImm",
        InstructionKind::I64CmpImmJumpIfFalse { .. } => "I64CmpImmJumpIfFalse",
        InstructionKind::BinaryIntLiteral { op, side, .. } => binary_int_literal_label(*op, *side),
        InstructionKind::BinaryFloatLiteral { op, side, .. } => {
            binary_float_literal_label(*op, *side)
        }
        InstructionKind::GuardType { .. } => "GuardType",
        InstructionKind::JumpIfFalse { .. } => "JumpIfFalse",
        InstructionKind::JumpIfNotMissing { .. } => "JumpIfNotMissing",
        InstructionKind::Jump { .. } => "Jump",
        InstructionKind::CallNative { .. } => "CallNative",
        InstructionKind::CallFunction { .. } => "CallFunction",
        InstructionKind::MakeClosure { .. } => "MakeClosure",
        InstructionKind::CallClosure { .. } => "CallClosure",
        InstructionKind::CallMethod { .. } => "CallMethod",
        InstructionKind::CallDynamicMethod { .. } => "CallDynamicMethod",
        InstructionKind::TryPropagate { .. } => "TryPropagate",
        InstructionKind::MakeArray { .. } => "MakeArray",
        InstructionKind::MakeMap { .. } => "MakeMap",
        InstructionKind::MakeRange { .. } => "MakeRange",
        InstructionKind::MakeRecord { .. } => "MakeRecord",
        InstructionKind::MakeEnum { .. } => "MakeEnum",
        InstructionKind::GetRecordSlot { .. } => "GetRecordSlot",
        InstructionKind::SetRecordSlot { .. } => "SetRecordSlot",
        InstructionKind::GetEnumSlot { .. } => "GetEnumSlot",
        InstructionKind::GetIndex { .. } => "GetIndex",
        InstructionKind::GetStringKeyIndex { .. } => "GetStringKeyIndex",
        InstructionKind::SetIndex { .. } => "SetIndex",
        InstructionKind::SetStringKeyIndex { .. } => "SetStringKeyIndex",
        InstructionKind::IterInit { .. } => "IterInit",
        InstructionKind::IterNext { .. } => "IterNext",
        InstructionKind::RangeNext { .. } => "RangeNext",
        InstructionKind::I64RangeNext { .. } => "I64RangeNext",
        InstructionKind::EnumTagEqual { .. } => "EnumTagEqual",
        InstructionKind::LoadGlobal { .. } => "LoadGlobal",
        InstructionKind::HostRead { .. } => "HostRead",
        InstructionKind::HostWrite { .. } => "HostWrite",
        InstructionKind::HostMutate { .. } => "HostMutate",
        InstructionKind::HostRemove { .. } => "HostRemove",
        InstructionKind::HostCall { .. } => "HostCall",
        InstructionKind::Return { .. } => "Return",
    }
}

fn binary_int_literal_label(op: BinaryLiteralOp, side: BinaryLiteralSide) -> &'static str {
    match (op, side) {
        (BinaryLiteralOp::Add, BinaryLiteralSide::Left) => "BinaryIntLiteral::Add::Left",
        (BinaryLiteralOp::Add, BinaryLiteralSide::Right) => "BinaryIntLiteral::Add::Right",
        (BinaryLiteralOp::Sub, BinaryLiteralSide::Left) => "BinaryIntLiteral::Sub::Left",
        (BinaryLiteralOp::Sub, BinaryLiteralSide::Right) => "BinaryIntLiteral::Sub::Right",
        (BinaryLiteralOp::Mul, BinaryLiteralSide::Left) => "BinaryIntLiteral::Mul::Left",
        (BinaryLiteralOp::Mul, BinaryLiteralSide::Right) => "BinaryIntLiteral::Mul::Right",
        (BinaryLiteralOp::Div, BinaryLiteralSide::Left) => "BinaryIntLiteral::Div::Left",
        (BinaryLiteralOp::Div, BinaryLiteralSide::Right) => "BinaryIntLiteral::Div::Right",
        (BinaryLiteralOp::Rem, BinaryLiteralSide::Left) => "BinaryIntLiteral::Rem::Left",
        (BinaryLiteralOp::Rem, BinaryLiteralSide::Right) => "BinaryIntLiteral::Rem::Right",
        (BinaryLiteralOp::Less, BinaryLiteralSide::Left) => "BinaryIntLiteral::Less::Left",
        (BinaryLiteralOp::Less, BinaryLiteralSide::Right) => "BinaryIntLiteral::Less::Right",
        (BinaryLiteralOp::LessEqual, BinaryLiteralSide::Left) => {
            "BinaryIntLiteral::LessEqual::Left"
        }
        (BinaryLiteralOp::LessEqual, BinaryLiteralSide::Right) => {
            "BinaryIntLiteral::LessEqual::Right"
        }
        (BinaryLiteralOp::Greater, BinaryLiteralSide::Left) => "BinaryIntLiteral::Greater::Left",
        (BinaryLiteralOp::Greater, BinaryLiteralSide::Right) => "BinaryIntLiteral::Greater::Right",
        (BinaryLiteralOp::GreaterEqual, BinaryLiteralSide::Left) => {
            "BinaryIntLiteral::GreaterEqual::Left"
        }
        (BinaryLiteralOp::GreaterEqual, BinaryLiteralSide::Right) => {
            "BinaryIntLiteral::GreaterEqual::Right"
        }
    }
}

fn binary_float_literal_label(op: BinaryLiteralOp, side: BinaryLiteralSide) -> &'static str {
    match (op, side) {
        (BinaryLiteralOp::Add, BinaryLiteralSide::Left) => "BinaryFloatLiteral::Add::Left",
        (BinaryLiteralOp::Add, BinaryLiteralSide::Right) => "BinaryFloatLiteral::Add::Right",
        (BinaryLiteralOp::Sub, BinaryLiteralSide::Left) => "BinaryFloatLiteral::Sub::Left",
        (BinaryLiteralOp::Sub, BinaryLiteralSide::Right) => "BinaryFloatLiteral::Sub::Right",
        (BinaryLiteralOp::Mul, BinaryLiteralSide::Left) => "BinaryFloatLiteral::Mul::Left",
        (BinaryLiteralOp::Mul, BinaryLiteralSide::Right) => "BinaryFloatLiteral::Mul::Right",
        (BinaryLiteralOp::Div, BinaryLiteralSide::Left) => "BinaryFloatLiteral::Div::Left",
        (BinaryLiteralOp::Div, BinaryLiteralSide::Right) => "BinaryFloatLiteral::Div::Right",
        (BinaryLiteralOp::Rem, BinaryLiteralSide::Left) => "BinaryFloatLiteral::Rem::Left",
        (BinaryLiteralOp::Rem, BinaryLiteralSide::Right) => "BinaryFloatLiteral::Rem::Right",
        (BinaryLiteralOp::Less, BinaryLiteralSide::Left) => "BinaryFloatLiteral::Less::Left",
        (BinaryLiteralOp::Less, BinaryLiteralSide::Right) => "BinaryFloatLiteral::Less::Right",
        (BinaryLiteralOp::LessEqual, BinaryLiteralSide::Left) => {
            "BinaryFloatLiteral::LessEqual::Left"
        }
        (BinaryLiteralOp::LessEqual, BinaryLiteralSide::Right) => {
            "BinaryFloatLiteral::LessEqual::Right"
        }
        (BinaryLiteralOp::Greater, BinaryLiteralSide::Left) => "BinaryFloatLiteral::Greater::Left",
        (BinaryLiteralOp::Greater, BinaryLiteralSide::Right) => {
            "BinaryFloatLiteral::Greater::Right"
        }
        (BinaryLiteralOp::GreaterEqual, BinaryLiteralSide::Left) => {
            "BinaryFloatLiteral::GreaterEqual::Left"
        }
        (BinaryLiteralOp::GreaterEqual, BinaryLiteralSide::Right) => {
            "BinaryFloatLiteral::GreaterEqual::Right"
        }
    }
}
