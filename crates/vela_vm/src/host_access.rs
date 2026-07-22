use vela_bytecode::linked::{DynamicCallArgumentLinked, LinkedMethodDispatchKind};
use vela_bytecode::{
    CacheSiteId, DebugNameId, HostTargetPlanId, LinkedProgram, MethodDispatchHandle, Register,
};
use vela_common::{HostMethodId, Span, StateSlot};
use vela_host::adapter::ExternStateBinding;
use vela_host::error::HostErrorKind;
use vela_host::path::HostPath;
use vela_host::protocol::{HostCollectionKey, HostCollectionKeyRef, HostCollectionQuery};
use vela_host::resolved::{HostAccessOp, HostAccessSpec, HostMutationOp, ResolvedHostAccess};
use vela_host::target::{HostPathArg, HostTargetInstance, HostTargetPlan};
use vela_host::value::HostValue;

use crate::heap::HeapValue;
use crate::heap_values::host_to_value;
use crate::host_values::{value_from_host, value_to_host};
use crate::{
    CallFrame, ExecutionBudget, HeapExecution, HostExecution, HostInlineCacheEntry,
    HostInlineCacheTarget, OwnedValue, Value, VmError, VmErrorKind, VmInlineCaches, VmResult,
    expect_host_ref, value_to_owned,
};

pub(crate) struct HostAccessRuntime<'a, 'host, 'heap> {
    pub(crate) frame: &'a CallFrame,
    pub(crate) heap: Option<&'a mut HeapExecution<'heap>>,
    pub(crate) budget: Option<&'a mut ExecutionBudget>,
    pub(crate) host: Option<&'a mut HostExecution<'host>>,
    pub(crate) inline_caches: Option<&'a dyn VmInlineCaches>,
    pub(crate) source_span: Option<Span>,
}

pub(crate) struct CodeHostTargetPlan<'a> {
    pub(crate) targets: &'a [HostTargetPlan],
    pub(crate) target_id: HostTargetPlanId,
    pub(crate) dynamic_args: &'a [Register],
    pub(crate) cache_site: CacheSiteId,
}

pub(crate) fn load_linked_cached_extern_state(
    runtime: HostAccessRuntime<'_, '_, '_>,
    program: &LinkedProgram,
    debug_name: DebugNameId,
    declared_slot: Option<StateSlot>,
) -> VmResult<Value> {
    let slot = declared_slot.ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "extern state slot",
        })
    })?;
    let state = program.state(slot).ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "extern state slot",
        })
    })?;
    let name = program.debug_name(debug_name);
    let host = runtime.host.ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "host context",
        })
    })?;
    let root = host
        .adapter
        .extern_state_ref(ExternStateBinding { id: state.id, name })
        .map_err(|error| error.with_source_span_if_absent(runtime.source_span))?;
    Ok(Value::HostRef(root))
}

pub(crate) fn load_linked_state(
    runtime: HostAccessRuntime<'_, '_, '_>,
    program: &LinkedProgram,
    slot: StateSlot,
) -> VmResult<Value> {
    let state = program.state(slot).ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "VM state slot",
        })
    })?;
    let value = runtime
        .host
        .and_then(|host| host.state_values.as_deref())
        .and_then(|states| states.get(state.id))
        .ok_or_else(|| {
            VmError::new(VmErrorKind::TypeMismatch {
                operation: "uninitialized VM state",
            })
        })?;
    Ok(value)
}

pub(crate) fn store_linked_state(
    runtime: HostAccessRuntime<'_, '_, '_>,
    program: &LinkedProgram,
    slot: StateSlot,
    value: Value,
) -> VmResult<()> {
    let state = program.state(slot).ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "VM state slot",
        })
    })?;
    let states = runtime
        .host
        .and_then(|host| host.state_values.as_deref_mut())
        .ok_or_else(|| {
            VmError::new(VmErrorKind::TypeMismatch {
                operation: "VM state context",
            })
        })?;
    states.insert(state.id, value);
    Ok(())
}

