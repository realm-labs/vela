use vela_bytecode::linked::LinkedMethodDispatchKind;
use vela_bytecode::{
    CacheSiteId, CallArgument, DebugNameId, InstructionOffset, LinkedProgram, MethodDispatchHandle,
    Register, UnlinkedProgramCode,
};
use vela_common::Span;
use vela_def::MethodId;

use crate::linked_execution::LinkedExecutionCall;
use crate::method_runtime::CallerRoots;
use crate::{
    CallFrame, ExecutionBudget, HeapExecution, HostExecution, SmallStorage, Value, Vm, VmResult,
    store_value_in_heap_if_needed,
};
use crate::{
    MethodInlineCacheEntry, MethodInlineCacheTarget, VmBytecodeProfiler, VmError, VmErrorKind,
    VmInlineCaches, callback_method_dispatch, host_access, script_builtin_methods,
    script_function_calls,
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

pub(crate) struct ScriptMethodRegisterCall<'a> {
    pub(crate) dst: Register,
    pub(crate) receiver: Register,
    pub(crate) method: &'a str,
    pub(crate) args: &'a [CallArgument],
}

pub(crate) fn dispatch_script_method_register_call(
    vm: &Vm,
    program: Option<&dyn UnlinkedProgramCode>,
    host: &mut Option<&mut HostExecution<'_>>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    frame: &mut CallFrame,
    call: ScriptMethodRegisterCall<'_>,
) -> VmResult<()> {
    if call.args.is_empty() {
        return dispatch_script_method_call(
            vm,
            program,
            host,
            heap,
            budget,
            frame,
            ScriptMethodCall {
                dst: call.dst,
                receiver: call.receiver,
                method: call.method,
                values: &[],
            },
        );
    }
    let values = script_function_calls::script_call_args_from_call_arguments(frame, call.args)?;
    dispatch_script_method_call(
        vm,
        program,
        host,
        heap,
        budget,
        frame,
        ScriptMethodCall {
            dst: call.dst,
            receiver: call.receiver,
            method: call.method,
            values: values.as_slice(),
        },
    )
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

    let caller_roots = CallerRoots::for_frame(frame, heap.as_deref());
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
            inline_caches: None,
            bytecode_profiler: None,
        },
    ) {
        let result =
            store_value_in_heap_if_needed(result?, heap.as_deref_mut(), budget.as_deref_mut())?;
        frame.write(call.dst, result)?;
    } else {
        let mut receiver_value = *frame.read(call.receiver)?;
        let caller_roots = CallerRoots::for_frame(frame, heap.as_deref());
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
                inline_caches: None,
                bytecode_profiler: None,
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

pub(crate) struct ScriptMethodIdRegisterCall<'a> {
    pub(crate) dst: Register,
    pub(crate) receiver: Register,
    pub(crate) method: &'a str,
    pub(crate) method_id: MethodId,
    pub(crate) args: &'a [CallArgument],
}

pub(crate) fn dispatch_script_method_id_register_call(
    vm: &Vm,
    program: Option<&dyn UnlinkedProgramCode>,
    host: &mut Option<&mut HostExecution<'_>>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    frame: &mut CallFrame,
    call: ScriptMethodIdRegisterCall<'_>,
) -> VmResult<()> {
    if call.args.is_empty() {
        return dispatch_script_method_id_call(
            vm,
            program,
            host,
            heap,
            budget,
            frame,
            ScriptMethodIdCall {
                dst: call.dst,
                receiver: call.receiver,
                method: call.method,
                method_id: call.method_id,
                values: &[],
            },
        );
    }
    let values = script_function_calls::script_call_args_from_call_arguments(frame, call.args)?;
    dispatch_script_method_id_call(
        vm,
        program,
        host,
        heap,
        budget,
        frame,
        ScriptMethodIdCall {
            dst: call.dst,
            receiver: call.receiver,
            method: call.method,
            method_id: call.method_id,
            values: values.as_slice(),
        },
    )
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
    let caller_roots = CallerRoots::for_frame(frame, heap.as_deref());
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
            inline_caches: None,
            bytecode_profiler: None,
        },
    )?;
    let result = store_value_in_heap_if_needed(result, heap.as_deref_mut(), budget.as_deref_mut())?;
    frame.write(call.receiver, receiver_value)?;
    frame.write(call.dst, result)
}

