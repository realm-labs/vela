use std::sync::Arc;

use vela_bytecode::{
    CallArgument, DebugNameId, InstructionOffset, LinkedProgram, Register, ScriptCallMode,
    ScriptFunctionHandle,
};
use vela_common::Span;

use crate::linked_execution::LinkedExecutionCall;
use crate::{
    CallFrame, ExecutionBudget, HeapExecution, HostExecution, SmallStorage, Value, Vm,
    VmBytecodeProfiler, VmError, VmErrorKind, VmInlineCaches, VmResult,
    store_value_in_heap_if_needed,
};

pub(crate) struct LinkedScriptFunctionCallContext<'a> {
    pub(crate) program: &'a LinkedProgram,
    pub(crate) inline_caches: Option<&'a dyn VmInlineCaches>,
    pub(crate) bytecode_profiler: Option<&'a dyn VmBytecodeProfiler>,
    pub(crate) call_site: Option<Span>,
    pub(crate) call_site_offset: Option<InstructionOffset>,
}

pub(crate) struct LinkedScriptFunctionCall<'a> {
    pub(crate) dst: Register,
    pub(crate) function: ScriptFunctionHandle,
    pub(crate) debug_name: DebugNameId,
    pub(crate) mode: ScriptCallMode,
    pub(crate) args: &'a [CallArgument],
}

pub(crate) fn dispatch_linked_script_function_call(
    vm: &Vm,
    context: LinkedScriptFunctionCallContext<'_>,
    host: &mut Option<&mut HostExecution<'_>>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    frame: &mut CallFrame,
    call: LinkedScriptFunctionCall<'_>,
) -> VmResult<()> {
    context.program.function(call.function).ok_or_else(|| {
        VmError::new(VmErrorKind::UnknownFunction {
            name: context.program.debug_name(call.debug_name).to_owned(),
        })
        .with_source_span_if_absent(context.call_site)
    })?;
    let values_storage;
    let values = if call.args.is_empty() {
        &[]
    } else {
        values_storage = script_call_args_from_call_arguments(frame, call.args)?;
        values_storage.as_slice()
    };
    let protected_root_len = heap.as_deref_mut().map(|heap| heap.push_frame_roots(frame));
    let owner = Arc::clone(frame.linked_owner().ok_or_else(|| {
        VmError::new(VmErrorKind::UnknownFunction {
            name: context.program.debug_name(call.debug_name).to_owned(),
        })
    })?);
    let result = vm.execute_linked_call(
        LinkedExecutionCall {
            owner,
            function: call.function,
            captures: &[],
            args: values,
            check_param_guards: matches!(call.mode, ScriptCallMode::Checked),
            call_site: context.call_site,
            call_site_offset: context.call_site_offset,
            inline_caches: context.inline_caches,
            bytecode_profiler: context.bytecode_profiler,
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
        CallArgument::Register(register) => Ok(frame.read(*register)?),
        CallArgument::Missing => Ok(Value::Missing),
    })
}