pub(crate) fn execute_host_read(
    runtime: HostAccessRuntime<'_, '_, '_>,
    root: Register,
    target_id: HostTargetPlanId,
    target: &HostTargetPlan,
    dynamic_args: &[Register],
    cache_site: CacheSiteId,
) -> VmResult<Value> {
    let root = expect_host_ref(&runtime.frame.read(root)?, "host_read")?;
    let args = materialize_host_args(
        runtime.frame,
        dynamic_args,
        runtime.heap.as_deref(),
        "host_read",
    )?;
    let instance = HostTargetInstance::new(root, target, args.as_slice());
    let host = runtime.host.ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "host context",
        })
    })?;
    let cached_access = resolve_cached_access(
        host.adapter,
        runtime.inline_caches,
        cache_site,
        HostInlineCacheTarget::TargetPlan(target_id),
        instance,
        HostAccessOp::Read,
        runtime.source_span,
    )?;
    let value =
        host.access
            .read_resolved(host.adapter, cached_access, instance, runtime.source_span)?;
    runtime_value_from_host(value, runtime.heap, runtime.budget)
}

pub(crate) fn execute_code_host_read(
    runtime: HostAccessRuntime<'_, '_, '_>,
    root: Register,
    target: CodeHostTargetPlan<'_>,
) -> VmResult<Value> {
    let plan = code_host_target(target.targets, target.target_id, runtime.source_span)?;
    execute_host_read(
        runtime,
        root,
        target.target_id,
        plan,
        target.dynamic_args,
        target.cache_site,
    )
}

pub(crate) fn execute_host_write(
    runtime: HostAccessRuntime<'_, '_, '_>,
    root: Register,
    target_id: HostTargetPlanId,
    target: &HostTargetPlan,
    dynamic_args: &[Register],
    src: Register,
    cache_site: CacheSiteId,
) -> VmResult<()> {
    let root = expect_host_ref(&runtime.frame.read(root)?, "host_write")?;
    let value = value_to_host(
        &runtime.frame.read(src)?,
        "set_host_field",
        runtime.heap.as_deref(),
    )?;
    let args = materialize_host_args(
        runtime.frame,
        dynamic_args,
        runtime.heap.as_deref(),
        "host_write",
    )?;
    let instance = HostTargetInstance::new(root, target, args.as_slice());
    let host = runtime.host.ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "host context",
        })
    })?;
    let cached_access = resolve_cached_access(
        host.adapter,
        runtime.inline_caches,
        cache_site,
        HostInlineCacheTarget::TargetPlan(target_id),
        instance,
        HostAccessOp::Write,
        runtime.source_span,
    )?;
    host.access.write_resolved(
        host.adapter,
        cached_access,
        instance,
        value,
        runtime.source_span,
    )?;
    Ok(())
}

pub(crate) fn execute_code_host_write(
    runtime: HostAccessRuntime<'_, '_, '_>,
    root: Register,
    target: CodeHostTargetPlan<'_>,
    src: Register,
) -> VmResult<()> {
    let plan = code_host_target(target.targets, target.target_id, runtime.source_span)?;
    execute_host_write(
        runtime,
        root,
        target.target_id,
        plan,
        target.dynamic_args,
        src,
        target.cache_site,
    )
}

pub(crate) fn execute_host_mutate(
    runtime: HostAccessRuntime<'_, '_, '_>,
    root: Register,
    mutation: HostMutationPlan<'_>,
) -> VmResult<()> {
    let root = expect_host_ref(&runtime.frame.read(root)?, "host_mutate")?;
    let value = value_to_host(
        &runtime.frame.read(mutation.rhs)?,
        "host_mutate",
        runtime.heap.as_deref(),
    )?;
    let args = materialize_host_args(
        runtime.frame,
        mutation.dynamic_args,
        runtime.heap.as_deref(),
        "host_mutate",
    )?;
    let instance = HostTargetInstance::new(root, mutation.target, args.as_slice());
    let host = runtime.host.ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "host context",
        })
    })?;
    let cached_access = resolve_cached_access(
        host.adapter,
        runtime.inline_caches,
        mutation.cache_site,
        HostInlineCacheTarget::TargetPlan(mutation.target_id),
        instance,
        HostAccessOp::Mutate(mutation.op),
        runtime.source_span,
    )?;
    host.access.mutate_resolved(
        host.adapter,
        cached_access,
        instance,
        mutation.op,
        value,
        runtime.source_span,
    )?;
    Ok(())
}

pub(crate) struct HostMutationPlan<'a> {
    pub(crate) target_id: HostTargetPlanId,
    pub(crate) target: &'a HostTargetPlan,
    pub(crate) dynamic_args: &'a [Register],
    pub(crate) op: HostMutationOp,
    pub(crate) rhs: Register,
    pub(crate) cache_site: CacheSiteId,
}

pub(crate) struct CodeHostMutationPlan<'a> {
    pub(crate) target: CodeHostTargetPlan<'a>,
    pub(crate) op: HostMutationOp,
    pub(crate) rhs: Register,
}

