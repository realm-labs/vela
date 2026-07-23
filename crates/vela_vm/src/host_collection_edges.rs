use vela_common::ScalarValue;
use vela_host::protocol::HostCollectionQuery;
use vela_host::resolved::{HostAccessOp, HostAccessSpec};
use vela_host::target::{HostTargetInstance, HostTargetPlan};
use vela_host::value::HostValue;

use crate::host_access::{HostAccessRuntime, missing_host_context};
use crate::{VmError, VmErrorKind, VmResult};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostArrayEdge {
    First,
    Last,
}

pub(crate) fn host_array_edge_index(
    runtime: &mut HostAccessRuntime<'_, '_, '_>,
    root: vela_host::path::HostRef,
    edge: HostArrayEdge,
) -> VmResult<Option<i64>> {
    let len = host_array_len(runtime, root, "host array element lookup")?;
    if len == 0 {
        return Ok(None);
    }
    let index = match edge {
        HostArrayEdge::First => 0,
        HostArrayEdge::Last => len - 1,
    };
    i64::try_from(index).map(Some).map_err(|_| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "host array element lookup",
        })
    })
}

pub(crate) fn host_array_len(
    runtime: &mut HostAccessRuntime<'_, '_, '_>,
    root: vela_host::path::HostRef,
    operation: &'static str,
) -> VmResult<usize> {
    let target = HostTargetPlan::new(root.type_id);
    let instance = HostTargetInstance::new(root, &target, &[]);
    let host = runtime
        .host
        .as_deref_mut()
        .ok_or_else(missing_host_context)?;
    let resolved = host
        .adapter
        .resolve_host_access(HostAccessSpec::new(HostAccessOp::Read, &target))
        .map_err(|error| error.with_source_span_if_absent(runtime.source_span))?;
    match host.access.query_collection_resolved(
        host.adapter,
        resolved,
        instance,
        HostCollectionQuery::Len,
        runtime.source_span,
    )? {
        HostValue::Scalar(ScalarValue::I64(len)) if len >= 0 => {
            usize::try_from(len).map_err(|_| VmError::new(VmErrorKind::TypeMismatch { operation }))
        }
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
    }
}
