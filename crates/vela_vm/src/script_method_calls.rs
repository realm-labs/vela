use vela_bytecode::linked::{DynamicCallArgumentLinked, LinkedMethodDispatchKind};
use vela_bytecode::{
    CacheSiteId, CallArgument, DebugNameId, LinkedProgram, MethodDispatchHandle, Register,
    ScriptFunctionHandle,
};
use vela_common::Span;
use vela_def::MethodId;

use crate::dynamic_method_resolution::{self, DynamicMethodTarget};
use crate::heap::HeapValue;
use crate::method_runtime::CallerRoots;
use crate::{
    CallFrame, ExecutionBudget, HeapExecution, HostExecution, Value, Vm, VmResult,
    store_value_in_heap_if_needed,
};
use crate::{
    DynamicMethodInlineCacheEntry, DynamicMethodInlineCacheTarget, DynamicReceiverGuard,
    EqualityRuntime, MethodInlineCacheEntry, MethodInlineCacheTarget, StandardMethodReceiver,
    VmBytecodeProfiler, VmError, VmErrorKind, VmInlineCaches, array_methods,
    callback_method_dispatch, host_access, script_builtin_methods, script_function_calls,
};

use crate::script_methods::{ScriptMethodDispatch, call_method_id};

pub(crate) struct ScriptMethodIdCall<'a> {
    pub(crate) dst: Register,
    pub(crate) receiver: Register,
    pub(crate) method: &'a str,
    pub(crate) method_id: MethodId,
    pub(crate) values: &'a [Value],
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
    let mut receiver_value = frame.read(call.receiver)?;
    let caller_roots = CallerRoots::for_frame(frame, heap.as_deref());
    let result = call_method_id(
        &mut receiver_value,
        call.method,
        call.method_id,
        call.values,
        ScriptMethodDispatch {
            vm,
            program: context.program,
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
}

pub(crate) struct LinkedScriptMethodCall<'a> {
    pub(crate) dst: Register,
    pub(crate) receiver: Register,
    pub(crate) dispatch: MethodDispatchHandle,
    pub(crate) debug_name: DebugNameId,
    pub(crate) args: &'a [CallArgument],
}

pub(crate) struct LinkedDynamicMethodCall<'a> {
    pub(crate) dst: Register,
    pub(crate) receiver: Register,
    pub(crate) method_name: DebugNameId,
    pub(crate) args: &'a [DynamicCallArgumentLinked],
}

pub(crate) struct LinkedDynamicScriptTarget {
    pub(crate) function: vela_bytecode::ScriptFunctionHandle,
    pub(crate) args: Vec<CallArgument>,
}

pub(crate) enum LinkedDynamicResolution {
    Script(LinkedDynamicScriptTarget),
    Other(LinkedDynamicNonScriptTarget),
}

pub(crate) enum LinkedDynamicNonScriptTarget {
    Host {
        method_id: vela_common::HostMethodId,
    },
    StandardValue {
        method_id: MethodId,
        standard_method: Option<crate::StandardMethodInlineCacheEntry>,
    },
}

