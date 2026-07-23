use vela_bytecode::{CacheSiteId, Register};
use vela_host::error::HostErrorKind;
use vela_host::resolved::{HostAccessOp, HostAccessSpec};
use vela_host::target::HostTargetInstance;
use vela_host::value::HostValue;

use crate::host_access::{
    HostAccessRuntime, missing_host_context, resolve_cached_access, runtime_value_from_host,
};
use crate::host_access_helpers::runtime_collection_index;
use crate::host_collection_edges::{HostArrayEdge, host_array_edge_index};
use crate::std_method_ids::HostCollectionLookup;
use crate::{HostInlineCacheTarget, Value, VmError, VmErrorKind, VmResult, expect_host_ref};

pub(crate) fn execute_host_root_collection_lookup(
    mut runtime: HostAccessRuntime<'_, '_, '_>,
    receiver: Register,
    lookup: HostCollectionLookup,
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
    let root = expect_host_ref(
        &runtime.frame.read(receiver)?,
        runtime.host.as_deref(),
        "host collection lookup",
    )?;
    let array_index = match lookup {
        HostCollectionLookup::ArrayFirst => {
            host_array_edge_index(&mut runtime, root, HostArrayEdge::First)?
        }
        HostCollectionLookup::ArrayLast => {
            host_array_edge_index(&mut runtime, root, HostArrayEdge::Last)?
        }
        HostCollectionLookup::MapHas
        | HostCollectionLookup::MapGet
        | HostCollectionLookup::MapGetOr
        | HostCollectionLookup::SetHas => None,
    };
    let key = match lookup {
        HostCollectionLookup::ArrayFirst | HostCollectionLookup::ArrayLast => array_index
            .map(|index| {
                runtime_collection_index(
                    &Value::I64(index),
                    runtime.heap.as_deref(),
                    runtime.host.as_deref(),
                    "host collection lookup",
                )
            })
            .transpose()?,
        HostCollectionLookup::MapHas
        | HostCollectionLookup::MapGet
        | HostCollectionLookup::MapGetOr
        | HostCollectionLookup::SetHas => Some(runtime_collection_index(
            &args[0],
            runtime.heap.as_deref(),
            runtime.host.as_deref(),
            "host collection lookup",
        )?),
    };
    let host = runtime
        .host
        .as_deref_mut()
        .ok_or_else(missing_host_context)?;
    let payload = if let Some(key) = key {
        let (target, arg) = key.target(root.type_id);
        let target_args = [arg];
        let instance = HostTargetInstance::new(root, &target, &target_args);
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
        match host
            .access
            .read_resolved(host.adapter, resolved, instance, runtime.source_span)
        {
            Ok(value) => Some(value),
            Err(error) if matches!(&error.kind, HostErrorKind::MissingCollectionEntry { .. }) => {
                None
            }
            Err(error) => return Err(error.into()),
        }
    } else {
        None
    };

    match lookup {
        HostCollectionLookup::MapHas => Ok(Value::Bool(payload.is_some())),
        HostCollectionLookup::SetHas => match payload {
            Some(HostValue::Bool(value)) => Ok(Value::Bool(value)),
            None => Ok(Value::Bool(false)),
            Some(_) => Err(VmError::new(VmErrorKind::TypeMismatch {
                operation: "host set has",
            })),
        },
        HostCollectionLookup::ArrayFirst
        | HostCollectionLookup::ArrayLast
        | HostCollectionLookup::MapGet => {
            let payload = payload
                .map(|payload| {
                    runtime_value_from_host(
                        payload,
                        runtime.heap.as_deref_mut(),
                        runtime.budget.as_deref_mut(),
                        host,
                    )
                })
                .transpose()?;
            let heap = runtime.heap.as_deref_mut().ok_or_else(|| {
                VmError::new(VmErrorKind::TypeMismatch {
                    operation: "host collection option lookup",
                })
            })?;
            crate::option_result::option_value(payload, heap, runtime.budget.as_deref_mut())
        }
        HostCollectionLookup::MapGetOr => match payload {
            Some(payload) => runtime_value_from_host(payload, runtime.heap, runtime.budget, host),
            None => Ok(args[1]),
        },
    }
}