pub(crate) fn dispatch_linked_method_id_call(
    vm: &Vm,
    context: LinkedMethodRuntimeContext<'_>,
    host: &mut Option<&mut HostExecution<'_>>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    frame: &mut CallFrame,
    call: ScriptMethodIdCall<'_>,
) -> VmResult<()> {
    let mut receiver_value = *frame.read(call.receiver)?;
    let caller_roots = CallerRoots::for_frame(frame, heap.as_deref());
    let result = call_method_id(
        &mut receiver_value,
        call.method,
        call.method_id,
        call.values,
        ScriptMethodDispatch {
            vm,
            program: None,
            linked_program: Some(context.program),
            host: host.as_deref_mut(),
            heap: heap.as_deref_mut(),
            budget: budget.as_deref_mut(),
            caller_roots,
            inline_caches: context.inline_caches,
            bytecode_profiler: context.bytecode_profiler,
        },
    )?;
    let result = store_value_in_heap_if_needed(result, heap.as_deref_mut(), budget.as_deref_mut())?;
    frame.write(call.receiver, receiver_value)?;
    frame.write(call.dst, result)
}

#[derive(Clone, Copy)]
pub(crate) struct LinkedMethodRuntimeContext<'a> {
    pub(crate) program: &'a LinkedProgram,
    pub(crate) inline_caches: Option<&'a dyn VmInlineCaches>,
    pub(crate) bytecode_profiler: Option<&'a dyn VmBytecodeProfiler>,
}

pub(crate) struct LinkedScriptMethodCallContext<'a> {
    pub(crate) program: &'a LinkedProgram,
    pub(crate) inline_caches: Option<&'a dyn VmInlineCaches>,
    pub(crate) cache_site: Option<CacheSiteId>,
    pub(crate) bytecode_profiler: Option<&'a dyn VmBytecodeProfiler>,
    pub(crate) call_site: Option<Span>,
    pub(crate) call_site_offset: Option<InstructionOffset>,
}

pub(crate) struct LinkedScriptMethodCall<'a> {
    pub(crate) dst: Register,
    pub(crate) receiver: Register,
    pub(crate) dispatch: MethodDispatchHandle,
    pub(crate) debug_name: DebugNameId,
    pub(crate) args: &'a [CallArgument],
}