pub(crate) fn execute_code_host_mutate(
    runtime: HostAccessRuntime<'_, '_, '_>,
    root: Register,
    mutation: CodeHostMutationPlan<'_>,
) -> VmResult<()> {
    let plan = code_host_target(
        mutation.target.targets,
        mutation.target.target_id,
        runtime.source_span,
    )?;
    execute_host_mutate(
        runtime,
        root,
        HostMutationPlan {
            target_id: mutation.target.target_id,
            target: plan,
            dynamic_args: mutation.target.dynamic_args,
            op: mutation.op,
            rhs: mutation.rhs,
            cache_site: mutation.target.cache_site,
        },
    )
}

pub(crate) fn execute_host_remove(
    runtime: HostAccessRuntime<'_, '_, '_>,
    root: Register,
    target_id: HostTargetPlanId,
    target: &HostTargetPlan,
    dynamic_args: &[Register],
    cache_site: CacheSiteId,
) -> VmResult<()> {
    let root = expect_host_ref(&runtime.frame.read(root)?, "host_remove")?;
    let args = materialize_host_args(
        runtime.frame,
        dynamic_args,
        runtime.heap.as_deref(),
        "host_remove",
    )?;
    let instance = HostTargetInstance::new(root, target, args.as_slice());
    let host = runtime.host.ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "host context",
        })
    })?;
    let cached_access = resolve_cached_access(
        host.adapter,
        runtime.inline_caches,
        cache_site,
        HostInlineCacheTarget::TargetPlan(target_id),
        instance,
        HostAccessOp::Remove,
        runtime.source_span,
    )?;
    host.access
        .remove_resolved(host.adapter, cached_access, instance, runtime.source_span)?;
    Ok(())
}

pub(crate) fn execute_code_host_remove(
    runtime: HostAccessRuntime<'_, '_, '_>,
    root: Register,
    target: CodeHostTargetPlan<'_>,
) -> VmResult<()> {
    let plan = code_host_target(target.targets, target.target_id, runtime.source_span)?;
    execute_host_remove(
        runtime,
        root,
        target.target_id,
        plan,
        target.dynamic_args,
        target.cache_site,
    )
}

pub(crate) struct HostCallPlan<'a> {
    pub(crate) target_id: HostTargetPlanId,
    pub(crate) target: &'a HostTargetPlan,
    pub(crate) dynamic_args: &'a [Register],
    pub(crate) method: HostMethodId,
    pub(crate) args: &'a [Register],
    pub(crate) wants_return: bool,
    pub(crate) cache_site: CacheSiteId,
}

pub(crate) struct CodeHostCallPlan<'a> {
    pub(crate) target: CodeHostTargetPlan<'a>,
    pub(crate) method: HostMethodId,
    pub(crate) args: &'a [Register],
    pub(crate) wants_return: bool,
}

pub(crate) struct LinkedCodeHostCallPlan<'a> {
    pub(crate) program: &'a LinkedProgram,
    pub(crate) target: CodeHostTargetPlan<'a>,
    pub(crate) method: MethodDispatchHandle,
    pub(crate) args: &'a [Register],
    pub(crate) wants_return: bool,
}

pub(crate) struct PreparedAsyncHostMethodArgs {
    pub(crate) receiver: HostPath,
    pub(crate) args: Vec<OwnedValue>,
}

pub(crate) struct HostRootMethodCall<'a> {
    pub(crate) method: HostMethodId,
    pub(crate) args: &'a [Value],
    pub(crate) wants_return: bool,
    pub(crate) cache_site: Option<CacheSiteId>,
}

pub(crate) fn execute_host_call(
    runtime: HostAccessRuntime<'_, '_, '_>,
    root: Register,
    call: HostCallPlan<'_>,
) -> VmResult<Option<Value>> {
    let root = expect_host_ref(&runtime.frame.read(root)?, "host_call")?;
    let dynamic_args = materialize_host_args(
        runtime.frame,
        call.dynamic_args,
        runtime.heap.as_deref(),
        "host_call",
    )?;
    let values = call
        .args
        .iter()
        .map(|register| {
            value_to_host(
                &runtime.frame.read(*register)?,
                "host_call",
                runtime.heap.as_deref(),
            )
        })
        .collect::<VmResult<Vec<_>>>()?;
    let instance = HostTargetInstance::new(root, call.target, dynamic_args.as_slice());
    let host = runtime.host.ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "host context",
        })
    })?;
    let cached_access = resolve_cached_access(
        host.adapter,
        runtime.inline_caches,
        call.cache_site,
        HostInlineCacheTarget::TargetPlan(call.target_id),
        instance,
        HostAccessOp::Call(call.method),
        runtime.source_span,
    )?;
    let value = host.access.call_resolved(
        host.adapter,
        cached_access,
        instance,
        call.method,
        &values,
        runtime.source_span,
    )?;
    if call.wants_return {
        runtime_value_from_host(value, runtime.heap, runtime.budget).map(Some)
    } else {
        Ok(None)
    }
}

