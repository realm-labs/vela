use vela_bytecode::linked::LinkedMethodDispatchKind;
use vela_bytecode::{
    CacheSiteId, DebugNameId, HostTargetPlanId, LinkedProgram, MethodDispatchHandle, Register,
};
use vela_common::{GlobalSlot, HostMethodId, Span};
use vela_host::adapter::GlobalBinding;
use vela_host::resolved::{HostAccessOp, HostAccessSpec, HostMutationOp, ResolvedHostAccess};
use vela_host::target::{HostPathArg, HostTargetInstance, HostTargetPlan};
use vela_host::value::HostValue;

use crate::heap::HeapValue;
use crate::heap_values::host_to_value;
use crate::host_values::{value_from_host, value_to_host};
use crate::{
    CallFrame, ExecutionBudget, HeapExecution, HostExecution, HostInlineCacheEntry,
    HostInlineCacheTarget, Value, VmError, VmErrorKind, VmInlineCaches, VmResult, expect_host_ref,
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

pub(crate) fn load_linked_cached_host_global(
    runtime: HostAccessRuntime<'_, '_, '_>,
    program: &LinkedProgram,
    debug_name: DebugNameId,
    declared_slot: Option<GlobalSlot>,
    cache_site: Option<CacheSiteId>,
) -> VmResult<Value> {
    load_cached_host_global(
        runtime,
        program.debug_name(debug_name),
        declared_slot,
        cache_site,
    )
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
    let root = expect_host_ref(runtime.frame.read(root)?, "host_remove")?;
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
    let method_id = match call.program.method_dispatch(call.method).map(|d| &d.kind) {
        Some(LinkedMethodDispatchKind::Host { method_id }) => *method_id,
        _ => {
            return Err(VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
                opcode: "HostCall",
            })
            .with_source_span_if_absent(runtime.source_span));
        }
    };
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
        .map(|register| host_arg_from_value(frame.read(*register)?, heap, operation))
        .collect::<VmResult<Vec<_>>>()
        .map(MaterializedHostArgs::Values)
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
