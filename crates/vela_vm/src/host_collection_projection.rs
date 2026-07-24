use vela_bytecode::{CacheSiteId, Register};
use vela_common::ScalarValue;
use vela_host::protocol::{HostCollectionProjection, HostCollectionQuery, HostCollectionSnapshot};
use vela_host::resolved::{HostAccessOp, HostAccessSpec};
use vela_host::target::{HostTargetInstance, HostTargetPlan};

use crate::heap::HeapValue;
use crate::heap_values::allocate_heap_value;
use crate::host_access::{
    HostAccessRuntime, missing_host_context, resolve_cached_access, runtime_value_from_host,
};
use crate::{
    HostInlineCacheTarget, StandardMethodReceiver, Value, VmError, VmErrorKind, VmResult,
    expect_host_ref, std_method_ids::HostArrayTransform,
};

pub(crate) fn execute_host_root_array_iteration(
    mut runtime: HostAccessRuntime<'_, '_, '_>,
    receiver: Register,
    args: &[Value],
) -> VmResult<Value> {
    if !args.is_empty() {
        return Err(VmError::new(VmErrorKind::ArityMismatch {
            name: "host array iteration".to_owned(),
            expected: 0,
            actual: args.len(),
        }));
    }
    let root = expect_host_ref(
        &runtime.frame.read(receiver)?,
        runtime.host.as_deref(),
        "host array iteration",
    )?;
    let root_target = HostTargetPlan::new(root.type_id);
    let root_instance = HostTargetInstance::new(root, &root_target, &[]);
    let element_target = HostTargetPlan::new(root.type_id).dyn_index(0);
    let host = runtime.host.as_deref_mut().ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "host context",
        })
    })?;
    let root_access = host
        .adapter
        .resolve_host_access(HostAccessSpec::new(HostAccessOp::Read, &root_target))
        .map_err(|error| error.with_source_span_if_absent(runtime.source_span))?;
    let len = host
        .access
        .query_collection_resolved(
            host.adapter,
            root_access,
            root_instance,
            HostCollectionQuery::Len,
            runtime.source_span,
        )
        .and_then(|value| match value {
            vela_host::value::HostValue::Scalar(ScalarValue::I64(value)) => usize::try_from(value)
                .map_err(|_| vela_host::error::HostError {
                    kind: vela_host::error::HostErrorKind::InvalidArgument {
                        expected: "host array length",
                    },
                    source_span: runtime.source_span,
                }),
            _ => Err(vela_host::error::HostError {
                kind: vela_host::error::HostErrorKind::InvalidArgument {
                    expected: "host array length",
                },
                source_span: runtime.source_span,
            }),
        })?;
    let element_access = host
        .adapter
        .resolve_host_access(HostAccessSpec::new(HostAccessOp::Read, &element_target))
        .map_err(|error| error.with_source_span_if_absent(runtime.source_span))?;
    let Some(heap) = runtime.heap.as_deref_mut() else {
        return Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "host array iterator heap",
        }));
    };
    allocate_heap_value(
        HeapValue::Iterator(crate::iteration::IteratorState::from_host_array(
            root,
            element_target,
            element_access,
            len,
        )),
        heap,
        runtime.budget.as_deref_mut(),
    )
}

pub(crate) fn execute_host_root_collection_projection(
    mut runtime: HostAccessRuntime<'_, '_, '_>,
    receiver: Register,
    projection: HostCollectionProjection,
    args: &[Value],
    cache_site: Option<CacheSiteId>,
) -> VmResult<Value> {
    if !args.is_empty() {
        return Err(VmError::new(VmErrorKind::ArityMismatch {
            name: projection.name().to_owned(),
            expected: 0,
            actual: args.len(),
        }));
    }
    let snapshot = snapshot_host_root_collection(&mut runtime, receiver, projection, cache_site)?;
    charge_projection(&snapshot, runtime.budget.as_deref_mut())?;
    let values = snapshot_values(snapshot, &mut runtime)?;
    let heap = runtime.heap.as_deref_mut().ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "host collection projection",
        })
    })?;
    crate::heap_values::allocate_heap_value(
        HeapValue::Iterator(crate::iteration::IteratorState::from_values(values)),
        heap,
        runtime.budget.as_deref_mut(),
    )
}