pub(crate) fn execute_code_host_call(
    runtime: HostAccessRuntime<'_, '_, '_>,
    root: Register,
    call: CodeHostCallPlan<'_>,
) -> VmResult<Option<Value>> {
    let plan = code_host_target(
        call.target.targets,
        call.target.target_id,
        runtime.source_span,
    )?;
    execute_host_call(
        runtime,
        root,
        HostCallPlan {
            target_id: call.target.target_id,
            target: plan,
            dynamic_args: call.target.dynamic_args,
            method: call.method,
            args: call.args,
            wants_return: call.wants_return,
            cache_site: call.target.cache_site,
        },
    )
}

pub(crate) fn execute_linked_code_host_call(
    runtime: HostAccessRuntime<'_, '_, '_>,
    root: Register,
    call: LinkedCodeHostCallPlan<'_>,
) -> VmResult<Option<Value>> {
    let method_id = linked_host_method_id(call.program, call.method, runtime.source_span)?;
    execute_code_host_call(
        runtime,
        root,
        CodeHostCallPlan {
            target: call.target,
            method: method_id,
            args: call.args,
            wants_return: call.wants_return,
        },
    )
}

pub(crate) fn linked_host_method_id(
    program: &LinkedProgram,
    method: MethodDispatchHandle,
    source_span: Option<Span>,
) -> VmResult<HostMethodId> {
    Ok(match program.method_dispatch(method).map(|d| &d.kind) {
        Some(LinkedMethodDispatchKind::Host { method_id }) => *method_id,
        _ => {
            return Err(VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
                opcode: "HostCall",
            })
            .with_source_span_if_absent(source_span));
        }
    })
}

pub(crate) fn prepare_async_host_method_args(
    frame: &CallFrame,
    heap: Option<&HeapExecution<'_>>,
    root: Register,
    target: CodeHostTargetPlan<'_>,
    args: &[Register],
    source_span: Option<Span>,
) -> VmResult<PreparedAsyncHostMethodArgs> {
    let root = expect_host_ref(&frame.read(root)?, "host_call")?;
    let plan = code_host_target(target.targets, target.target_id, source_span)?;
    let dynamic_args = materialize_host_args(frame, target.dynamic_args, heap, "host_call")?;
    let receiver = HostTargetInstance::new(root, plan, dynamic_args.as_slice())
        .to_diagnostic_path()
        .to_host_path();
    let args = args
        .iter()
        .map(|register| value_to_owned(&frame.read(*register)?, heap))
        .collect::<VmResult<Vec<_>>>()?;
    Ok(PreparedAsyncHostMethodArgs { receiver, args })
}

pub(crate) fn prepare_async_host_root_method_args(
    frame: &CallFrame,
    heap: Option<&HeapExecution<'_>>,
    receiver: Register,
    args: &[DynamicCallArgumentLinked],
) -> VmResult<PreparedAsyncHostMethodArgs> {
    let root = expect_host_ref(&frame.read(receiver)?, "host_call")?;
    let args = crate::script_method_calls::dynamic_value_args_from_linked_arguments(frame, args)?
        .iter()
        .map(|value| value_to_owned(value, heap))
        .collect::<VmResult<Vec<_>>>()?;
    Ok(PreparedAsyncHostMethodArgs {
        receiver: HostPath::new(root),
        args,
    })
}