pub(crate) fn dispatch_linked_method_call(
    vm: &Vm,
    context: LinkedScriptMethodCallContext<'_>,
    host: &mut Option<&mut HostExecution<'_>>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    frame: &mut CallFrame,
    call: LinkedScriptMethodCall<'_>,
) -> VmResult<()> {
    let dispatch = linked_method_dispatch_target(&context, call.dispatch, call.debug_name)?;
    let values_storage;
    let values = if call.args.is_empty() {
        &[]
    } else {
        values_storage =
            script_function_calls::script_call_args_from_call_arguments(frame, call.args)?;
        values_storage.as_slice()
    };
    match dispatch.target {
        MethodInlineCacheTarget::Script {
            method_id: _,
            function,
        } => dispatch_linked_script_method_call(
            vm,
            context,
            host,
            heap,
            budget,
            frame,
            ScriptLinkedMethodCall {
                dst: call.dst,
                receiver: call.receiver,
                debug_name: dispatch.debug_name,
                function,
                values,
            },
        ),
        MethodInlineCacheTarget::Value {
            method_id,
            standard_method,
        } => {
            if let Some(result) = linked_standard_value_method_result(
                &context,
                frame,
                heap,
                budget,
                LinkedStandardValueMethodCall {
                    dispatch: call.dispatch,
                    debug_name: dispatch.debug_name,
                    receiver: call.receiver,
                    method_id,
                    standard_method,
                    values,
                },
            ) {
                let result = result?;
                frame.write(call.dst, result)?;
                return Ok(());
            }
            if let Some(result) = linked_callback_value_method_result(
                vm,
                &context,
                host,
                heap,
                budget,
                frame,
                LinkedCallbackValueMethodCall {
                    dispatch: call.dispatch,
                    debug_name: dispatch.debug_name,
                    receiver: call.receiver,
                    method_id,
                    callback_method: None,
                    values,
                },
            ) {
                let result = result?;
                frame.write(call.dst, result)?;
                return Ok(());
            }
            dispatch_linked_method_id_call(
                vm,
                LinkedMethodRuntimeContext {
                    program: context.program,
                    inline_caches: context.inline_caches,
                    bytecode_profiler: context.bytecode_profiler,
                },
                host,
                heap,
                budget,
                frame,
                ScriptMethodIdCall {
                    dst: call.dst,
                    receiver: call.receiver,
                    method: context.program.debug_name(dispatch.debug_name),
                    method_id,
                    values,
                },
            )
        }
        MethodInlineCacheTarget::CallbackValue {
            method_id,
            callback_method,
        } => {
            if let Some(result) = linked_callback_value_method_result(
                vm,
                &context,
                host,
                heap,
                budget,
                frame,
                LinkedCallbackValueMethodCall {
                    dispatch: call.dispatch,
                    debug_name: dispatch.debug_name,
                    receiver: call.receiver,
                    method_id,
                    callback_method: Some(callback_method),
                    values,
                },
            ) {
                let result = result?;
                frame.write(call.dst, result)?;
                return Ok(());
            }
            dispatch_linked_method_id_call(
                vm,
                LinkedMethodRuntimeContext {
                    program: context.program,
                    inline_caches: context.inline_caches,
                    bytecode_profiler: context.bytecode_profiler,
                },
                host,
                heap,
                budget,
                frame,
                ScriptMethodIdCall {
                    dst: call.dst,
                    receiver: call.receiver,
                    method: context.program.debug_name(dispatch.debug_name),
                    method_id,
                    values,
                },
            )
        }
        MethodInlineCacheTarget::Host { method_id } => {
            let return_value = host_access::execute_host_root_method_call(
                host_access::HostAccessRuntime {
                    frame,
                    heap: heap.as_deref_mut(),
                    budget: budget.as_deref_mut(),
                    host: host.as_deref_mut(),
                    inline_caches: context.inline_caches,
                    source_span: context.call_site,
                },
                call.receiver,
                host_access::HostRootMethodCall {
                    method: method_id,
                    args: values,
                    wants_return: true,
                    cache_site: context.cache_site,
                },
            )?;
            if let Some(return_value) = return_value {
                frame.write(call.dst, return_value)?;
            }
            Ok(())
        }
    }
}

#[derive(Clone, Copy)]
struct LinkedMethodDispatchTarget {
    debug_name: DebugNameId,
    target: MethodInlineCacheTarget,
}

