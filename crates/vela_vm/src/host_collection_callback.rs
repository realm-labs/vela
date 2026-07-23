use vela_bytecode::{CacheSiteId, Register};
use vela_def::MethodId;

use crate::callback_method_dispatch::{callback_cache_entry, host_collection_callback_cache_entry};
use crate::host_access::HostAccessRuntime;
use crate::{CallbackMethodInlineCacheEntry, Value, VmResult};

pub(crate) struct PreparedCallbackReceiver {
    pub(crate) value: Value,
    pub(crate) cache: CallbackMethodInlineCacheEntry,
    pub(crate) cacheable_receiver: bool,
}

/// Prepares one standard callback receiver.
///
/// Owned script values retain their existing zero-copy dispatch. A HostRef
/// collection is captured once through its semantic projection protocol and
/// materialized as the corresponding temporary Array, Map, or Set before the
/// ordinary resumable callback state machine starts.
pub(crate) fn prepare_callback_receiver(
    method_id: MethodId,
    receiver_value: Value,
    runtime: HostAccessRuntime<'_, '_, '_>,
    receiver: Register,
    cache_site: Option<CacheSiteId>,
) -> VmResult<Option<PreparedCallbackReceiver>> {
    if !matches!(receiver_value, Value::HostRef(_)) {
        return Ok(
            callback_cache_entry(method_id, &receiver_value, runtime.heap.as_deref()).map(
                |cache| PreparedCallbackReceiver {
                    value: receiver_value,
                    cache,
                    cacheable_receiver: true,
                },
            ),
        );
    }

    let Some((cache, projection)) = host_collection_callback_cache_entry(method_id) else {
        return Ok(None);
    };
    let value = crate::host_collection_projection::materialize_host_root_collection_for_callback(
        runtime,
        receiver,
        cache.receiver,
        projection,
        cache_site,
    )?;
    Ok(Some(PreparedCallbackReceiver {
        value,
        cache,
        cacheable_receiver: false,
    }))
}