pub(crate) fn execute_host_root_method_call(
    runtime: HostAccessRuntime<'_, '_, '_>,
    receiver: Register,
    call: HostRootMethodCall<'_>,
) -> VmResult<Option<Value>> {
    let root = expect_host_ref(&runtime.frame.read(receiver)?, "host_call")?;
    let values = call
        .args
        .iter()
        .map(|value| value_to_host(value, "host_call", runtime.heap.as_deref()))
        .collect::<VmResult<Vec<_>>>()?;
    let target = HostTargetPlan::new(root.type_id);
    let dynamic_args = [];
    let instance = HostTargetInstance::new(root, &target, &dynamic_args);
    let host = runtime.host.ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "host context",
        })
    })?;
    let op = HostAccessOp::Call(call.method);
    let resolved = if let Some(cache_site) = call.cache_site {
        resolve_cached_access(
            host.adapter,
            runtime.inline_caches,
            cache_site,
            HostInlineCacheTarget::RootObject,
            instance,
            op,
            runtime.source_span,
        )?
    } else {
        host.adapter
            .resolve_host_access(HostAccessSpec::new(op, &target))
            .map_err(|error| error.with_source_span_if_absent(runtime.source_span))?
    };
    let value = host.access.call_resolved(
        host.adapter,
        resolved,
        instance,
        call.method,
        &values,
        runtime.source_span,
    )?;
    if call.wants_return {
        runtime_value_from_host(value, runtime.heap, runtime.budget).map(Some)
    } else {
        Ok(None)
    }
}

pub(crate) fn execute_host_root_collection_query(
    runtime: HostAccessRuntime<'_, '_, '_>,
    receiver: Register,
    query: HostCollectionQuery,
    cache_site: Option<CacheSiteId>,
) -> VmResult<Value> {
    let root = expect_host_ref(&runtime.frame.read(receiver)?, "host collection query")?;
    let target = HostTargetPlan::new(root.type_id);
    let instance = HostTargetInstance::new(root, &target, &[]);
    let host = runtime.host.ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "host context",
        })
    })?;
    let resolved = if let Some(cache_site) = cache_site {
        resolve_cached_access(
            host.adapter,
            runtime.inline_caches,
            cache_site,
            HostInlineCacheTarget::RootObject,
            instance,
            HostAccessOp::Read,
            runtime.source_span,
        )?
    } else {
        host.adapter
            .resolve_host_access(HostAccessSpec::new(HostAccessOp::Read, &target))
            .map_err(|error| error.with_source_span_if_absent(runtime.source_span))?
    };
    let value = host.access.query_collection_resolved(
        host.adapter,
        resolved,
        instance,
        query,
        runtime.source_span,
    )?;
    runtime_value_from_host(value, runtime.heap, runtime.budget)
}

pub(crate) fn execute_host_root_collection_lookup(
    mut runtime: HostAccessRuntime<'_, '_, '_>,
    receiver: Register,
    lookup: crate::std_method_ids::HostCollectionLookup,
    args: &[Value],
    cache_site: Option<CacheSiteId>,
) -> VmResult<Value> {
    if args.len() != lookup.arity() {
        return Err(VmError::new(VmErrorKind::ArityMismatch {
            name: lookup.name().to_owned(),
            expected: lookup.arity(),
            actual: args.len(),
        }));
    }
    let root = expect_host_ref(&runtime.frame.read(receiver)?, "host collection lookup")?;
    let key =
        runtime_collection_index(&args[0], runtime.heap.as_deref(), "host collection lookup")?;
    let (target, arg) = key.target(root.type_id);
    let target_args = [arg];
    let instance = HostTargetInstance::new(root, &target, &target_args);
    let host = runtime
        .host
        .as_deref_mut()
        .ok_or_else(missing_host_context)?;
    let resolved = if let Some(cache_site) = cache_site {
        resolve_cached_access(
            host.adapter,
            runtime.inline_caches,
            cache_site,
            HostInlineCacheTarget::CollectionKey,
            instance,
            HostAccessOp::Read,
            runtime.source_span,
        )?
    } else {
        host.adapter
            .resolve_host_access(HostAccessSpec::new(HostAccessOp::Read, &target))
            .map_err(|error| error.with_source_span_if_absent(runtime.source_span))?
    };
    let payload =
        match host
            .access
            .read_resolved(host.adapter, resolved, instance, runtime.source_span)
        {
            Ok(value) => Some(value),
            Err(error) if matches!(&error.kind, HostErrorKind::MissingCollectionEntry { .. }) => {
                None
            }
            Err(error) => return Err(error.into()),
        };

    use crate::std_method_ids::HostCollectionLookup;
    match lookup {
        HostCollectionLookup::MapHas => Ok(Value::Bool(payload.is_some())),
        HostCollectionLookup::SetHas => match payload {
            Some(HostValue::Bool(value)) => Ok(Value::Bool(value)),
            None => Ok(Value::Bool(false)),
            Some(_) => Err(VmError::new(VmErrorKind::TypeMismatch {
                operation: "host set has",
            })),
        },
        HostCollectionLookup::MapGet => {
            let payload = payload
                .map(|payload| {
                    runtime_value_from_host(
                        payload,
                        runtime.heap.as_deref_mut(),
                        runtime.budget.as_deref_mut(),
                    )
                })
                .transpose()?;
            let heap = runtime.heap.as_deref_mut().ok_or_else(|| {
                VmError::new(VmErrorKind::TypeMismatch {
                    operation: "host map get",
                })
            })?;
            crate::option_result::option_value(payload, heap, runtime.budget.as_deref_mut())
        }
        HostCollectionLookup::MapGetOr => match payload {
            Some(payload) => runtime_value_from_host(payload, runtime.heap, runtime.budget),
            None => Ok(args[1]),
        },
    }
}