pub(crate) fn resolve_linked_dynamic_script_target(
    vm: &Vm,
    context: &LinkedScriptMethodCallContext<'_>,
    host: Option<&HostExecution<'_>>,
    heap: Option<&HeapExecution<'_>>,
    frame: &CallFrame,
    call: &LinkedDynamicMethodCall<'_>,
) -> VmResult<LinkedDynamicResolution> {
    let method = context.program.debug_name(call.method_name);
    let receiver = frame.read(call.receiver)?;
    let target = linked_dynamic_method_dispatch_target(
        vm,
        context,
        &receiver,
        method,
        call.method_name,
        heap,
        host,
    )?;
    let function = match target {
        DynamicMethodInlineCacheTarget::Script { function, .. } => function,
        DynamicMethodInlineCacheTarget::Host { method_id } => {
            return Ok(LinkedDynamicResolution::Other(
                LinkedDynamicNonScriptTarget::Host { method_id },
            ));
        }
        DynamicMethodInlineCacheTarget::StandardValue {
            method_id,
            standard_method,
        } => {
            return Ok(LinkedDynamicResolution::Other(
                LinkedDynamicNonScriptTarget::StandardValue {
                    method_id,
                    standard_method,
                },
            ));
        }
    };
    let args =
        dynamic_script_call_args_from_linked_arguments(context.program, function, call.args)?;
    Ok(LinkedDynamicResolution::Script(LinkedDynamicScriptTarget {
        function,
        args: args.as_slice().to_vec(),
    }))
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
            function: _,
        } => Err(VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
            opcode: "script method escaped the execution session",
        })),
        MethodInlineCacheTarget::Value {
            method_id,
            standard_method,
        } => {
            if let Some(result) = linked_standard_value_method_result(
                vm,
                &context,
                frame,
                host,
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn dispatch_resolved_linked_dynamic_method_call(
    vm: &Vm,
    context: LinkedScriptMethodCallContext<'_>,
    host: &mut Option<&mut HostExecution<'_>>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    frame: &mut CallFrame,
    call: LinkedDynamicMethodCall<'_>,
    target: LinkedDynamicNonScriptTarget,
) -> VmResult<()> {
    let method = context.program.debug_name(call.method_name);
    let receiver = frame.read(call.receiver)?;
    match target {
        LinkedDynamicNonScriptTarget::Host { method_id } => {
            let values_storage = dynamic_value_args_from_linked_arguments(frame, call.args)?;
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
                    args: values_storage.as_slice(),
                    wants_return: true,
                    cache_site: context.cache_site,
                },
            )?;
            if let Some(return_value) = return_value {
                frame.write(call.dst, return_value)?;
            }
            Ok(())
        }
        LinkedDynamicNonScriptTarget::StandardValue {
            method_id,
            standard_method,
        } => {
            let values_storage = dynamic_value_args_from_linked_arguments(frame, call.args)?;
            let contextual_result = {
                let caller_roots = CallerRoots::for_frame(frame, heap.as_deref());
                let mut runtime = EqualityRuntime {
                    vm,
                    program: context.program,
                    host: host.as_deref_mut(),
                    heap: heap.as_deref_mut(),
                    budget: budget.as_deref_mut(),
                    caller_roots,
                    inline_caches: context.inline_caches,
                    bytecode_profiler: context.bytecode_profiler,
                };
                contextual_array_standard_value_method(
                    &receiver,
                    method_id,
                    standard_method,
                    values_storage.as_slice(),
                    &mut runtime,
                )
            };
            if let Some(result) = contextual_result {
                return frame.write(
                    call.dst,
                    result.map_err(|error| error.with_source_span_if_absent(context.call_site))?,
                );
            }
            if let Some(standard_method) = standard_method.or_else(|| {
                script_builtin_methods::standard_cache_entry(method_id, &receiver, heap.as_deref())
            }) {
                let result = script_builtin_methods::call_standard_cached(
                    &receiver,
                    standard_method,
                    values_storage.as_slice(),
                    heap,
                    budget,
                )
                .ok_or_else(|| {
                    VmError::new(VmErrorKind::UnknownMethod {
                        method: method.to_owned(),
                    })
                })??;
                return frame.write(call.dst, result);
            }

            let caller_roots = CallerRoots::for_frame(frame, heap.as_deref());
            let mut dispatch = callback_method_dispatch::CallbackMethodDispatch {
                vm,
                program: context.program,
                host: host.as_deref_mut(),
                heap: heap.as_deref_mut(),
                budget: budget.as_deref_mut(),
                caller_roots,
                inline_caches: context.inline_caches,
                bytecode_profiler: context.bytecode_profiler,
            };
            let result = callback_method_dispatch::call_by_id(
                method_id,
                &receiver,
                values_storage.as_slice(),
                &mut dispatch,
            )
            .ok_or_else(|| {
                VmError::new(VmErrorKind::UnknownMethod {
                    method: method.to_owned(),
                })
            })??;
            frame.write(call.dst, result)
        }
    }
}

fn linked_dynamic_method_dispatch_target(
    vm: &Vm,
    context: &LinkedScriptMethodCallContext<'_>,
    receiver: &Value,
    method: &str,
    method_name: DebugNameId,
    heap: Option<&HeapExecution<'_>>,
    host: Option<&HostExecution<'_>>,
) -> VmResult<DynamicMethodInlineCacheTarget> {
    if let Some(site) = context.cache_site
        && let Some(entry) = context
            .inline_caches
            .and_then(|caches| caches.dynamic_method_dispatch(site))
        && entry.method_name == method_name
        && dynamic_receiver_guard_matches(&entry.receiver_guard, receiver, heap, host)
    {
        return Ok(entry.target);
    }

    let target = dynamic_method_resolution::resolve_linked_dynamic_method(
        receiver,
        method,
        context.program,
        heap,
        vm.type_registry(),
    )
    .ok_or_else(|| {
        VmError::new(VmErrorKind::UnknownMethod {
            method: method.to_owned(),
        })
    })?;
    let cache_target = dynamic_cache_target_from_resolved(target, receiver, heap);
    if let Some(site) = context.cache_site
        && let Some(caches) = context.inline_caches
        && let Some(receiver_guard) =
            dynamic_receiver_guard_for_target(&cache_target, receiver, heap, host)
    {
        caches.set_dynamic_method_dispatch(
            site,
            DynamicMethodInlineCacheEntry {
                method_name,
                receiver_guard,
                target: cache_target,
            },
        );
    }
    Ok(cache_target)
}

