use vela_bytecode::{HostPathSegment, Register};
use vela_common::{FieldId, Span, SymbolInterner};
use vela_host::path::HostPath;
use vela_host::value::HostValue;

use crate::heap_values::host_to_value;
use crate::host_paths::host_path_from_segments;
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
    let path = HostPath::new(root).field(field);
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
    let path = HostPath::new(root).field(field);
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
        budget.reserve_patch(host.tx.patches().len())?;
    }
    host.tx.set_path(path, value, runtime.source_span)?;
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
