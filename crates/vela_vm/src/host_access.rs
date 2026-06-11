use vela_bytecode::{CacheSiteId, HostTargetPlanId, Register};
use vela_common::{GlobalSlot, HostMethodId, Span};
use vela_host::adapter::GlobalBinding;
use vela_host::resolved::{HostAccessOp, HostAccessSpec, HostMutationOp, ResolvedHostAccess};
use vela_host::target::{HostPathArg, HostTargetInstance, HostTargetPlan};
use vela_host::value::HostValue;

use crate::heap::HeapValue;
use crate::heap_values::host_to_value;
use crate::host_values::{value_from_host, value_to_host};
use crate::{
    CallFrame, ExecutionBudget, HeapExecution, HostExecution, HostInlineCacheEntry, Value, VmError,
    VmErrorKind, VmInlineCaches, VmResult, expect_host_ref,
};

pub(crate) struct HostAccessRuntime<'a, 'host, 'heap> {
    pub(crate) frame: &'a CallFrame,
    pub(crate) heap: Option<&'a mut HeapExecution<'heap>>,
    pub(crate) budget: Option<&'a mut ExecutionBudget>,
    pub(crate) host: Option<&'a mut HostExecution<'host>>,
    pub(crate) inline_caches: Option<&'a dyn VmInlineCaches>,
    pub(crate) source_span: Option<Span>,
}

pub(crate) fn load_host_global(
    runtime: HostAccessRuntime<'_, '_, '_>,
    name: &str,
    slot: Option<GlobalSlot>,
) -> VmResult<Value> {
    let host = runtime.host.ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "host context",
        })
    })?;
    if let Some(script_globals) = host.script_globals
        && let Some(value) = script_globals.get_resolved(name, slot)
    {
        return Ok(value);
    }
    let root = host
        .adapter
        .global_ref(GlobalBinding { name, slot })
        .map_err(|error| error.with_source_span_if_absent(runtime.source_span))?;
    Ok(Value::HostRef(root))
}

pub(crate) fn load_cached_host_global(
    runtime: HostAccessRuntime<'_, '_, '_>,
    name: &str,
    declared_slot: Option<GlobalSlot>,
    cache_site: Option<CacheSiteId>,
) -> VmResult<Value> {
    let inline_caches = runtime.inline_caches;
    let cached_slot = cache_site
        .and_then(|site| inline_caches.and_then(|caches| caches.global_read_slot(site)))
        .or(declared_slot);
    let value = load_host_global(runtime, name, cached_slot)?;
    if let (Some(caches), Some(cache_site), Some(slot)) = (inline_caches, cache_site, declared_slot)
        && caches.global_read_slot(cache_site).is_none()
    {
        caches.set_global_read_slot(cache_site, slot);
    }
    Ok(value)
}

pub(crate) fn execute_host_read(
    runtime: HostAccessRuntime<'_, '_, '_>,
    root: Register,
    target_id: HostTargetPlanId,
    target: &HostTargetPlan,
    dynamic_args: &[Register],
    cache_site: CacheSiteId,
) -> VmResult<Value> {
    let root = expect_host_ref(runtime.frame.read(root)?, "host_read")?;
    let args = materialize_host_args(
        runtime.frame,
        dynamic_args,
        runtime.heap.as_deref(),
        "host_read",
    )?;
    let instance = HostTargetInstance::new(root, target, &args);
    let host = runtime.host.ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "host context",
        })
    })?;
    let cached_access = resolve_cached_access(
        host.adapter,
        runtime.inline_caches,
        cache_site,
        target_id,
        instance,
        HostAccessOp::Read,
        runtime.source_span,
    )?;
    let value =
        host.access
            .read_resolved(host.adapter, cached_access, instance, runtime.source_span)?;
    runtime_value_from_host(value, runtime.heap, runtime.budget)
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
    let root = expect_host_ref(runtime.frame.read(root)?, "host_write")?;
    let value = value_to_host(
        runtime.frame.read(src)?,
        "set_host_field",
        runtime.heap.as_deref(),
    )?;
    let args = materialize_host_args(
        runtime.frame,
        dynamic_args,
        runtime.heap.as_deref(),
        "host_write",
    )?;
    let instance = HostTargetInstance::new(root, target, &args);
    let host = runtime.host.ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "host context",
        })
    })?;
    let cached_access = resolve_cached_access(
        host.adapter,
        runtime.inline_caches,
        cache_site,
        target_id,
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

pub(crate) fn execute_host_mutate(
    runtime: HostAccessRuntime<'_, '_, '_>,
    root: Register,
    mutation: HostMutationPlan<'_>,
) -> VmResult<()> {
    let root = expect_host_ref(runtime.frame.read(root)?, "host_mutate")?;
    let value = value_to_host(
        runtime.frame.read(mutation.rhs)?,
        "host_mutate",
        runtime.heap.as_deref(),
    )?;
    let args = materialize_host_args(
        runtime.frame,
        mutation.dynamic_args,
        runtime.heap.as_deref(),
        "host_mutate",
    )?;
    let instance = HostTargetInstance::new(root, mutation.target, &args);
    let host = runtime.host.ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "host context",
        })
    })?;
    let cached_access = resolve_cached_access(
        host.adapter,
        runtime.inline_caches,
        mutation.cache_site,
        mutation.target_id,
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

pub(crate) fn execute_host_remove(
    runtime: HostAccessRuntime<'_, '_, '_>,
    root: Register,
    target_id: HostTargetPlanId,
    target: &HostTargetPlan,
    dynamic_args: &[Register],
    cache_site: CacheSiteId,
) -> VmResult<()> {
    let root = expect_host_ref(runtime.frame.read(root)?, "host_remove")?;
    let args = materialize_host_args(
        runtime.frame,
        dynamic_args,
        runtime.heap.as_deref(),
        "host_remove",
    )?;
    let instance = HostTargetInstance::new(root, target, &args);
    let host = runtime.host.ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "host context",
        })
    })?;
    let cached_access = resolve_cached_access(
        host.adapter,
        runtime.inline_caches,
        cache_site,
        target_id,
        instance,
        HostAccessOp::Remove,
        runtime.source_span,
    )?;
    host.access
        .remove_resolved(host.adapter, cached_access, instance, runtime.source_span)?;
    Ok(())
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

