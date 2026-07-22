use vela_bytecode::{CacheSiteId, Register};
use vela_host::protocol::{HostCollectionProjection, HostCollectionSnapshot};
use vela_host::resolved::{HostAccessOp, HostAccessSpec};
use vela_host::target::{HostTargetInstance, HostTargetPlan};

use crate::heap::HeapValue;
use crate::host_access::{
    HostAccessRuntime, missing_host_context, resolve_cached_access, runtime_value_from_host,
};
use crate::{HostInlineCacheTarget, Value, VmError, VmErrorKind, VmResult, expect_host_ref};

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
    let root = expect_host_ref(&runtime.frame.read(receiver)?, "host collection projection")?;
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
    let snapshot = host.access.snapshot_collection_resolved(
        host.adapter,
        resolved,
        instance,
        projection,
        runtime.source_span,
    )?;
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
                )?;
                let value = runtime_value_from_host(
                    value,
                    runtime.heap.as_deref_mut(),
                    runtime.budget.as_deref_mut(),
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
