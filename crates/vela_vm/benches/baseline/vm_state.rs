use std::error::Error;
use std::sync::Arc;

use vela_bytecode::LinkedArtifact;

use vela_vm::budget::ExecutionBudget;
use vela_vm::owned_value::OwnedValue;
use vela_vm::{HostExecution, Vm, VmStateValues};

pub(crate) fn run_vm_state(
    vm: &Vm,
    program: &Arc<LinkedArtifact>,
    values: &mut VmStateValues,
) -> Result<OwnedValue, Box<dyn Error>> {
    let mut host = HostExecution {
        state_values: Some(values),
    };
    let mut budget = ExecutionBudget::unbounded();
    Ok(vm.run_linked_program_with_host_budget_and_caches(
        program,
        "main",
        &[],
        &mut host,
        &mut budget,
        None,
    )?)
}
