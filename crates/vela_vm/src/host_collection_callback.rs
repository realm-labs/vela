use vela_bytecode::{CacheSiteId, Register};
use vela_def::MethodId;

use crate::callback_method_dispatch::{callback_cache_entry, host_collection_callback_cache_entry};
use crate::host_access::HostAccessRuntime;
use crate::{
    CallbackMethodInlineCacheEntry, CallbackMethodInlineCacheTarget, StandardMethodReceiver, Value,
    VmResult,
};

pub(crate) struct PreparedCallbackReceiver {
    pub(crate) value: Value,
    pub(crate) cache: CallbackMethodInlineCacheEntry,
    pub(crate) cacheable_receiver: bool,
    pub(crate) host_retain_writeback: Option<HostRetainWriteback>,
    pub(crate) host_sequence: Option<crate::iteration::IteratorState>,
}

#[derive(Clone, Copy)]
pub(crate) struct HostRetainWriteback {
    pub(crate) receiver: Register,
    pub(crate) cache_site: Option<CacheSiteId>,
}

/// Prepares one standard callback receiver.
///
/// Owned script values retain their existing zero-copy dispatch. Read-only
/// HostRef Array callbacks use a prepared live iterator, while operations that
/// require a stable transactional or ordering snapshot still materialize the
/// corresponding temporary Array, Map, or Set before callback execution.
pub(crate) fn prepare_callback_receiver(
    method_id: MethodId,
    receiver_value: Value,
    mut runtime: HostAccessRuntime<'_, '_, '_>,
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
                    host_retain_writeback: None,
                    host_sequence: None,
                },
            ),
        );
    }

    let Some((cache, projection)) = host_collection_callback_cache_entry(method_id) else {
        return Ok(None);
    };
    if supports_live_host_sequence(cache) {
        let host_sequence = crate::host_collection_projection::prepare_host_root_array_iterator(
            &mut runtime,
            receiver,
        )?;
        return Ok(Some(PreparedCallbackReceiver {
            value: receiver_value,
            cache,
            cacheable_receiver: false,
            host_retain_writeback: None,
            host_sequence: Some(host_sequence),
        }));
    }
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
        host_retain_writeback: (cache.target == CallbackMethodInlineCacheTarget::Retain).then_some(
            HostRetainWriteback {
                receiver,
                cache_site,
            },
        ),
        host_sequence: None,
    }))
}

fn supports_live_host_sequence(cache: CallbackMethodInlineCacheEntry) -> bool {
    cache.receiver == StandardMethodReceiver::Array
        && matches!(
            cache.target,
            CallbackMethodInlineCacheTarget::Map
                | CallbackMethodInlineCacheTarget::Filter
                | CallbackMethodInlineCacheTarget::Find
                | CallbackMethodInlineCacheTarget::Any
                | CallbackMethodInlineCacheTarget::All
                | CallbackMethodInlineCacheTarget::Count
                | CallbackMethodInlineCacheTarget::GroupBy
                | CallbackMethodInlineCacheTarget::Sum
        )
}