pub(crate) fn execute_host_root_collection_mutation(
    mut runtime: HostAccessRuntime<'_, '_, '_>,
    receiver: Register,
    mutation: crate::std_method_ids::HostCollectionMutation,
    args: &[Value],
    cache_site: Option<CacheSiteId>,
) -> VmResult<Value> {
    if args.len() != mutation.arity() {
        return Err(VmError::new(VmErrorKind::ArityMismatch {
            name: mutation.name().to_owned(),
            expected: mutation.arity(),
            actual: args.len(),
        }));
    }
    let root = expect_host_ref(&runtime.frame.read(receiver)?, "host collection mutation")?;
    let key = runtime_collection_index(
        &args[0],
        runtime.heap.as_deref(),
        "host collection mutation",
    )?;
    let (target, arg) = key.target(root.type_id);
    let target_args = [arg];
    let instance = HostTargetInstance::new(root, &target, &target_args);
    let map_value = matches!(
        mutation,
        crate::std_method_ids::HostCollectionMutation::MapSet
    )
    .then(|| value_to_host(&args[1], "host map set", runtime.heap.as_deref()))
    .transpose()?;
    let host = runtime
        .host
        .as_deref_mut()
        .ok_or_else(missing_host_context)?;

    use crate::std_method_ids::HostCollectionMutation;
    match mutation {
        HostCollectionMutation::MapSet => {
            let resolved = resolve_collection_key_access(
                host,
                runtime.inline_caches,
                cache_site,
                instance,
                HostAccessOp::Write,
                runtime.source_span,
            )?;
            host.access.write_resolved(
                host.adapter,
                resolved,
                instance,
                map_value.expect("MapSet prepared a value"),
                runtime.source_span,
            )?;
            Ok(args[1])
        }
        HostCollectionMutation::SetAdd | HostCollectionMutation::SetRemove => {
            let read = resolve_collection_key_access(
                host,
                runtime.inline_caches,
                cache_site,
                instance,
                HostAccessOp::Read,
                runtime.source_span,
            )?;
            let current =
                host.access
                    .read_resolved(host.adapter, read, instance, runtime.source_span)?;
            let HostValue::Bool(current) = current else {
                return Err(VmError::new(VmErrorKind::TypeMismatch {
                    operation: "host set mutation",
                }));
            };
            let desired = matches!(mutation, HostCollectionMutation::SetAdd);
            let changed = current != desired;
            if changed {
                let write = resolve_collection_key_access(
                    host,
                    runtime.inline_caches,
                    cache_site,
                    instance,
                    HostAccessOp::Write,
                    runtime.source_span,
                )?;
                host.access.write_resolved(
                    host.adapter,
                    write,
                    instance,
                    HostValue::Bool(desired),
                    runtime.source_span,
                )?;
            }
            Ok(Value::Bool(changed))
        }
    }
}

fn resolve_collection_key_access(
    host: &HostExecution<'_>,
    inline_caches: Option<&dyn VmInlineCaches>,
    cache_site: Option<CacheSiteId>,
    target: HostTargetInstance<'_>,
    op: HostAccessOp,
    source_span: Option<Span>,
) -> VmResult<ResolvedHostAccess> {
    if let Some(cache_site) = cache_site {
        resolve_cached_access(
            host.adapter,
            inline_caches,
            cache_site,
            HostInlineCacheTarget::CollectionKey,
            target,
            op,
            source_span,
        )
    } else {
        host.adapter
            .resolve_host_access(HostAccessSpec::new(op, target.plan))
            .map_err(|error| error.with_source_span_if_absent(source_span).into())
    }
}

