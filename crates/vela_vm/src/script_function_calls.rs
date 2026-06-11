use vela_bytecode::{
    CallArgument, DebugNameId, InstructionOffset, LinkedProgram, Register, ScriptCallMode,
    ScriptFunctionHandle, UnlinkedProgramCode,
};
use vela_common::Span;
use vela_def::FunctionId;

use crate::linked_execution::LinkedExecutionCall;
use crate::{
    CallFrame, ExecutionBudget, ExecutionCall, HeapExecution, HostExecution, SmallStorage, Value,
    Vm, VmError, VmErrorKind, VmInlineCaches, VmResult, store_value_in_heap_if_needed,
};

pub(crate) struct ScriptFunctionCall<'a> {
    pub(crate) dst: Register,
    pub(crate) target: FunctionId,
    pub(crate) name: &'a str,
    pub(crate) mode: ScriptCallMode,
    pub(crate) args: &'a [CallArgument],
    pub(crate) call_site: Option<Span>,
    pub(crate) call_site_offset: InstructionOffset,
}

pub(crate) fn dispatch_script_function_call(
    vm: &Vm,
    program: Option<&dyn UnlinkedProgramCode>,
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
    let function = program.function_by_id(call.target).ok_or_else(|| {
        VmError::new(VmErrorKind::UnknownFunction {
            name: call.name.to_owned(),
        })
    })?;
    let values_storage;
    let values = if call.args.is_empty() {
        &[]
    } else {
        values_storage = script_call_args_from_call_arguments(frame, call.args)?;
        values_storage.as_slice()
    };
    let protected_root_len = heap.as_deref_mut().map(|heap| heap.push_frame_roots(frame));
    let result = vm.execute_call(
        ExecutionCall {
            code: function,
            program: Some(program),
            captures: &[],
            args: values,
            check_param_guards: matches!(call.mode, ScriptCallMode::Checked),
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

pub(crate) struct LinkedScriptFunctionCallContext<'a> {
    pub(crate) program: &'a LinkedProgram,
    pub(crate) inline_caches: Option<&'a dyn VmInlineCaches>,
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
    let function_code = context.program.function(call.function).ok_or_else(|| {
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
    let result = vm.execute_linked_call(
        LinkedExecutionCall {
            code: function_code,
            program: context.program,
            captures: &[],
            args: values,
            check_param_guards: matches!(call.mode, ScriptCallMode::Checked),
            call_site: context.call_site,
            call_site_offset: context.call_site_offset,
            inline_caches: context.inline_caches,
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
