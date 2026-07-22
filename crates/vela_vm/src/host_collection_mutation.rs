use vela_bytecode::{CacheSiteId, Register};
use vela_common::ScalarValue;
use vela_host::protocol::{HostCollectionMutation, HostCollectionQuery};
use vela_host::resolved::{HostAccessOp, HostAccessSpec};
use vela_host::target::{HostTargetInstance, HostTargetPlan};
use vela_host::value::HostValue;

use crate::host_access::{HostAccessRuntime, missing_host_context, resolve_cached_access};
use crate::{HostInlineCacheTarget, Value, VmError, VmErrorKind, VmResult, expect_host_ref};

pub(crate) fn execute_host_root_collection_clear(
    mut runtime: HostAccessRuntime<'_, '_, '_>,
    receiver: Register,
    cache_site: Option<CacheSiteId>,
) -> VmResult<Value> {
    let root = expect_host_ref(&runtime.frame.read(receiver)?, "host collection clear")?;
    let target = HostTargetPlan::new(root.type_id);
    let instance = HostTargetInstance::new(root, &target, &[]);
    let host = runtime
        .host
        .as_deref_mut()
        .ok_or_else(missing_host_context)?;
    let read = resolve_access(
        host.adapter,
        runtime.inline_caches,
        cache_site,
        instance,
        HostAccessOp::Read,
        runtime.source_span,
    )?;
    let len = host.access.query_collection_resolved(
        host.adapter,
        read,
        instance,
        HostCollectionQuery::Len,
        runtime.source_span,
    )?;
    let HostValue::Scalar(ScalarValue::I64(len)) = len else {
        return Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "host collection clear length",
        }));
    };
    let units = u64::try_from(len).map_err(|_| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "host collection clear length",
        })
    })?;
    if let Some(budget) = runtime.budget.as_deref_mut() {
        budget.charge_execution_units(units)?;
    }
    let write = resolve_access(
        host.adapter,
        runtime.inline_caches,
        cache_site,
        instance,
        HostAccessOp::Write,
        runtime.source_span,
    )?;
    host.access.mutate_collection_resolved(
        host.adapter,
        write,
        instance,
        HostCollectionMutation::Clear,
        runtime.source_span,
    )?;
    Ok(Value::Unit)
}

fn resolve_access(
    adapter: &dyn vela_host::adapter::ScriptStateAdapter,
    inline_caches: Option<&dyn crate::VmInlineCaches>,
    cache_site: Option<CacheSiteId>,
    target: HostTargetInstance<'_>,
    op: HostAccessOp,
    source_span: Option<vela_common::Span>,
) -> VmResult<vela_host::resolved::ResolvedHostAccess> {
    if let Some(cache_site) = cache_site {
        resolve_cached_access(
            adapter,
            inline_caches,
            cache_site,
            HostInlineCacheTarget::RootObject,
            target,
            op,
            source_span,
        )
    } else {
        adapter
            .resolve_host_access(HostAccessSpec::new(op, target.plan))
            .map_err(|error| error.with_source_span_if_absent(source_span).into())
    }
}
