use vela_bytecode::Register;
use vela_host::resolved::{HostAccessOp, HostAccessSpec};
use vela_host::target::{HostPathArg, HostTargetInstance, HostTargetPlan};
use vela_host::value::HostValue;

use crate::host_access_helpers::runtime_collection_index;
use crate::host_values::value_to_host;
use crate::{Value, VmResult, expect_host_ref};

use super::{HostAccessRuntime, missing_host_context, runtime_value_from_host};

pub(crate) fn execute_host_collection_index_read(
    mut runtime: HostAccessRuntime<'_, '_, '_>,
    receiver: Register,
    index: Register,
) -> VmResult<Value> {
    let root = expect_host_ref(
        &runtime.frame.read(receiver)?,
        runtime.host.as_deref(),
        "host collection index",
    )?;
    let index = runtime_collection_index(
        &runtime.frame.read(index)?,
        runtime.heap.as_deref(),
        runtime.host.as_deref(),
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
    let value =
        host.access
            .read_resolved_scoped(host.adapter, access, instance, runtime.source_span)?;
    runtime_value_from_host(value, runtime.heap.take(), runtime.budget.take(), host)
}

pub(crate) fn execute_host_collection_string_key_read(
    mut runtime: HostAccessRuntime<'_, '_, '_>,
    receiver: Register,
    key: &str,
) -> VmResult<Value> {
    let root = expect_host_ref(
        &runtime.frame.read(receiver)?,
        runtime.host.as_deref(),
        "host collection index",
    )?;
    let target = HostTargetPlan::new(root.type_id).const_key(key);
    let instance = HostTargetInstance::new(root, &target, &[]);
    let host = runtime.host.ok_or_else(missing_host_context)?;
    let access = host
        .adapter
        .resolve_host_access(HostAccessSpec::new(HostAccessOp::Read, &target))
        .map_err(|error| error.with_source_span_if_absent(runtime.source_span))?;
    let value =
        host.access
            .read_resolved_scoped(host.adapter, access, instance, runtime.source_span)?;
    runtime_value_from_host(value, runtime.heap.take(), runtime.budget.take(), host)
}

pub(crate) fn execute_host_collection_index_write(
    runtime: HostAccessRuntime<'_, '_, '_>,
    receiver: Register,
    index: Register,
    src: Register,
) -> VmResult<()> {
    let root = expect_host_ref(
        &runtime.frame.read(receiver)?,
        runtime.host.as_deref(),
        "host collection index assignment",
    )?;
    let index = runtime_collection_index(
        &runtime.frame.read(index)?,
        runtime.heap.as_deref(),
        runtime.host.as_deref(),
        "host collection index assignment",
    )?;
    let value = value_to_host(
        &runtime.frame.read(src)?,
        "host collection index assignment",
        runtime.heap.as_deref(),
        runtime.host.as_deref(),
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
        runtime.host.as_deref(),
        "host collection index assignment",
    )?;
    let value = value_to_host(
        &runtime.frame.read(src)?,
        "host collection index assignment",
        runtime.heap.as_deref(),
        runtime.host.as_deref(),
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