/// Captures one HostRef-backed collection and materializes only the temporary
/// script collection needed by an existing resumable callback method.
///
/// The host adapter sees a semantic projection, never a Vela method ID. The
/// resulting Array/Map/Set is script-owned and detached from later structural
/// host changes; HostRef leaves keep their ordinary handle identity.
pub(crate) fn materialize_host_root_collection_for_callback(
    mut runtime: HostAccessRuntime<'_, '_, '_>,
    receiver: Register,
    receiver_kind: StandardMethodReceiver,
    projection: HostCollectionProjection,
    cache_site: Option<CacheSiteId>,
) -> VmResult<Value> {
    let snapshot = snapshot_host_root_collection(&mut runtime, receiver, projection, cache_site)?;
    charge_projection(&snapshot, runtime.budget.as_deref_mut())?;
    match (receiver_kind, snapshot) {
        (StandardMethodReceiver::Array, HostCollectionSnapshot::Items(items)) => {
            let values = host_items_to_values(items, &mut runtime)?;
            let heap = runtime
                .heap
                .as_deref_mut()
                .ok_or_else(missing_projection_heap)?;
            crate::heap_values::make_array_value(values, heap, runtime.budget.as_deref_mut())
        }
        (StandardMethodReceiver::Map, HostCollectionSnapshot::Entries(entries)) => {
            let entries = host_entries_to_values(entries, &mut runtime)?;
            crate::map_methods::make_map_from_entries(
                entries,
                &mut runtime.heap,
                &mut runtime.budget,
                "host collection callback snapshot",
            )
        }
        (StandardMethodReceiver::Set, HostCollectionSnapshot::Items(items)) => {
            let values = host_items_to_values(items, &mut runtime)?;
            let heap = runtime
                .heap
                .as_deref_mut()
                .ok_or_else(missing_projection_heap)?;
            crate::heap_values::make_set_value(values, heap, runtime.budget.as_deref_mut())
        }
        _ => Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "host collection callback snapshot",
        })),
    }
}

pub(crate) fn project_host_root_collection_items(
    runtime: &mut HostAccessRuntime<'_, '_, '_>,
    receiver: Register,
    cache_site: Option<CacheSiteId>,
) -> VmResult<Vec<Value>> {
    let snapshot = snapshot_host_root_collection(
        runtime,
        receiver,
        HostCollectionProjection::Values,
        cache_site,
    )?;
    charge_projection(&snapshot, runtime.budget.as_deref_mut())?;
    let HostCollectionSnapshot::Items(items) = snapshot else {
        return Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "host collection values projection",
        }));
    };
    host_items_to_values(items, runtime)
}

pub(crate) fn project_host_root_collection_entries(
    runtime: &mut HostAccessRuntime<'_, '_, '_>,
    receiver: Register,
    cache_site: Option<CacheSiteId>,
) -> VmResult<Vec<(Value, Value)>> {
    let snapshot = snapshot_host_root_collection(
        runtime,
        receiver,
        HostCollectionProjection::Entries,
        cache_site,
    )?;
    charge_projection(&snapshot, runtime.budget.as_deref_mut())?;
    let HostCollectionSnapshot::Entries(entries) = snapshot else {
        return Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "host collection entries projection",
        }));
    };
    host_entries_to_values(entries, runtime)
}

pub(crate) fn snapshot_host_collection_value(
    runtime: &mut HostAccessRuntime<'_, '_, '_>,
    receiver: &Value,
    projection: HostCollectionProjection,
) -> VmResult<HostCollectionSnapshot> {
    let root = expect_host_ref(
        receiver,
        runtime.host.as_deref(),
        "host collection projection",
    )?;
    let snapshot = snapshot_host_collection_root(runtime, root, projection, None)?;
    charge_projection(&snapshot, runtime.budget.as_deref_mut())?;
    Ok(snapshot)
}

pub(crate) fn execute_host_root_array_transform(
    mut runtime: HostAccessRuntime<'_, '_, '_>,
    receiver: Register,
    transform: HostArrayTransform,
    args: &[Value],
    cache_site: Option<CacheSiteId>,
) -> VmResult<Value> {
    if args.len() != transform.arity() {
        return Err(VmError::new(VmErrorKind::ArityMismatch {
            name: transform.name().to_owned(),
            expected: transform.arity(),
            actual: args.len(),
        }));
    }
    let values = project_host_root_collection_items(&mut runtime, receiver, cache_site)?;
    match transform {
        HostArrayTransform::Distinct => {
            crate::array_methods::distinct_projected(values, &mut runtime.heap, &mut runtime.budget)
        }
        HostArrayTransform::Join => crate::array_methods::join_projected(
            values,
            args,
            &mut runtime.heap,
            &mut runtime.budget,
        ),
        HostArrayTransform::Reverse => {
            crate::array_methods::reverse_projected(values, &mut runtime.heap, &mut runtime.budget)
        }
        HostArrayTransform::Slice => crate::array_methods::slice_projected(
            values,
            args,
            &mut runtime.heap,
            &mut runtime.budget,
        ),
    }
}

