use vela_bytecode::CacheSiteId;
use vela_common::Span;
use vela_host::error::HostErrorKind;
use vela_host::resolved::HostAccessOp;
use vela_host::target::HostTargetInstance;
use vela_host::value::HostValue;

use super::{resolve_collection_key_access, runtime_value_from_host};
use crate::{
    ExecutionBudget, HeapExecution, HostExecution, Value, VmError, VmErrorKind, VmInlineCaches,
    VmResult,
};

pub(super) struct Runtime<'a, 'host, 'heap> {
    pub(super) heap: Option<&'a mut HeapExecution<'heap>>,
    pub(super) budget: Option<&'a mut ExecutionBudget>,
    pub(super) inline_caches: Option<&'a dyn VmInlineCaches>,
    pub(super) source_span: Option<Span>,
    pub(super) host: &'a mut HostExecution<'host>,
}

pub(super) fn execute(
    mut runtime: Runtime<'_, '_, '_>,
    cache_site: Option<CacheSiteId>,
    instance: HostTargetInstance<'_>,
    map_value: HostValue,
) -> VmResult<Value> {
    let read = resolve_collection_key_access(
        runtime.host,
        runtime.inline_caches,
        cache_site,
        instance,
        HostAccessOp::Read,
        runtime.source_span,
    )?;
    let current = match runtime.host.access.read_resolved_scoped(
        runtime.host.adapter,
        read,
        instance,
        runtime.source_span,
    ) {
        Ok(value) => Some(value),
        Err(error) if matches!(&error.kind, HostErrorKind::MissingCollectionEntry { .. }) => None,
        Err(error) => return Err(error.into()),
    };
    let current = current
        .map(|value| {
            runtime_value_from_host(
                value,
                runtime.heap.as_deref_mut(),
                runtime.budget.as_deref_mut(),
                runtime.host,
            )
        })
        .transpose()?;
    let heap = runtime.heap.ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "host map insert",
        })
    })?;
    let result = crate::option_result::option_value(current, heap, runtime.budget)?;
    let write = resolve_collection_key_access(
        runtime.host,
        runtime.inline_caches,
        cache_site,
        instance,
        HostAccessOp::Write,
        runtime.source_span,
    )?;
    runtime.host.access.write_resolved(
        runtime.host.adapter,
        write,
        instance,
        map_value,
        runtime.source_span,
    )?;
    Ok(result)
}
