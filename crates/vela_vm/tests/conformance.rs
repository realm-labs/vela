use vela_bytecode::compiler::compile_module_sources;
use vela_common::SourceId;
use vela_hir::{ModulePath, ModuleSource};
use vela_vm::{Value, Vm};

#[test]
fn core_language_fixture_executes() {
    let core = include_str!("../../../tests/fixtures/conformance/core_language.lang");
    let reward = include_str!("../../../tests/fixtures/conformance/reward_module.lang");
    let program = compile_module_sources(&[
        ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_dotted("conformance.core"),
            core,
        ),
        ModuleSource::new(
            SourceId::new(2),
            ModulePath::from_dotted("conformance.reward"),
            reward,
        ),
    ])
    .expect("core language conformance fixture should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let result = vm
        .run_program(&program, "conformance.core.main", &[])
        .expect("core language conformance fixture should run");

    assert_eq!(result, Value::Int(194));
}