fn linked_method_dispatch_target(
    context: &LinkedScriptMethodCallContext<'_>,
    dispatch_handle: MethodDispatchHandle,
    debug_name: DebugNameId,
) -> VmResult<LinkedMethodDispatchTarget> {
    if let Some(site) = context.cache_site
        && let Some(entry) = context
            .inline_caches
            .and_then(|caches| caches.method_dispatch(site))
        && entry.dispatch == dispatch_handle
        && context
            .program
            .method_dispatch(dispatch_handle)
            .is_some_and(|dispatch| {
                entry.debug_name == dispatch.debug_name
                    && cached_method_target_matches_dispatch(&entry.target, &dispatch.kind)
            })
    {
        return Ok(LinkedMethodDispatchTarget {
            debug_name: entry.debug_name,
            target: entry.target,
        });
    }

    let dispatch = context
        .program
        .method_dispatch(dispatch_handle)
        .ok_or_else(|| {
            VmError::new(VmErrorKind::UnknownMethod {
                method: context.program.debug_name(debug_name).to_owned(),
            })
            .with_source_span_if_absent(context.call_site)
        })?;
    let target = method_inline_cache_target(&dispatch.kind);
    if let Some(site) = context.cache_site
        && let Some(caches) = context.inline_caches
    {
        caches.set_method_dispatch(
            site,
            MethodInlineCacheEntry {
                dispatch: dispatch_handle,
                debug_name: dispatch.debug_name,
                target,
            },
        );
    }
    Ok(LinkedMethodDispatchTarget {
        debug_name: dispatch.debug_name,
        target,
    })
}

fn method_inline_cache_target(kind: &LinkedMethodDispatchKind) -> MethodInlineCacheTarget {
    match kind {
        LinkedMethodDispatchKind::Script {
            method_id,
            function,
        } => MethodInlineCacheTarget::Script {
            method_id: *method_id,
            function: *function,
        },
        LinkedMethodDispatchKind::Value { method_id } => MethodInlineCacheTarget::Value {
            method_id: *method_id,
            standard_method: None,
        },
        LinkedMethodDispatchKind::Host { method_id } => MethodInlineCacheTarget::Host {
            method_id: *method_id,
        },
    }
}

fn cached_method_target_matches_dispatch(
    target: &MethodInlineCacheTarget,
    kind: &LinkedMethodDispatchKind,
) -> bool {
    match (target, kind) {
        (
            MethodInlineCacheTarget::Script {
                method_id,
                function,
            },
            LinkedMethodDispatchKind::Script {
                method_id: dispatch_method,
                function: dispatch_function,
            },
        ) => *method_id == *dispatch_method && *function == *dispatch_function,
        (
            MethodInlineCacheTarget::Value { method_id, .. }
            | MethodInlineCacheTarget::CallbackValue { method_id, .. },
            LinkedMethodDispatchKind::Value {
                method_id: dispatch_method,
            },
        ) => *method_id == *dispatch_method,
        (
            MethodInlineCacheTarget::Host { method_id },
            LinkedMethodDispatchKind::Host {
                method_id: dispatch_method,
            },
        ) => *method_id == *dispatch_method,
        _ => false,
    }
}

struct LinkedStandardValueMethodCall<'a> {
    dispatch: MethodDispatchHandle,
    debug_name: DebugNameId,
    receiver: Register,
    method_id: MethodId,
    standard_method: Option<crate::StandardMethodInlineCacheEntry>,
    values: &'a [Value],
}

fn linked_standard_value_method_result(
    context: &LinkedScriptMethodCallContext<'_>,
    frame: &CallFrame,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    call: LinkedStandardValueMethodCall<'_>,
) -> Option<VmResult<Value>> {
    let receiver = match frame.read(call.receiver) {
        Ok(receiver) => receiver,
        Err(error) => return Some(Err(error)),
    };
    if let Some(standard_method) = call.standard_method
        && script_builtin_methods::standard_cache_entry_matches_method_id(
            call.method_id,
            standard_method,
        )
        && let Some(result) = script_builtin_methods::call_standard_cached(
            receiver,
            standard_method,
            call.values,
            heap,
            budget,
        )
    {
        return Some(result);
    }
    let standard_method =
        script_builtin_methods::standard_cache_entry(call.method_id, receiver, heap.as_deref())?;
    let result = script_builtin_methods::call_standard_cached(
        receiver,
        standard_method,
        call.values,
        heap,
        budget,
    )?;
    if let Some(site) = context.cache_site
        && let Some(caches) = context.inline_caches
    {
        caches.set_method_dispatch(
            site,
            MethodInlineCacheEntry {
                dispatch: call.dispatch,
                debug_name: call.debug_name,
                target: MethodInlineCacheTarget::Value {
                    method_id: call.method_id,
                    standard_method: Some(standard_method),
                },
            },
        );
    }
    Some(result)
}

