use vela_bytecode::{CacheSiteId, Register};

use crate::host_access::HostAccessRuntime;
use crate::{Value, VmError, VmErrorKind, VmResult};

pub(crate) fn execute_host_root_map_merge(
    mut runtime: HostAccessRuntime<'_, '_, '_>,
    receiver: Register,
    args: &[Value],
    cache_site: Option<CacheSiteId>,
) -> VmResult<Value> {
    if args.len() != 1 {
        return Err(VmError::new(VmErrorKind::ArityMismatch {
            name: "merge".to_owned(),
            expected: 1,
            actual: args.len(),
        }));
    }
    const OPERATION: &str = "method merge";
    if !crate::map_methods::is_map(&args[0], runtime.heap.as_deref()) {
        return Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: OPERATION,
        }));
    }

    let entries = crate::host_collection_projection::project_host_root_collection_entries(
        &mut runtime,
        receiver,
        cache_site,
    )?;
    let merged = {
        let heap = runtime.heap.as_deref().ok_or_else(|| {
            VmError::new(VmErrorKind::TypeMismatch {
                operation: OPERATION,
            })
        })?;
        let right = crate::map_methods::map_slots(&args[0], Some(heap), OPERATION)?;
        crate::map_methods::merge_payload(entries, right)
    };
    crate::map_methods::make_map_from_entries(
        merged,
        &mut runtime.heap,
        &mut runtime.budget,
        OPERATION,
    )
}
