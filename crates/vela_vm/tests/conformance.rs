use vela_bytecode::compiler::compile_module_sources;
use vela_common::SourceId;
use vela_hir::module_graph::{ModulePath, ModuleSource};
use vela_vm::Vm;
use vela_vm::owned_value::OwnedValue as Value;

#[test]
fn core_language_fixture_executes() {
    let core = include_str!("../../../tests/fixtures/conformance/core_language.vela");
    let reward = include_str!("../../../tests/fixtures/conformance/reward_module.vela");
    let program = compile_module_sources(&[
        ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_qualified("conformance::core"),
            core,
        ),
        ModuleSource::new(
            SourceId::new(2),
            ModulePath::from_qualified("conformance::reward"),
            reward,
        ),
    ])
    .expect("core language conformance fixture should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let result = vm
        .run_program(&program, "conformance::core::main", &[])
        .expect("core language conformance fixture should run");

    assert_eq!(result, Value::Int(609));
}
