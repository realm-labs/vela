use vela_bytecode::{HostPathSegment, HostTargetPlanId, Register};
use vela_common::{FieldId, GlobalSlot, HostMethodId, Span, SymbolInterner};
use vela_host::adapter::GlobalBinding;
use vela_host::path::HostPath;
use vela_host::resolved::HostMutationOp;
use vela_host::target::{HostPathArg, HostTargetInstance, HostTargetPlan};
use vela_host::value::HostValue;

use crate::heap::HeapValue;
use crate::heap_values::host_to_value;
use crate::host_mutations;
pub(crate) use crate::host_mutations::HostNumericMutation;
use crate::host_paths::{host_field_path, host_path_from_segments};
use crate::host_values::{value_from_host, value_to_host};
use crate::{
    CallFrame, ExecutionBudget, HeapExecution, HostExecution, Value, VmError, VmErrorKind,
    VmResult, expect_host_ref,
};

pub(crate) struct HostAccessRuntime<'a, 'host, 'heap> {
    pub(crate) frame: &'a CallFrame,
    pub(crate) heap: Option<&'a mut HeapExecution<'heap>>,
    pub(crate) budget: Option<&'a mut ExecutionBudget>,
    pub(crate) host: Option<&'a mut HostExecution<'host>>,
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

pub(crate) fn read_host_field(
    runtime: HostAccessRuntime<'_, '_, '_>,
    root: Register,
    field: FieldId,
) -> VmResult<Value> {
    let root = expect_host_ref(runtime.frame.read(root)?, "get_host_field")?;
    let path = host_field_path(root, field);
    read_host_path_value(path, runtime)
}

pub(crate) fn read_host_path(
    runtime: HostAccessRuntime<'_, '_, '_>,
    root: Register,
    segments: &[HostPathSegment],
    symbols: &mut SymbolInterner,
) -> VmResult<Value> {
    let root = expect_host_ref(runtime.frame.read(root)?, "get_host_path")?;
    let path = host_path_from_segments(
        root,
        segments,
        runtime.frame,
        runtime.heap.as_deref(),
        symbols,
    )?;
    read_host_path_value(path, runtime)
}

pub(crate) fn set_host_field(
    runtime: HostAccessRuntime<'_, '_, '_>,
    root: Register,
    field: FieldId,
    src: Register,
) -> VmResult<()> {
    let root = expect_host_ref(runtime.frame.read(root)?, "set_host_field")?;
    let value = value_to_host(
        runtime.frame.read(src)?,
        "set_host_field",
        runtime.heap.as_deref(),
    )?;
    let path = host_field_path(root, field);
    set_host_path_value(path, value, runtime)
}

pub(crate) fn set_host_path(
    runtime: HostAccessRuntime<'_, '_, '_>,
    root: Register,
    segments: &[HostPathSegment],
    src: Register,
    symbols: &mut SymbolInterner,
) -> VmResult<()> {
    let root = expect_host_ref(runtime.frame.read(root)?, "set_host_path")?;
    let value = value_to_host(
        runtime.frame.read(src)?,
        "set_host_path",
        runtime.heap.as_deref(),
    )?;
    let path = host_path_from_segments(
        root,
        segments,
        runtime.frame,
        runtime.heap.as_deref(),
        symbols,
    )?;
    set_host_path_value(path, value, runtime)
}

pub(crate) fn execute_host_read(
    runtime: HostAccessRuntime<'_, '_, '_>,
    root: Register,
    target: &HostTargetPlan,
    dynamic_args: &[Register],
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
    let value = host
        .access
        .read(host.adapter, instance, runtime.source_span)?;
    runtime_value_from_host(value, runtime.heap, runtime.budget)
}

pub(crate) fn execute_host_write(
    runtime: HostAccessRuntime<'_, '_, '_>,
    root: Register,
    target: &HostTargetPlan,
    dynamic_args: &[Register],
    src: Register,
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
    host.access
        .write(host.adapter, instance, value, runtime.source_span)?;
    Ok(())
}

pub(crate) fn execute_host_mutate(
    runtime: HostAccessRuntime<'_, '_, '_>,
    root: Register,
    target: &HostTargetPlan,
    dynamic_args: &[Register],
    op: HostMutationOp,
    rhs: Register,
) -> VmResult<()> {
    let root = expect_host_ref(runtime.frame.read(root)?, "host_mutate")?;
    let value = value_to_host(
        runtime.frame.read(rhs)?,
        "host_mutate",
        runtime.heap.as_deref(),
    )?;
    let args = materialize_host_args(
        runtime.frame,
        dynamic_args,
        runtime.heap.as_deref(),
        "host_mutate",
    )?;
    let instance = HostTargetInstance::new(root, target, &args);
    let host = runtime.host.ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "host context",
        })
    })?;
    host.access
        .mutate(host.adapter, instance, op, value, runtime.source_span)?;
    Ok(())
}

