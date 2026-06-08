use vela_bytecode::{CallArgument, InstructionOffset, ProgramCode, Register};
use vela_common::Span;

use crate::{
    CallFrame, ExecutionBudget, ExecutionCall, HeapExecution, HostExecution, SmallStorage, Value,
    Vm, VmError, VmErrorKind, VmResult, store_value_in_heap_if_needed,
};

pub(crate) struct ScriptFunctionCall<'a> {
    pub(crate) dst: Register,
    pub(crate) name: &'a str,
    pub(crate) args: &'a [CallArgument],
    pub(crate) call_site: Option<Span>,
    pub(crate) call_site_offset: InstructionOffset,
}

pub(crate) fn dispatch_script_function_call(
    vm: &Vm,
    program: Option<&dyn ProgramCode>,
    host: &mut Option<&mut HostExecution<'_>>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    frame: &mut CallFrame,
    call: ScriptFunctionCall<'_>,
) -> VmResult<()> {
    let program = program.ok_or_else(|| {
        VmError::new(VmErrorKind::UnknownFunction {
            name: call.name.to_owned(),
        })
    })?;
    let function = program.function(call.name).ok_or_else(|| {
        VmError::new(VmErrorKind::UnknownFunction {
            name: call.name.to_owned(),
        })
    })?;
    let values = script_call_args_from_call_arguments(frame, call.args)?;
    let protected_root_len = heap.as_deref_mut().map(|heap| heap.push_frame_roots(frame));
    let result = vm.execute_call(
        ExecutionCall {
            code: function,
            program: Some(program),
            captures: &[],
            args: values.as_slice(),
            call_site: call.call_site,
            call_site_offset: Some(call.call_site_offset),
            inline_caches: None,
        },
        host.as_deref_mut(),
        heap.as_deref_mut(),
        budget.as_deref_mut(),
    );
    if let (Some(heap), Some(protected_root_len)) = (heap.as_deref_mut(), protected_root_len) {
        heap.truncate_protected_roots(protected_root_len);
    }
    let result =
        store_value_in_heap_if_needed(result?, heap.as_deref_mut(), budget.as_deref_mut())?;
    frame.write(call.dst, result)
}

#[inline]
pub(crate) fn script_call_args_from_call_arguments(
    frame: &CallFrame,
    args: &[CallArgument],
) -> VmResult<SmallStorage<Value>> {
    SmallStorage::try_from_slice_map(args, 4, |arg| match arg {
        CallArgument::Register(register) => Ok(*frame.read(*register)?),
        CallArgument::Missing => Ok(Value::Missing),
    })
}