fn snapshot_host_root_collection(
    runtime: &mut HostAccessRuntime<'_, '_, '_>,
    receiver: Register,
    projection: HostCollectionProjection,
    cache_site: Option<CacheSiteId>,
) -> VmResult<HostCollectionSnapshot> {
    let receiver = runtime.frame.read(receiver)?;
    let root = expect_host_ref(
        &receiver,
        runtime.host.as_deref(),
        "host collection projection",
    )?;
    snapshot_host_collection_root(runtime, root, projection, cache_site)
}

fn snapshot_host_collection_root(
    runtime: &mut HostAccessRuntime<'_, '_, '_>,
    root: vela_host::path::HostRef,
    projection: HostCollectionProjection,
    cache_site: Option<CacheSiteId>,
) -> VmResult<HostCollectionSnapshot> {
    let target = HostTargetPlan::new(root.type_id);
    let instance = HostTargetInstance::new(root, &target, &[]);
    let host = runtime
        .host
        .as_deref_mut()
        .ok_or_else(missing_host_context)?;
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
    Ok(host.access.snapshot_collection_resolved(
        host.adapter,
        resolved,
        instance,
        projection,
        runtime.source_span,
    )?)
}

fn charge_projection(
    snapshot: &HostCollectionSnapshot,
    budget: Option<&mut crate::ExecutionBudget>,
) -> VmResult<()> {
    let Some(budget) = budget else {
        return Ok(());
    };
    let len = match snapshot {
        HostCollectionSnapshot::Items(items) => items.len(),
        HostCollectionSnapshot::Entries(entries) => entries.len(),
    };
    budget.charge_execution_units(u64::try_from(len).unwrap_or(u64::MAX))
}

fn snapshot_values(
    snapshot: HostCollectionSnapshot,
    runtime: &mut HostAccessRuntime<'_, '_, '_>,
) -> VmResult<Vec<Value>> {
    match snapshot {
        HostCollectionSnapshot::Items(items) => items
            .into_iter()
            .map(|item| {
                runtime_value_from_host(
                    item,
                    runtime.heap.as_deref_mut(),
                    runtime.budget.as_deref_mut(),
                    runtime
                        .host
                        .as_deref_mut()
                        .ok_or_else(missing_host_context)?,
                )
            })
            .collect(),
        HostCollectionSnapshot::Entries(entries) => {
            let mut values = Vec::with_capacity(entries.len());
            for (key, value) in entries {
                let key = runtime_value_from_host(
                    key,
                    runtime.heap.as_deref_mut(),
                    runtime.budget.as_deref_mut(),
                    runtime
                        .host
                        .as_deref_mut()
                        .ok_or_else(missing_host_context)?,
                )?;
                let value = runtime_value_from_host(
                    value,
                    runtime.heap.as_deref_mut(),
                    runtime.budget.as_deref_mut(),
                    runtime
                        .host
                        .as_deref_mut()
                        .ok_or_else(missing_host_context)?,
                )?;
                values.push(crate::map_methods::map_entry(
                    key,
                    value,
                    &mut runtime.heap,
                    &mut runtime.budget,
                )?);
            }
            Ok(values)
        }
    }
}

fn host_items_to_values(
    items: Vec<vela_host::value::HostValue>,
    runtime: &mut HostAccessRuntime<'_, '_, '_>,
) -> VmResult<Vec<Value>> {
    items
        .into_iter()
        .map(|item| {
            runtime_value_from_host(
                item,
                runtime.heap.as_deref_mut(),
                runtime.budget.as_deref_mut(),
                runtime
                    .host
                    .as_deref_mut()
                    .ok_or_else(missing_host_context)?,
            )
        })
        .collect()
}

fn host_entries_to_values(
    entries: Vec<(vela_host::value::HostValue, vela_host::value::HostValue)>,
    runtime: &mut HostAccessRuntime<'_, '_, '_>,
) -> VmResult<Vec<(Value, Value)>> {
    entries
        .into_iter()
        .map(|(key, value)| {
            let key = runtime_value_from_host(
                key,
                runtime.heap.as_deref_mut(),
                runtime.budget.as_deref_mut(),
                runtime
                    .host
                    .as_deref_mut()
                    .ok_or_else(missing_host_context)?,
            )?;
            let value = runtime_value_from_host(
                value,
                runtime.heap.as_deref_mut(),
                runtime.budget.as_deref_mut(),
                runtime
                    .host
                    .as_deref_mut()
                    .ok_or_else(missing_host_context)?,
            )?;
            Ok((key, value))
        })
        .collect()
}

fn missing_projection_heap() -> VmError {
    VmError::new(VmErrorKind::TypeMismatch {
        operation: "host collection callback snapshot",
    })
}
