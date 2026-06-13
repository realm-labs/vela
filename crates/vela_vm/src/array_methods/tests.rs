use vela_bytecode::compiler::compile_function_source_with_registry;
use vela_bytecode::compiler::error::CompileResult;
use vela_bytecode::{Linker, UnlinkedCodeObject, UnlinkedProgram};
use vela_common::SourceId;

use crate::owned_value::OwnedValue;
use crate::{ExecutionBudget, Vm, VmErrorKind, VmResult};

mod aggregation_and_ordering;
mod higher_order_and_mutation;
mod lookup_and_transform;

fn compile_function_source(
    source: SourceId,
    text: &str,
    function_name: &str,
) -> CompileResult<UnlinkedCodeObject> {
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    compile_function_source_with_registry(source, text, function_name, registry.compile_view())
}

fn run_linked_array_test_code(vm: &Vm, code: UnlinkedCodeObject) -> VmResult<OwnedValue> {
    let mut budget = ExecutionBudget::unbounded();
    run_linked_array_test_code_with_budget(vm, code, &mut budget)
}

fn run_linked_array_test_code_with_budget(
    vm: &Vm,
    code: UnlinkedCodeObject,
    budget: &mut ExecutionBudget,
) -> VmResult<OwnedValue> {
    let entry = code.name.clone();
    let mut program = UnlinkedProgram::new();
    program.insert_function(code);

    let mut linker = Linker::new();
    for id in vm.native_implementation_ids() {
        linker.add_native_implementation(id);
    }
    let linked = linker
        .link_program(&program)
        .expect("array method test code should link");

    vm.run_linked_program_with_budget(&linked, &entry, &[], budget)
}
