use vela_bytecode::{LinkedProgram, Register, UnlinkedProgramCode};
use vela_def::MethodId;

use crate::heap::GcRef;
use crate::{
    CallFrame, ExecutionBudget, HeapExecution, HostExecution, Value, Vm, VmResult,
    store_value_in_heap_if_needed,
};

use crate::script_methods::{
    ScriptMethodDispatch, call_method, call_method_id, call_non_mutating_method,
    call_readonly_method_without_callbacks,
};

pub(crate) struct ScriptMethodCall<'a> {
    pub(crate) dst: Register,
    pub(crate) receiver: Register,
    pub(crate) method: &'a str,
    pub(crate) values: &'a [Value],
}

pub(crate) fn dispatch_script_method_call(
    vm: &Vm,
    program: Option<&dyn UnlinkedProgramCode>,
    host: &mut Option<&mut HostExecution<'_>>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    frame: &mut CallFrame,
    call: ScriptMethodCall<'_>,
) -> VmResult<()> {
    if let Some(result) = call_readonly_method_without_callbacks(
        frame.read(call.receiver)?,
        call.method,
        None,
        call.values,
        heap.as_deref(),
    ) {
        let result =
            store_value_in_heap_if_needed(result?, heap.as_deref_mut(), budget.as_deref_mut())?;
        frame.write(call.dst, result)?;
        return Ok(());
    }

    let caller_roots = caller_roots_for_heap(frame, heap.as_deref());
    if let Some(result) = call_non_mutating_method(
        frame.read(call.receiver)?,
        call.method,
        None,
        call.values,
        ScriptMethodDispatch {
            vm,
            program,
            linked_program: None,
            host: host.as_deref_mut(),
            heap: heap.as_deref_mut(),
            budget: budget.as_deref_mut(),
            caller_roots,
        },
    ) {
        let result =
            store_value_in_heap_if_needed(result?, heap.as_deref_mut(), budget.as_deref_mut())?;
        frame.write(call.dst, result)?;
    } else {
        let mut receiver_value = *frame.read(call.receiver)?;
        let caller_roots = caller_roots_for_heap(frame, heap.as_deref());
        let result = call_method(
            &mut receiver_value,
            call.method,
            None,
            call.values,
            ScriptMethodDispatch {
                vm,
                program,
                linked_program: None,
                host: host.as_deref_mut(),
                heap: heap.as_deref_mut(),
                budget: budget.as_deref_mut(),
                caller_roots,
            },
        )?;
        let result =
            store_value_in_heap_if_needed(result, heap.as_deref_mut(), budget.as_deref_mut())?;
        frame.write(call.receiver, receiver_value)?;
        frame.write(call.dst, result)?;
    }
    Ok(())
}

pub(crate) struct ScriptMethodIdCall<'a> {
    pub(crate) dst: Register,
    pub(crate) receiver: Register,
    pub(crate) method: &'a str,
    pub(crate) method_id: MethodId,
    pub(crate) values: &'a [Value],
}

pub(crate) fn dispatch_script_method_id_call(
    vm: &Vm,
    program: Option<&dyn UnlinkedProgramCode>,
    host: &mut Option<&mut HostExecution<'_>>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    frame: &mut CallFrame,
    call: ScriptMethodIdCall<'_>,
) -> VmResult<()> {
    let mut receiver_value = *frame.read(call.receiver)?;
    let caller_roots = caller_roots_for_heap(frame, heap.as_deref());
    let result = call_method_id(
        &mut receiver_value,
        call.method,
        call.method_id,
        call.values,
        ScriptMethodDispatch {
            vm,
            program,
            linked_program: None,
            host: host.as_deref_mut(),
            heap: heap.as_deref_mut(),
            budget: budget.as_deref_mut(),
            caller_roots,
        },
    )?;
    let result = store_value_in_heap_if_needed(result, heap.as_deref_mut(), budget.as_deref_mut())?;
    frame.write(call.receiver, receiver_value)?;
    frame.write(call.dst, result)
}

pub(crate) fn dispatch_linked_method_id_call(
    vm: &Vm,
    program: &LinkedProgram,
    host: &mut Option<&mut HostExecution<'_>>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    frame: &mut CallFrame,
    call: ScriptMethodIdCall<'_>,
) -> VmResult<()> {
    let mut receiver_value = *frame.read(call.receiver)?;
    let caller_roots = caller_roots_for_heap(frame, heap.as_deref());
    let result = call_method_id(
        &mut receiver_value,
        call.method,
        call.method_id,
        call.values,
        ScriptMethodDispatch {
            vm,
            program: None,
            linked_program: Some(program),
            host: host.as_deref_mut(),
            heap: heap.as_deref_mut(),
            budget: budget.as_deref_mut(),
            caller_roots,
        },
    )?;
    let result = store_value_in_heap_if_needed(result, heap.as_deref_mut(), budget.as_deref_mut())?;
    frame.write(call.receiver, receiver_value)?;
    frame.write(call.dst, result)
}

fn caller_roots_for_heap(frame: &CallFrame, heap: Option<&HeapExecution<'_>>) -> Vec<GcRef> {
    if heap.is_some() {
        frame.heap_roots()
    } else {
        Vec::new()
    }
}
