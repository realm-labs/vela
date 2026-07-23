use vela_bytecode::{CacheSiteId, Register};

use crate::array_methods::{ResumableArrayOrdering, ResumableArrayOrderingKind};
use crate::host_access::HostAccessRuntime;
use crate::{Value, VmResult};

/// Prepares one Array ordering operation from either an owned Array or a
/// bounded HostRef values projection.
///
/// A host projection ends before the resumable comparison state starts. The
/// ordering state owns only Vela values and never retains a Rust borrow or
/// HostAccess guard across a nested comparison call.
pub(crate) fn prepare_array_ordering(
    kind: ResumableArrayOrderingKind,
    receiver_value: Value,
    args: &[Value],
    mut runtime: HostAccessRuntime<'_, '_, '_>,
    receiver: Register,
    cache_site: Option<CacheSiteId>,
) -> VmResult<Option<ResumableArrayOrdering>> {
    if matches!(receiver_value, Value::HostRef(_)) {
        ResumableArrayOrdering::validate_arity(kind, args)?;
        let values = crate::host_collection_projection::project_host_root_collection_items(
            &mut runtime,
            receiver,
            cache_site,
        )?;
        return ResumableArrayOrdering::from_projected_values(
            kind,
            values,
            args,
            runtime.heap.as_deref(),
        )
        .map(Some);
    }
    if crate::array_methods::is_array(&receiver_value, runtime.heap.as_deref()) {
        return ResumableArrayOrdering::new(kind, &receiver_value, args, runtime.heap.as_deref())
            .map(Some);
    }
    Ok(None)
}