struct LinkedCallbackValueMethodCall<'a> {
    dispatch: MethodDispatchHandle,
    debug_name: DebugNameId,
    receiver: Register,
    method_id: MethodId,
    callback_method: Option<crate::CallbackMethodInlineCacheEntry>,
    values: &'a [Value],
}

fn linked_callback_value_method_result(
    vm: &Vm,
    context: &LinkedScriptMethodCallContext<'_>,
    host: &mut Option<&mut HostExecution<'_>>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    frame: &CallFrame,
    call: LinkedCallbackValueMethodCall<'_>,
) -> Option<VmResult<Value>> {
    let receiver = match frame.read(call.receiver) {
        Ok(receiver) => receiver,
        Err(error) => return Some(Err(error)),
    };
    let caller_roots = CallerRoots::for_frame(frame, heap.as_deref());
    let mut dispatch = callback_method_dispatch::CallbackMethodDispatch {
        vm,
        program: None,
        linked_program: Some(context.program),
        host: host.as_deref_mut(),
        heap: heap.as_deref_mut(),
        budget: budget.as_deref_mut(),
        caller_roots,
        inline_caches: context.inline_caches,
        bytecode_profiler: context.bytecode_profiler,
    };
    if let Some(callback_method) = call.callback_method
        && callback_method_dispatch::callback_cache_entry_matches_method_id(
            call.method_id,
            callback_method,
        )
        && let Some(result) = callback_method_dispatch::call_cached(
            receiver,
            callback_method,
            call.values,
            &mut dispatch,
        )
    {
        return Some(result);
    }
    let callback_method = callback_method_dispatch::callback_cache_entry(
        call.method_id,
        receiver,
        dispatch.heap_ref(),
    )?;
    let result = callback_method_dispatch::call_cached(
        receiver,
        callback_method,
        call.values,
        &mut dispatch,
    )
    .expect("resolved callback method cache entry should match receiver");
    if let Some(site) = context.cache_site
        && let Some(caches) = context.inline_caches
    {
        caches.set_method_dispatch(
            site,
            MethodInlineCacheEntry {
                dispatch: call.dispatch,
                debug_name: call.debug_name,
                target: MethodInlineCacheTarget::CallbackValue {
                    method_id: call.method_id,
                    callback_method,
                },
            },
        );
    }
    Some(result)
}

struct ScriptLinkedMethodCall<'a> {
    dst: Register,
    receiver: Register,
    debug_name: DebugNameId,
    function: vela_bytecode::ScriptFunctionHandle,
    values: &'a [Value],
}

fn dispatch_linked_script_method_call(
    vm: &Vm,
    context: LinkedScriptMethodCallContext<'_>,
    host: &mut Option<&mut HostExecution<'_>>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    frame: &mut CallFrame,
    call: ScriptLinkedMethodCall<'_>,
) -> VmResult<()> {
    let function_code = context.program.function(call.function).ok_or_else(|| {
        VmError::new(VmErrorKind::UnknownMethod {
            method: context.program.debug_name(call.debug_name).to_owned(),
        })
        .with_source_span_if_absent(context.call_site)
    })?;
    let receiver_value = *frame.read(call.receiver)?;
    let method_args =
        SmallStorage::try_from_prefix_and_slice_map(receiver_value, call.values, 4, |value| {
            Ok::<_, VmError>(*value)
        })?;
    let protected_root_len = heap.as_deref_mut().map(|heap| heap.push_frame_roots(frame));
    let result = vm.execute_linked_call(
        LinkedExecutionCall {
            code: function_code,
            program: context.program,
            captures: &[],
            args: method_args.as_slice(),
            check_param_guards: true,
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