pub(crate) fn execute_host_collection_index_read(
    mut runtime: HostAccessRuntime<'_, '_, '_>,
    receiver: Register,
    index: Register,
) -> VmResult<Value> {
    let root = expect_host_ref(&runtime.frame.read(receiver)?, "host collection index")?;
    let index = runtime_collection_index(
        &runtime.frame.read(index)?,
        runtime.heap.as_deref(),
        "host collection index",
    )?;
    let (target, arg) = index.target(root.type_id);
    let args = [arg];
    let instance = HostTargetInstance::new(root, &target, &args);
    let host = runtime.host.ok_or_else(missing_host_context)?;
    let access = host
        .adapter
        .resolve_host_access(HostAccessSpec::new(HostAccessOp::Read, &target))
        .map_err(|error| error.with_source_span_if_absent(runtime.source_span))?;
    let value = host
        .access
        .read_resolved(host.adapter, access, instance, runtime.source_span)?;
    runtime_value_from_host(value, runtime.heap.take(), runtime.budget.take())
}

pub(crate) fn execute_host_collection_string_key_read(
    mut runtime: HostAccessRuntime<'_, '_, '_>,
    receiver: Register,
    key: &str,
) -> VmResult<Value> {
    let root = expect_host_ref(&runtime.frame.read(receiver)?, "host collection index")?;
    let target = HostTargetPlan::new(root.type_id).const_key(key);
    let instance = HostTargetInstance::new(root, &target, &[]);
    let host = runtime.host.ok_or_else(missing_host_context)?;
    let access = host
        .adapter
        .resolve_host_access(HostAccessSpec::new(HostAccessOp::Read, &target))
        .map_err(|error| error.with_source_span_if_absent(runtime.source_span))?;
    let value = host
        .access
        .read_resolved(host.adapter, access, instance, runtime.source_span)?;
    runtime_value_from_host(value, runtime.heap.take(), runtime.budget.take())
}

pub(crate) fn execute_host_collection_index_write(
    runtime: HostAccessRuntime<'_, '_, '_>,
    receiver: Register,
    index: Register,
    src: Register,
) -> VmResult<()> {
    let root = expect_host_ref(
        &runtime.frame.read(receiver)?,
        "host collection index assignment",
    )?;
    let index = runtime_collection_index(
        &runtime.frame.read(index)?,
        runtime.heap.as_deref(),
        "host collection index assignment",
    )?;
    let value = value_to_host(
        &runtime.frame.read(src)?,
        "host collection index assignment",
        runtime.heap.as_deref(),
    )?;
    let (target, arg) = index.target(root.type_id);
    let args = [arg];
    execute_host_collection_index_write_target(runtime, root, &target, &args, value)
}

pub(crate) fn execute_host_collection_string_key_write(
    runtime: HostAccessRuntime<'_, '_, '_>,
    receiver: Register,
    key: &str,
    src: Register,
) -> VmResult<()> {
    let root = expect_host_ref(
        &runtime.frame.read(receiver)?,
        "host collection index assignment",
    )?;
    let value = value_to_host(
        &runtime.frame.read(src)?,
        "host collection index assignment",
        runtime.heap.as_deref(),
    )?;
    let target = HostTargetPlan::new(root.type_id).const_key(key);
    execute_host_collection_index_write_target(runtime, root, &target, &[], value)
}

fn execute_host_collection_index_write_target(
    runtime: HostAccessRuntime<'_, '_, '_>,
    root: vela_host::path::HostRef,
    target: &HostTargetPlan,
    args: &[HostPathArg<'_>],
    value: HostValue,
) -> VmResult<()> {
    let instance = HostTargetInstance::new(root, target, args);
    let host = runtime.host.ok_or_else(missing_host_context)?;
    let access = host
        .adapter
        .resolve_host_access(HostAccessSpec::new(HostAccessOp::Write, target))
        .map_err(|error| error.with_source_span_if_absent(runtime.source_span))?;
    host.access
        .write_resolved(host.adapter, access, instance, value, runtime.source_span)?;
    Ok(())
}

struct RuntimeCollectionIndex(HostCollectionKey);

impl RuntimeCollectionIndex {
    fn target(&self, root_type: vela_common::HostTypeId) -> (HostTargetPlan, HostPathArg<'_>) {
        (
            HostTargetPlan::new(root_type).dyn_key(0),
            HostPathArg::Key(self.0.as_ref()),
        )
    }
}

fn runtime_collection_index(
    index: &Value,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<RuntimeCollectionIndex> {
    let key = match index {
        Value::Bool(value) => HostCollectionKey::Bool(*value),
        Value::Char(value) => HostCollectionKey::Char(*value),
        Value::I8(value) => HostCollectionKey::I8(*value),
        Value::I16(value) => HostCollectionKey::I16(*value),
        Value::I32(value) => HostCollectionKey::I32(*value),
        Value::I64(value) => HostCollectionKey::I64(*value),
        Value::U8(value) => HostCollectionKey::U8(*value),
        Value::U16(value) => HostCollectionKey::U16(*value),
        Value::U32(value) => HostCollectionKey::U32(*value),
        Value::U64(value) => HostCollectionKey::U64(*value),
        Value::HostRef(value) => HostCollectionKey::HostRef(*value),
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::String(key)) => HostCollectionKey::String(key.clone()),
            Some(HeapValue::Bytes(key)) => HostCollectionKey::Bytes(key.clone()),
            _ => return Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
        },
        Value::Missing | Value::Unit | Value::F32(_) | Value::F64(_) | Value::Range(_) => {
            return Err(VmError::new(VmErrorKind::TypeMismatch { operation }));
        }
    };
    Ok(RuntimeCollectionIndex(key))
}

