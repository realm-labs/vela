use vela_bytecode::LinkedProgram;
use vela_common::StateSlot;

use crate::{HostExecution, Value, VmError, VmErrorKind, VmResult};

pub(crate) fn load(
    host: Option<&HostExecution<'_>>,
    program: &LinkedProgram,
    slot: StateSlot,
) -> VmResult<Value> {
    let state = program
        .state(slot)
        .ok_or_else(|| state_error("VM state slot"))?;
    host.and_then(|host| host.state_values.as_deref())
        .and_then(|states| states.get(state.id))
        .ok_or_else(|| state_error("uninitialized VM state"))
}

pub(crate) fn store(
    host: Option<&mut HostExecution<'_>>,
    program: &LinkedProgram,
    slot: StateSlot,
    value: Value,
) -> VmResult<()> {
    let state = program
        .state(slot)
        .ok_or_else(|| state_error("VM state slot"))?;
    let states = host
        .and_then(|host| host.state_values.as_deref_mut())
        .ok_or_else(|| state_error("VM state context"))?;
    states.insert(state.id, value);
    Ok(())
}

fn state_error(operation: &'static str) -> VmError {
    VmError::new(VmErrorKind::TypeMismatch { operation })
}