fn dynamic_cache_target_from_resolved(
    target: DynamicMethodTarget,
    receiver: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> DynamicMethodInlineCacheTarget {
    match target {
        DynamicMethodTarget::Script { dispatch, function } => {
            DynamicMethodInlineCacheTarget::Script { dispatch, function }
        }
        DynamicMethodTarget::Host { method_id } => {
            DynamicMethodInlineCacheTarget::Host { method_id }
        }
        DynamicMethodTarget::StandardValue { method_id } => {
            DynamicMethodInlineCacheTarget::StandardValue {
                method_id,
                standard_method: script_builtin_methods::standard_cache_entry(
                    method_id, receiver, heap,
                ),
            }
        }
    }
}

fn dynamic_receiver_guard_for_target(
    target: &DynamicMethodInlineCacheTarget,
    receiver: &Value,
    heap: Option<&HeapExecution<'_>>,
    host: Option<&HostExecution<'_>>,
) -> Option<DynamicReceiverGuard> {
    match target {
        DynamicMethodInlineCacheTarget::StandardValue { .. } => {
            standard_receiver_guard(receiver, heap)
        }
        DynamicMethodInlineCacheTarget::Script { .. } => script_receiver_guard(receiver, heap),
        DynamicMethodInlineCacheTarget::Host { .. } => host_receiver_guard(receiver, host),
    }
}

fn dynamic_receiver_guard_matches(
    guard: &DynamicReceiverGuard,
    receiver: &Value,
    heap: Option<&HeapExecution<'_>>,
    host: Option<&HostExecution<'_>>,
) -> bool {
    match guard {
        DynamicReceiverGuard::StdValue { receiver: expected } => {
            standard_receiver_guard(receiver, heap).is_some_and(|actual| {
                matches!(actual, DynamicReceiverGuard::StdValue { receiver } if receiver == *expected)
            })
        }
        DynamicReceiverGuard::ScriptType {
            type_name,
            shape_id,
        } => script_receiver_guard(receiver, heap).is_some_and(|actual| {
            matches!(
                actual,
                DynamicReceiverGuard::ScriptType {
                    type_name: actual_type,
                    shape_id: actual_shape,
                } if actual_type == *type_name && actual_shape == *shape_id
            )
        }),
        DynamicReceiverGuard::HostType {
            type_id,
            schema_epoch,
        } => host_receiver_guard(receiver, host).is_some_and(|actual| {
            matches!(
                actual,
                DynamicReceiverGuard::HostType {
                    type_id: actual_type,
                    schema_epoch: actual_epoch,
                } if actual_type == *type_id && actual_epoch == *schema_epoch
            )
        }),
    }
}

fn standard_receiver_guard(
    receiver: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> Option<DynamicReceiverGuard> {
    let receiver = match dynamic_method_resolution::classify_dynamic_receiver(receiver, heap, None)
    {
        dynamic_method_resolution::DynamicReceiverKind::String => StandardMethodReceiver::String,
        dynamic_method_resolution::DynamicReceiverKind::Bytes => StandardMethodReceiver::Bytes,
        dynamic_method_resolution::DynamicReceiverKind::Char => StandardMethodReceiver::Char,
        dynamic_method_resolution::DynamicReceiverKind::Array => StandardMethodReceiver::Array,
        dynamic_method_resolution::DynamicReceiverKind::Map => StandardMethodReceiver::Map,
        dynamic_method_resolution::DynamicReceiverKind::Set => StandardMethodReceiver::Set,
        dynamic_method_resolution::DynamicReceiverKind::Option => StandardMethodReceiver::Option,
        dynamic_method_resolution::DynamicReceiverKind::Result => StandardMethodReceiver::Result,
        dynamic_method_resolution::DynamicReceiverKind::Range => StandardMethodReceiver::Range,
        dynamic_method_resolution::DynamicReceiverKind::Iterator => {
            StandardMethodReceiver::Iterator
        }
        dynamic_method_resolution::DynamicReceiverKind::ScriptRecord { .. }
        | dynamic_method_resolution::DynamicReceiverKind::ScriptEnum { .. }
        | dynamic_method_resolution::DynamicReceiverKind::Host { .. }
        | dynamic_method_resolution::DynamicReceiverKind::Unsupported => return None,
    };
    Some(DynamicReceiverGuard::StdValue { receiver })
}

fn script_receiver_guard(
    receiver: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> Option<DynamicReceiverGuard> {
    let Value::HeapRef(reference) = receiver else {
        return None;
    };
    match heap.and_then(|heap| heap.heap.get(*reference)) {
        Some(HeapValue::Record {
            type_name,
            identity,
            fields,
        }) => Some(DynamicReceiverGuard::ScriptType {
            type_name: type_name.clone(),
            shape_id: identity.map_or_else(
                || Some(fields.shape_id()),
                |identity| Some(identity.shape_id),
            ),
        }),
        Some(HeapValue::Enum {
            enum_name,
            identity,
            fields,
            ..
        }) => Some(DynamicReceiverGuard::ScriptType {
            type_name: enum_name.clone(),
            shape_id: identity
                .map(|_| None)
                .unwrap_or_else(|| Some(fields.shape_id())),
        }),
        _ => None,
    }
}

fn host_receiver_guard(
    receiver: &Value,
    host: Option<&HostExecution<'_>>,
) -> Option<DynamicReceiverGuard> {
    let Value::HostRef(reference) = receiver else {
        return None;
    };
    let schema_epoch = host?.adapter.host_schema_epoch();
    Some(DynamicReceiverGuard::HostType {
        type_id: reference.type_id,
        schema_epoch,
    })
}

fn dynamic_script_call_args_from_linked_arguments(
    program: &LinkedProgram,
    function: ScriptFunctionHandle,
    args: &[DynamicCallArgumentLinked],
) -> VmResult<Vec<CallArgument>> {
    let code = program.function(function).ok_or_else(|| {
        VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
            opcode: "CallDynamicMethod",
        })
    })?;
    let params = code.params.get(1..).unwrap_or(&[]);
    let defaults = code.param_defaults.get(1..).unwrap_or(&[]);
    let mut slots = vec![None; params.len()];
    let mut next_positional = 0_usize;
    let mut seen_named = false;
    for arg in args {
        let index = if let Some(name) = arg.name {
            seen_named = true;
            let name = program.debug_name(name);
            params
                .iter()
                .position(|param| program.debug_name(*param) == name)
                .ok_or_else(|| {
                    VmError::new(VmErrorKind::TypeMismatch {
                        operation: "dynamic method unknown named argument",
                    })
                })?
        } else {
            if seen_named {
                return Err(VmError::new(VmErrorKind::TypeMismatch {
                    operation: "dynamic method positional argument after named argument",
                }));
            }
            let index = next_positional;
            next_positional = next_positional.saturating_add(1);
            if index >= params.len() {
                return Err(VmError::new(VmErrorKind::ArityMismatch {
                    name: program.debug_name(code.debug_name).to_owned(),
                    expected: params.len(),
                    actual: args.len(),
                }));
            }
            index
        };
        if slots[index].is_some() {
            return Err(VmError::new(VmErrorKind::TypeMismatch {
                operation: "dynamic method duplicate argument",
            }));
        }
        slots[index] = Some(arg.value);
    }
    slots
        .into_iter()
        .enumerate()
        .map(|(index, slot)| {
            if let Some(register) = slot {
                Ok(CallArgument::Register(register))
            } else if defaults.get(index).copied().unwrap_or(false) {
                Ok(CallArgument::Missing)
            } else {
                Err(VmError::new(VmErrorKind::ArityMismatch {
                    name: program.debug_name(code.debug_name).to_owned(),
                    expected: params.len(),
                    actual: args.len(),
                }))
            }
        })
        .collect()
}

fn dynamic_value_args_from_linked_arguments(
    frame: &CallFrame,
    args: &[DynamicCallArgumentLinked],
) -> VmResult<Vec<Value>> {
    let mut values = Vec::with_capacity(args.len());
    for arg in args {
        if arg.name.is_some() {
            return Err(VmError::new(VmErrorKind::TypeMismatch {
                operation: "dynamic method named arguments",
            }));
        }
        values.push(frame.read(arg.value)?);
    }
    Ok(values)
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
    vm: &Vm,
    context: &LinkedScriptMethodCallContext<'_>,
    frame: &CallFrame,
    host: &mut Option<&mut HostExecution<'_>>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    call: LinkedStandardValueMethodCall<'_>,
) -> Option<VmResult<Value>> {
    let receiver = match frame.read(call.receiver) {
        Ok(receiver) => receiver,
        Err(error) => return Some(Err(error)),
    };
    let resolved_standard_method = call.standard_method.or_else(|| {
        script_builtin_methods::standard_cache_entry(call.method_id, &receiver, heap.as_deref())
    });
    let contextual_result = {
        let caller_roots = CallerRoots::for_frame(frame, heap.as_deref());
        let mut runtime = EqualityRuntime {
            vm,
            program: context.program,
            host: host.as_deref_mut(),
            heap: heap.as_deref_mut(),
            budget: budget.as_deref_mut(),
            caller_roots,
            inline_caches: context.inline_caches,
            bytecode_profiler: context.bytecode_profiler,
        };
        contextual_array_standard_value_method(
            &receiver,
            call.method_id,
            resolved_standard_method,
            call.values,
            &mut runtime,
        )
    };
    if let Some(result) = contextual_result {
        if call.standard_method.is_none()
            && let Some(site) = context.cache_site
            && let Some(caches) = context.inline_caches
            && let Some(standard_method) = resolved_standard_method
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
        return Some(result.map_err(|error| error.with_source_span_if_absent(context.call_site)));
    }
    if let Some(standard_method) = call.standard_method
        && script_builtin_methods::standard_cache_entry_matches_method_id(
            call.method_id,
            standard_method,
        )
        && let Some(result) = script_builtin_methods::call_standard_cached(
            &receiver,
            standard_method,
            call.values,
            heap,
            budget,
        )
    {
        return Some(result);
    }
    let standard_method =
        script_builtin_methods::standard_cache_entry(call.method_id, &receiver, heap.as_deref())?;
    let result = script_builtin_methods::call_standard_cached(
        &receiver,
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

fn contextual_array_standard_value_method(
    receiver: &Value,
    method_id: MethodId,
    standard_method: Option<crate::StandardMethodInlineCacheEntry>,
    args: &[Value],
    runtime: &mut EqualityRuntime<'_, '_, '_>,
) -> Option<VmResult<Value>> {
    if !array_methods::is_array(receiver, runtime.heap.as_deref()) {
        return None;
    }
    let ids = crate::std_method_ids::std_method_ids();
    let target = standard_method.map(|method| method.target);
    let is_contains = method_id == ids.array_contains
        || matches!(
            target,
            Some(crate::StandardMethodInlineCacheTarget::Contains)
        );
    let is_index_of = method_id == ids.array_index_of
        || matches!(
            target,
            Some(crate::StandardMethodInlineCacheTarget::IndexOf)
        );
    let is_distinct = method_id == ids.array_distinct
        || matches!(
            target,
            Some(crate::StandardMethodInlineCacheTarget::Distinct)
        );
    let is_sort = method_id == ids.array_sort
        || matches!(target, Some(crate::StandardMethodInlineCacheTarget::Sort));
    let is_min = method_id == ids.array_min
        || matches!(target, Some(crate::StandardMethodInlineCacheTarget::Min));
    let is_max = method_id == ids.array_max
        || matches!(target, Some(crate::StandardMethodInlineCacheTarget::Max));
    if !(is_contains || is_index_of || is_distinct || is_sort || is_min || is_max) {
        return None;
    }
    if is_contains {
        return Some(
            array_methods::contains_by_key(receiver, args, runtime.heap.as_deref())
                .map(Value::Bool),
        );
    }
    if is_index_of {
        return Some(array_methods::index_of_by_key(
            receiver,
            args,
            &mut runtime.heap,
            &mut runtime.budget,
        ));
    }
    if is_distinct {
        return Some(array_methods::distinct_by_key(
            receiver,
            args,
            &mut runtime.heap,
            &mut runtime.budget,
        ));
    }
    if is_sort {
        return Some(array_methods::sort_with_ordering(receiver, args, runtime));
    }
    if is_min {
        return Some(array_methods::min_with_ordering(receiver, args, runtime));
    }
    Some(array_methods::max_with_ordering(receiver, args, runtime))
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
        program: context.program,
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
            &receiver,
            callback_method,
            call.values,
            &mut dispatch,
        )
    {
        return Some(result);
    }
    let callback_method = callback_method_dispatch::callback_cache_entry(
        call.method_id,
        &receiver,
        dispatch.heap_ref(),
    )?;
    let result = callback_method_dispatch::call_cached(
        &receiver,
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