pub(crate) struct HostRootMethodCall<'a> {
    pub(crate) method: HostMethodId,
    pub(crate) args: &'a [Value],
    pub(crate) wants_return: bool,
}

pub(crate) fn execute_host_call(
    runtime: HostAccessRuntime<'_, '_, '_>,
    root: Register,
    call: HostCallPlan<'_>,
) -> VmResult<Option<Value>> {
    let root = expect_host_ref(runtime.frame.read(root)?, "host_call")?;
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
                runtime.frame.read(*register)?,
                "host_call",
                runtime.heap.as_deref(),
            )
        })
        .collect::<VmResult<Vec<_>>>()?;
    let instance = HostTargetInstance::new(root, call.target, &dynamic_args);
    let host = runtime.host.ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "host context",
        })
    })?;
    let cached_access = resolve_cached_access(
        host.adapter,
        runtime.inline_caches,
        call.cache_site,
        call.target_id,
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

pub(crate) fn execute_host_root_method_call(
    runtime: HostAccessRuntime<'_, '_, '_>,
    receiver: Register,
    call: HostRootMethodCall<'_>,
) -> VmResult<Option<Value>> {
    let root = expect_host_ref(runtime.frame.read(receiver)?, "host_call")?;
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
    let resolved = host
        .adapter
        .resolve_host_access(HostAccessSpec::new(
            HostAccessOp::Call(call.method),
            &target,
        ))
        .map_err(|error| error.with_source_span_if_absent(runtime.source_span))?;
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

fn resolve_cached_access(
    adapter: &dyn vela_host::adapter::ScriptStateAdapter,
    inline_caches: Option<&dyn VmInlineCaches>,
    cache_site: CacheSiteId,
    target_id: HostTargetPlanId,
    target: HostTargetInstance<'_>,
    op: HostAccessOp,
    source_span: Option<Span>,
) -> VmResult<ResolvedHostAccess> {
    let schema_epoch = adapter.host_schema_epoch();
    if let Some(cache) = inline_caches
        && let Some(entry) = cache.host_access(cache_site)
        && entry.root_type == target.root.type_id
        && entry.plan_id == target_id
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
                plan_id: target_id,
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

fn materialize_host_args<'a>(
    frame: &CallFrame,
    registers: &[Register],
    heap: Option<&'a HeapExecution<'a>>,
    operation: &'static str,
) -> VmResult<Vec<HostPathArg<'a>>> {
    registers
        .iter()
        .map(|register| host_arg_from_value(frame.read(*register)?, heap, operation))
        .collect()
}

fn host_arg_from_value<'a>(
    value: &Value,
    heap: Option<&'a HeapExecution<'a>>,
    operation: &'static str,
) -> VmResult<HostPathArg<'a>> {
    match value {
        Value::Scalar(vela_common::ScalarValue::I64(index)) => {
            let index = u32::try_from(*index).map_err(|_| {
                VmError::new(VmErrorKind::TypeMismatch {
                    operation: "host path index",
                })
            })?;
            Ok(HostPathArg::Index(index))
        }
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::String(value)) => Ok(HostPathArg::Key(value.as_str())),
            _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
        },
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
    }
}
