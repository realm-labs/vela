use vela_bytecode::{DebugNameId, InstructionOffset, LinkedCodeObject};
use vela_common::SourceId;
use vela_vm::owned_value::OwnedValue;

use crate::engine::Engine;
use crate::runtime::{CallArgs, CallOptions, Runtime};

#[test]
fn runtime_bytecode_profile_counts_linked_instruction_offsets() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
fn main() {
    return 1 + 2;
}
"#,
        )
        .expect("profile source should compile");
    let mut runtime = Runtime::new(engine, program);
    let main_name = linked_function(&runtime, "main").debug_name;

    assert_eq!(
        runtime
            .state
            .bytecode_profile
            .instruction_hit_count(main_name, InstructionOffset(0)),
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
        runtime
            .state
            .bytecode_profile
            .instruction_hit_count(main_name, InstructionOffset(0)),
        Some(1)
    );

    runtime
        .call("main", CallArgs::new(), CallOptions::unbounded())
        .expect("main should run again");
    assert_eq!(
        runtime
            .state
            .bytecode_profile
            .instruction_hit_count(main_name, InstructionOffset(0)),
        Some(2)
    );
}

#[test]
fn accepted_hot_reload_clears_runtime_bytecode_profile_counts() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(
            SourceId::new(1),
            r#"
fn main() {
    return 1;
}
"#,
        )
        .expect("initial hot reload source should compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let initial_name = linked_function(&runtime, "main").debug_name;

    let first = runtime
        .call("main", CallArgs::new(), CallOptions::unbounded())
        .expect("initial main should run");
    assert_eq!(
        runtime.value_to_owned(&first),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
    assert_eq!(
        runtime
            .state
            .bytecode_profile
            .instruction_hit_count(initial_name, InstructionOffset(0)),
        Some(1)
    );

    let update = runtime
        .compile_hot_reload_update_with_id(
            SourceId::new(2),
            r#"
fn main() {
    return 2;
}
"#,
        )
        .expect("runtime should compile hot reload update")
        .expect("compatible return-value change should be accepted");
    let report = runtime
        .apply_hot_update(update)
        .expect("hot reload update should apply");
    assert!(report.accepted);

    let reloaded_name = linked_function(&runtime, "main").debug_name;
    assert_eq!(
        runtime
            .state
            .bytecode_profile
            .instruction_hit_count(reloaded_name, InstructionOffset(0)),
        Some(0)
    );

    let second = runtime
        .call("main", CallArgs::new(), CallOptions::unbounded())
        .expect("reloaded main should run");
    assert_eq!(
        runtime.value_to_owned(&second),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );
    assert_eq!(
        runtime
            .state
            .bytecode_profile
            .instruction_hit_count(reloaded_name, InstructionOffset(0)),
        Some(1)
    );
}

fn linked_function<'runtime>(runtime: &'runtime Runtime, name: &str) -> &'runtime LinkedCodeObject {
    let program = runtime.image.linked_program();
    let debug_name = debug_name_id(program, name);
    let function = program
        .entry_point(debug_name)
        .unwrap_or_else(|| panic!("{name} should be an entry point"));
    program
        .function(function)
        .unwrap_or_else(|| panic!("{name} should have linked function code"))
}

fn debug_name_id(program: &vela_bytecode::LinkedProgram, name: &str) -> DebugNameId {
    program
        .entry_points()
        .find_map(|(id, _)| (program.debug_name(id) == name).then_some(id))
        .unwrap_or_else(|| panic!("{name} should have a debug-name id"))
}
