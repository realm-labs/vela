use vela_bytecode::compiler::{
    compile_function_source, compile_program_source, compile_program_source_with_registry,
};
use vela_bytecode::{Linker, UnlinkedProgram};
use vela_common::SourceId;

use crate::owned_value::OwnedValue;
use crate::{ExecutionBudget, Vm, VmErrorKind};

mod core;
mod option_result;
mod option_result_chains;

fn run_linked_stdlib_test_program_with_budget(
    vm: &Vm,
    program: &UnlinkedProgram,
    entry: &str,
    args: &[OwnedValue],
    budget: &mut ExecutionBudget,
) -> crate::VmResult<OwnedValue> {
    let mut linker = Linker::new();
    vm.native_ids
        .keys()
        .chain(vm.host_native_ids.keys())
        .copied()
        .for_each(|id| linker.add_native_implementation(id));
    let linked = linker
        .link_program(program)
        .expect("stdlib test program should link");
    vm.run_linked_program_with_budget(&linked, entry, args, budget)
}

fn compile_standard_program_source(
    source: SourceId,
    text: &str,
) -> vela_bytecode::compiler::error::CompileResult<UnlinkedProgram> {
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    compile_program_source_with_registry(source, text, registry.compile_view())
}
