use vela_bytecode::{DebugNameId, InstructionOffset, LinkedCodeObject};
use vela_common::SourceId;
use vela_vm::owned_value::OwnedValue;

use crate::engine::Engine;
use crate::runtime::{BytecodeProfileSnapshot, CallArgs, CallOptions, Runtime};

#[test]
fn default_runtime_allocates_no_instruction_profile() {
    let engine = Engine::builder().build().expect("engine should build");
    let mut source = String::new();
    for index in 0..256 {
        source.push_str(&format!("fn helper_{index}() {{ return {index}; }}\n"));
    }
    source.push_str("fn main() { return helper_0(); }");
    let program = engine
        .compile_source_with_id(SourceId::new(1), &source)
        .expect("profile source should compile");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");

    assert!(runtime.bytecode_profile_snapshot().is_none());
    assert!(!runtime.image.execution_data().has_bytecode_profile());
    runtime
        .call("main", CallArgs::new(), CallOptions::unbounded())
        .expect("main should run");
    assert!(runtime.bytecode_profile_snapshot().is_none());
    assert!(!runtime.image.execution_data().has_bytecode_profile());
}

#[test]
fn enabled_profile_counts_offsets_and_reset_clears_the_generation() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(SourceId::new(1), "fn main() { return 1 + 2; }")
        .expect("profile source should compile");
    let mut runtime = Runtime::builder(engine, program)
        .expect("runtime builder")
        .with_bytecode_profiling()
        .build()
        .expect("runtime should initialize");
    let main_name = linked_function(&runtime, "main").debug_name;

    assert_eq!(
        profile_hit(&runtime, main_name, InstructionOffset(0)),
        Some(0)
    );
    let first = runtime
        .call("main", CallArgs::new(), CallOptions::unbounded())
        .expect("main should run");
    assert_eq!(
        runtime.value_to_owned(&first),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(3)))
    );
    assert_eq!(
        profile_hit(&runtime, main_name, InstructionOffset(0)),
        Some(1)
    );
    assert!(runtime.reset_bytecode_profile());
    assert_eq!(
        profile_hit(&runtime, main_name, InstructionOffset(0)),
        Some(0)
    );
}

#[test]
fn aggregate_profile_counters_saturate_instead_of_wrapping() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(SourceId::new(1), "fn main() { return 1; }")
        .expect("profile source should compile");
    let mut runtime = Runtime::builder(engine, program)
        .expect("runtime builder")
        .with_bytecode_profiling()
        .build()
        .expect("runtime should initialize");
    let main_name = linked_function(&runtime, "main").debug_name;
    assert!(
        runtime
            .state
            .generations
            .active_bytecode_profile()
            .expect("enabled profile")
            .set_instruction_hit_count(main_name, InstructionOffset(0), u64::MAX)
    );

    runtime
        .call("main", CallArgs::new(), CallOptions::unbounded())
        .expect("main should run");
    assert_eq!(
        profile_hit(&runtime, main_name, InstructionOffset(0)),
        Some(u64::MAX)
    );
}

#[test]
fn engine_deployment_uses_one_aggregate_profile_across_owned_images() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(SourceId::new(1), "fn main() { return 7; }")
        .expect("profile source should compile");
    let artifact = engine
        .link_compiled_program(program)
        .expect("profile program should link");
    let first_builder = Runtime::builder_from_linked_artifact(engine.clone(), artifact.clone());
    let mut second = Runtime::from_linked_artifact(engine, artifact)
        .expect("second owned runtime should initialize");
    let mut first = first_builder
        .with_bytecode_profiling()
        .build()
        .expect("first runtime should initialize");
    let main_name = linked_function(&first, "main").debug_name;

    first
        .call("main", CallArgs::new(), CallOptions::unbounded())
        .expect("first runtime executes");
    second
        .call("main", CallArgs::new(), CallOptions::unbounded())
        .expect("second runtime executes");

    assert_eq!(
        profile_hit(&first, main_name, InstructionOffset(0)),
        Some(2)
    );
    assert_eq!(
        profile_hit(&second, main_name, InstructionOffset(0)),
        Some(2)
    );
    assert_eq!(
        first.bytecode_profile_snapshot(),
        second.bytecode_profile_snapshot()
    );
}

#[test]
fn accepted_reload_publishes_a_fresh_generation_profile() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(SourceId::new(1), "fn main() { return 1; }")
        .expect("initial hot reload source should compile");
    let mut runtime = Runtime::builder_from_hot_reload_version(engine, initial)
        .with_bytecode_profiling()
        .build()
        .expect("runtime should initialize");
    let initial_name = linked_function(&runtime, "main").debug_name;
    runtime
        .call("main", CallArgs::new(), CallOptions::unbounded())
        .expect("initial main should run");
    let initial_snapshot = runtime
        .bytecode_profile_snapshot()
        .expect("enabled initial profile");
    assert_eq!(
        snapshot_hit(&initial_snapshot, initial_name, InstructionOffset(0)),
        Some(1)
    );

    let update = runtime
        .compile_reload_with_id(SourceId::new(2), "fn main() { return 2; }")
        .expect("runtime should compile hot reload update")
        .expect("compatible return-value change should be accepted");
    let report = runtime
        .apply_reload_update_for_test(update)
        .expect("hot reload update should apply");
    assert!(report.accepted);

    let reloaded_name = linked_function(&runtime, "main").debug_name;
    let reloaded_snapshot = runtime
        .bytecode_profile_snapshot()
        .expect("enabled reloaded profile");
    assert_ne!(
        initial_snapshot.generation(),
        reloaded_snapshot.generation()
    );
    assert_eq!(
        snapshot_hit(&reloaded_snapshot, reloaded_name, InstructionOffset(0)),
        Some(0)
    );
    runtime
        .call("main", CallArgs::new(), CallOptions::unbounded())
        .expect("reloaded main should run");
    assert_eq!(
        profile_hit(&runtime, reloaded_name, InstructionOffset(0)),
        Some(1)
    );
}

fn profile_hit<I>(
    runtime: &crate::runtime::RuntimeImpl<I>,
    function: DebugNameId,
    offset: InstructionOffset,
) -> Option<u64>
where
    I: crate::runtime::RuntimeImageStorage,
{
    runtime
        .bytecode_profile_snapshot()
        .as_ref()
        .and_then(|snapshot| snapshot_hit(snapshot, function, offset))
}

fn snapshot_hit(
    snapshot: &BytecodeProfileSnapshot,
    function: DebugNameId,
    offset: InstructionOffset,
) -> Option<u64> {
    snapshot
        .functions()
        .iter()
        .find(|profile| profile.debug_name() == function)?
        .instruction_hits()
        .get(offset.0)
        .copied()
}

fn linked_function<'runtime, I>(
    runtime: &'runtime crate::runtime::RuntimeImpl<I>,
    name: &str,
) -> &'runtime LinkedCodeObject
where
    I: crate::runtime::RuntimeImageStorage,
{
    let program = runtime.image.linked_program();
    let debug_name = program
        .entry_points()
        .find_map(|(id, _)| (program.debug_name(id) == name).then_some(id))
        .unwrap_or_else(|| panic!("{name} should have a debug-name id"));
    let function = program
        .entry_point(debug_name)
        .unwrap_or_else(|| panic!("{name} should be an entry point"));
    program
        .function(function)
        .unwrap_or_else(|| panic!("{name} should have linked function code"))
}