fn missing_host_context() -> VmError {
    VmError::new(VmErrorKind::TypeMismatch {
        operation: "host context",
    })
}

fn resolve_cached_access(
    adapter: &dyn vela_host::adapter::ScriptStateAdapter,
    inline_caches: Option<&dyn VmInlineCaches>,
    cache_site: CacheSiteId,
    target_key: HostInlineCacheTarget,
    target: HostTargetInstance<'_>,
    op: HostAccessOp,
    source_span: Option<Span>,
) -> VmResult<ResolvedHostAccess> {
    let schema_epoch = adapter.host_schema_epoch();
    if let Some(cache) = inline_caches
        && let Some(entry) = cache.host_access(cache_site)
        && entry.root_type == target.root.type_id
        && entry.target == target_key
        && entry.op == op
        && entry.schema_epoch == schema_epoch
    {
        return Ok(entry.resolved);
    }
    let resolved = adapter
        .resolve_host_access(HostAccessSpec::new(op, target.plan))
        .map_err(|error| error.with_source_span_if_absent(source_span))?;
    if let Some(cache) = inline_caches {
        cache.set_host_access(
            cache_site,
            HostInlineCacheEntry {
                root_type: target.root.type_id,
                target: target_key,
                op,
                schema_epoch: resolved.schema_epoch,
                resolved,
            },
        );
    }
    Ok(resolved)
}

pub(crate) fn code_host_target(
    targets: &[HostTargetPlan],
    id: HostTargetPlanId,
    source_span: Option<Span>,
) -> VmResult<&HostTargetPlan> {
    targets.get(id.index()).ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "host target",
        })
        .with_source_span(source_span)
    })
}

fn runtime_value_from_host(
    value: HostValue,
    heap: Option<&mut HeapExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    if let Some(heap) = heap {
        host_to_value(value, heap, budget)
    } else {
        Ok(value_from_host(value))
    }
}

enum MaterializedHostArgs<'a> {
    Empty,
    Values(Vec<HostPathArg<'a>>),
}

impl<'a> MaterializedHostArgs<'a> {
    fn as_slice(&'a self) -> &'a [HostPathArg<'a>] {
        match self {
            Self::Empty => &[],
            Self::Values(args) => args,
        }
    }
}

fn materialize_host_args<'a>(
    frame: &CallFrame,
    registers: &[Register],
    heap: Option<&'a HeapExecution<'a>>,
    operation: &'static str,
) -> VmResult<MaterializedHostArgs<'a>> {
    if registers.is_empty() {
        return Ok(MaterializedHostArgs::Empty);
    }
    registers
        .iter()
        .map(|register| host_arg_from_value(&frame.read(*register)?, heap, operation))
        .collect::<VmResult<Vec<_>>>()
        .map(MaterializedHostArgs::Values)
}

fn host_arg_from_value<'a>(
    value: &Value,
    heap: Option<&'a HeapExecution<'a>>,
    operation: &'static str,
) -> VmResult<HostPathArg<'a>> {
    match value {
        Value::I64(index) => {
            let index = u32::try_from(*index).map_err(|_| {
                VmError::new(VmErrorKind::TypeMismatch {
                    operation: "host path index",
                })
            })?;
            Ok(HostPathArg::Index(index))
        }
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::String(value)) => Ok(HostPathArg::Key(HostCollectionKeyRef::String(
                value.as_str(),
            ))),
            _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
        },
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
    }
}