pub(crate) fn execute_host_remove(
    runtime: HostAccessRuntime<'_, '_, '_>,
    root: Register,
    target: &HostTargetPlan,
    dynamic_args: &[Register],
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
    host.access
        .remove(host.adapter, instance, runtime.source_span)?;
    Ok(())
}

pub(crate) struct HostCallPlan<'a> {
    pub(crate) target: &'a HostTargetPlan,
    pub(crate) dynamic_args: &'a [Register],
    pub(crate) method: HostMethodId,
    pub(crate) args: &'a [Register],
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
    let value = host.access.call(
        host.adapter,
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

pub(crate) fn write_host_field_numeric_mutation(
    runtime: HostAccessRuntime<'_, '_, '_>,
    root: Register,
    field: FieldId,
    rhs: Register,
    patch: HostNumericMutation,
) -> VmResult<()> {
    host_mutations::write_host_field_numeric_mutation(
        host_mutations::HostMutationRuntime {
            frame: runtime.frame,
            heap: runtime.heap.as_deref(),
            host: runtime.host,
            source_span: runtime.source_span,
        },
        root,
        field,
        rhs,
        patch,
    )
}

pub(crate) fn write_host_path_numeric_mutation(
    runtime: HostAccessRuntime<'_, '_, '_>,
    root: Register,
    segments: &[HostPathSegment],
    rhs: Register,
    patch: HostNumericMutation,
    symbols: &mut SymbolInterner,
) -> VmResult<()> {
    host_mutations::write_host_path_numeric_mutation(
        host_mutations::HostMutationRuntime {
            frame: runtime.frame,
            heap: runtime.heap.as_deref(),
            host: runtime.host,
            source_span: runtime.source_span,
        },
        root,
        segments,
        rhs,
        patch,
        symbols,
    )
}

pub(crate) fn push_host_path(
    runtime: HostAccessRuntime<'_, '_, '_>,
    root: Register,
    segments: &[HostPathSegment],
    value: Register,
    symbols: &mut SymbolInterner,
) -> VmResult<()> {
    let root = expect_host_ref(runtime.frame.read(root)?, "push_host_path")?;
    let value = value_to_host(
        runtime.frame.read(value)?,
        "push_host_path",
        runtime.heap.as_deref(),
    )?;
    let path = host_path_from_segments(
        root,
        segments,
        runtime.frame,
        runtime.heap.as_deref(),
        symbols,
    )?;
    let host = runtime.host.ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "host context",
        })
    })?;
    host.access
        .push_path(host.adapter, path, value, runtime.source_span)?;
    Ok(())
}

pub(crate) fn remove_host_path(
    runtime: HostAccessRuntime<'_, '_, '_>,
    root: Register,
    segments: &[HostPathSegment],
    symbols: &mut SymbolInterner,
) -> VmResult<()> {
    let root = expect_host_ref(runtime.frame.read(root)?, "remove_host_path")?;
    let path = host_path_from_segments(
        root,
        segments,
        runtime.frame,
        runtime.heap.as_deref(),
        symbols,
    )?;
    let host = runtime.host.ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "host context",
        })
    })?;
    host.access
        .remove_path(host.adapter, path, runtime.source_span)?;
    Ok(())
}

pub(crate) fn call_host_method(
    runtime: HostAccessRuntime<'_, '_, '_>,
    root: Register,
    segments: &[HostPathSegment],
    method: HostMethodId,
    args: &[Register],
    wants_return: bool,
    symbols: &mut SymbolInterner,
) -> VmResult<Option<Value>> {
    let root = expect_host_ref(runtime.frame.read(root)?, "call_host_method")?;
    let path = host_path_from_segments(
        root,
        segments,
        runtime.frame,
        runtime.heap.as_deref(),
        symbols,
    )?;
    let values = args
        .iter()
        .map(|register| {
            value_to_host(
                runtime.frame.read(*register)?,
                "call_host_method",
                runtime.heap.as_deref(),
            )
        })
        .collect::<VmResult<Vec<_>>>()?;
    let host = runtime.host.ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "host context",
        })
    })?;
    let return_value =
        host.access
            .call_method(host.adapter, path, method, values, runtime.source_span)?;
    if wants_return {
        runtime_value_from_host(return_value, runtime.heap, runtime.budget).map(Some)
    } else {
        Ok(None)
    }
}

fn read_host_path_value(path: HostPath, runtime: HostAccessRuntime<'_, '_, '_>) -> VmResult<Value> {
    let host = runtime.host.ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "host context",
        })
    })?;
    let value = host
        .access
        .read_path_at(host.adapter, &path, runtime.source_span)?;
    runtime_value_from_host(value, runtime.heap, runtime.budget)
}

fn set_host_path_value(
    path: HostPath,
    value: HostValue,
    runtime: HostAccessRuntime<'_, '_, '_>,
) -> VmResult<()> {
    let host = runtime.host.ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "host context",
        })
    })?;
    host.access
        .set_path(host.adapter, path, value, runtime.source_span)?;
    Ok(())
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
        Value::Int(index) => {
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
