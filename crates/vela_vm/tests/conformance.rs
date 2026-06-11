use vela_bytecode::{Linker, compiler::compile_module_sources_with_registry};
use vela_common::SourceId;
use vela_hir::module_graph::{ModulePath, ModuleSource};
use vela_vm::Vm;
use vela_vm::owned_value::OwnedValue;

#[test]
fn core_language_fixture_executes() {
    let core = include_str!("../../../tests/fixtures/conformance/core_language.vela");
    let reward = include_str!("../../../tests/fixtures/conformance/reward_module.vela");
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    let program = compile_module_sources_with_registry(
        &[
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
        ],
        registry.compile_view(),
    )
    .expect("core language conformance fixture should compile");
    let mut linker = Linker::with_registry(&registry);
    for spec in vela_stdlib::STD_FUNCTIONS {
        linker.add_native_implementation(spec.id());
    }
    let linked = linker
        .link_program(&program)
        .expect("core language conformance fixture should link");
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let result = vm
        .run_linked_program(&linked, "conformance::core::main", &[])
        .expect("core language conformance fixture should run");

    assert_eq!(
        result,
        OwnedValue::Scalar(vela_common::ScalarValue::I64(609))
    );
}
