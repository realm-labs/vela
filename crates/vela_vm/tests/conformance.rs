use vela_bytecode::compiler::compile_program_source;
use vela_common::SourceId;
use vela_vm::{Value, Vm};

#[test]
fn core_language_fixture_executes() {
    let source = include_str!("../../../tests/fixtures/conformance/core_language.lang");
    let program = compile_program_source(SourceId::new(1), source)
        .expect("core language conformance fixture should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let result = vm
        .run_program(&program, "main", &[])
        .expect("core language conformance fixture should run");

    assert_eq!(result, Value::Int(131));
}
