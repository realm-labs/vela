use std::sync::RwLock;

use vela_bytecode::{CacheSiteDesc, CacheSiteId, CacheSiteKind};
use vela_vm::{
    DynamicMethodInlineCacheEntry, HostInlineCacheEntry, MethodInlineCacheEntry,
    NativeInlineCacheEntry, RecordFieldInlineCacheEntry,
};

/// Generation-qualified cache storage shared by every Runtime using one exact
/// Engine deployment and linked artifact.
pub(super) struct GenerationInlineCaches {
    slots: Box<[GenerationCacheSlot]>,
}

enum GenerationCacheSlot {
    Empty,
    HostAccess(RwLock<Option<HostInlineCacheEntry>>),
    RecordField(RwLock<Option<RecordFieldInlineCacheEntry>>),
    Method {
        linked: RwLock<Option<MethodInlineCacheEntry>>,
        dynamic: RwLock<Option<DynamicMethodInlineCacheEntry>>,
    },
    NativeCall(RwLock<Option<NativeInlineCacheEntry>>),
}

impl GenerationInlineCaches {
    pub(super) fn for_layout(layout: &[CacheSiteDesc]) -> Self {
        let slots = layout
            .iter()
            .enumerate()
            .map(|(index, site)| {
                assert_eq!(
                    site.id.index(),
                    index,
                    "linked cache layout must be dense and site-indexed"
                );
                match site.kind {
                    CacheSiteKind::StateRead
                    | CacheSiteKind::ExternStateRead
                    | CacheSiteKind::StateWrite => GenerationCacheSlot::Empty,
                    CacheSiteKind::RecordFieldRead | CacheSiteKind::RecordFieldWrite => {
                        GenerationCacheSlot::RecordField(RwLock::new(None))
                    }
                    CacheSiteKind::MethodCall => GenerationCacheSlot::Method {
                        linked: RwLock::new(None),
                        dynamic: RwLock::new(None),
                    },
                    CacheSiteKind::HostPathRead
                    | CacheSiteKind::HostPathWrite
                    | CacheSiteKind::HostPathMutate
                    | CacheSiteKind::HostPathRemove
                    | CacheSiteKind::HostPathCall => {
                        GenerationCacheSlot::HostAccess(RwLock::new(None))
                    }
                    CacheSiteKind::NativeCall => GenerationCacheSlot::NativeCall(RwLock::new(None)),
                }
            })
            .collect();
        Self { slots }
    }

    pub(super) fn len(&self) -> usize {
        self.slots.len()
    }

    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    pub(super) fn host_access(&self, site: CacheSiteId) -> Option<HostInlineCacheEntry> {
        let GenerationCacheSlot::HostAccess(slot) = self.slots.get(site.index())? else {
            return None;
        };
        *slot.read().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub(super) fn set_host_access(&self, site: CacheSiteId, entry: HostInlineCacheEntry) {
        let Some(GenerationCacheSlot::HostAccess(slot)) = self.slots.get(site.index()) else {
            return;
        };
        *slot
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(entry);
    }

    pub(super) fn record_field(&self, site: CacheSiteId) -> Option<RecordFieldInlineCacheEntry> {
        let GenerationCacheSlot::RecordField(slot) = self.slots.get(site.index())? else {
            return None;
        };
        *slot.read().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub(super) fn set_record_field(&self, site: CacheSiteId, entry: RecordFieldInlineCacheEntry) {
        let Some(GenerationCacheSlot::RecordField(slot)) = self.slots.get(site.index()) else {
            return;
        };
        *slot
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(entry);
    }

    pub(super) fn method_dispatch(&self, site: CacheSiteId) -> Option<MethodInlineCacheEntry> {
        let GenerationCacheSlot::Method { linked, .. } = self.slots.get(site.index())? else {
            return None;
        };
        *linked
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub(super) fn set_method_dispatch(&self, site: CacheSiteId, entry: MethodInlineCacheEntry) {
        let Some(GenerationCacheSlot::Method { linked, .. }) = self.slots.get(site.index()) else {
            return;
        };
        *linked
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(entry);
    }

    pub(super) fn dynamic_method_dispatch(
        &self,
        site: CacheSiteId,
    ) -> Option<DynamicMethodInlineCacheEntry> {
        let GenerationCacheSlot::Method { dynamic, .. } = self.slots.get(site.index())? else {
            return None;
        };
        dynamic
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub(super) fn set_dynamic_method_dispatch(
        &self,
        site: CacheSiteId,
        entry: DynamicMethodInlineCacheEntry,
    ) {
        let Some(GenerationCacheSlot::Method { dynamic, .. }) = self.slots.get(site.index()) else {
            return;
        };
        *dynamic
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(entry);
    }

    pub(super) fn native_call(&self, site: CacheSiteId) -> Option<NativeInlineCacheEntry> {
        let GenerationCacheSlot::NativeCall(slot) = self.slots.get(site.index())? else {
            return None;
        };
        slot.read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub(super) fn set_native_call(&self, site: CacheSiteId, entry: NativeInlineCacheEntry) {
        let Some(GenerationCacheSlot::NativeCall(slot)) = self.slots.get(site.index()) else {
            return;
        };
        *slot
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(entry);
    }
}

impl vela_vm::VmInlineCaches for GenerationInlineCaches {
    fn len(&self) -> usize {
        self.len()
    }

    fn host_access(&self, site: CacheSiteId) -> Option<HostInlineCacheEntry> {
        self.host_access(site)
    }

    fn set_host_access(&self, site: CacheSiteId, entry: HostInlineCacheEntry) {
        self.set_host_access(site, entry);
    }

    fn record_field(&self, site: CacheSiteId) -> Option<RecordFieldInlineCacheEntry> {
        self.record_field(site)
    }

    fn set_record_field(&self, site: CacheSiteId, entry: RecordFieldInlineCacheEntry) {
        self.set_record_field(site, entry);
    }

    fn method_dispatch(&self, site: CacheSiteId) -> Option<MethodInlineCacheEntry> {
        self.method_dispatch(site)
    }

    fn set_method_dispatch(&self, site: CacheSiteId, entry: MethodInlineCacheEntry) {
        self.set_method_dispatch(site, entry);
    }

    fn dynamic_method_dispatch(&self, site: CacheSiteId) -> Option<DynamicMethodInlineCacheEntry> {
        self.dynamic_method_dispatch(site)
    }

    fn set_dynamic_method_dispatch(&self, site: CacheSiteId, entry: DynamicMethodInlineCacheEntry) {
        self.set_dynamic_method_dispatch(site, entry);
    }

    fn native_call(&self, site: CacheSiteId) -> Option<NativeInlineCacheEntry> {
        self.native_call(site)
    }

    fn set_native_call(&self, site: CacheSiteId, entry: NativeInlineCacheEntry) {
        self.set_native_call(site, entry);
    }
}

#[cfg(test)]
#[path = "inline_cache_core_tests.rs"]
mod core_tests;
#[cfg(test)]
#[path = "inline_cache_host_access_tests.rs"]
mod host_access_tests;
#[cfg(test)]
#[path = "inline_cache_host_tests.rs"]
mod host_tests;
#[cfg(test)]
#[path = "inline_cache_hot_reload_tests.rs"]
mod hot_reload_tests;
#[cfg(test)]
#[path = "inline_cache_method_tests.rs"]
mod method_tests;
#[cfg(test)]
#[path = "inline_cache_native_tests.rs"]
mod native_tests;
