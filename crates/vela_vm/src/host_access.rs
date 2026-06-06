use vela_bytecode::{HostPathSegment, Register};
use vela_common::{FieldId, HostMethodId, Span, SymbolInterner};
use vela_host::path::HostPath;
use vela_host::value::HostValue;

use crate::heap_values::host_to_value;
use crate::host_patches;
pub(crate) use crate::host_patches::HostNumericPatch;
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

pub(crate) fn write_host_field_numeric_patch(
    runtime: HostAccessRuntime<'_, '_, '_>,
    root: Register,
    field: FieldId,
    rhs: Register,
    patch: HostNumericPatch,
) -> VmResult<()> {
    host_patches::write_host_field_numeric_patch(
        host_patches::HostPatchRuntime {
            frame: runtime.frame,
            heap: runtime.heap.as_deref(),
            budget: runtime.budget.as_deref(),
            host: runtime.host,
            source_span: runtime.source_span,
        },
        root,
        field,
        rhs,
        patch,
    )
}

pub(crate) fn write_host_path_numeric_patch(
    runtime: HostAccessRuntime<'_, '_, '_>,
    root: Register,
    segments: &[HostPathSegment],
    rhs: Register,
    patch: HostNumericPatch,
    symbols: &mut SymbolInterner,
) -> VmResult<()> {
    host_patches::write_host_path_numeric_patch(
        host_patches::HostPatchRuntime {
            frame: runtime.frame,
            heap: runtime.heap.as_deref(),
            budget: runtime.budget.as_deref(),
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
    if let Some(budget) = runtime.budget.as_deref() {
        budget.reserve_host_mutation(host.tx.mutation_count())?;
    }
    host.tx
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
    if let Some(budget) = runtime.budget.as_deref() {
        budget.reserve_host_mutation(host.tx.mutation_count())?;
    }
    host.tx
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
    if let Some(budget) = runtime.budget.as_deref() {
        budget.reserve_host_mutation(host.tx.mutation_count())?;
    }
    let return_value =
        host.tx
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
        .tx
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
    if let Some(budget) = runtime.budget.as_deref() {
        budget.reserve_host_mutation(host.tx.mutation_count())?;
    }
    host.tx
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
