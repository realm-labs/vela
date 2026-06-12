use std::cell::RefCell;

use vela_bytecode::CacheSiteId;
use vela_common::GlobalSlot;
use vela_vm::{
    HostInlineCacheEntry, MethodInlineCacheEntry, NativeInlineCacheEntry,
    RecordFieldInlineCacheEntry,
};

use super::image::RuntimeImage;

#[derive(Debug, Default)]
pub(super) struct InlineCaches {
    entries: RefCell<Vec<InlineCacheEntry>>,
}

#[derive(Clone, Debug)]
pub(super) enum InlineCacheEntry {
    Empty,
    GlobalRead { slot: GlobalSlot },
    HostAccess(HostInlineCacheEntry),
    RecordField(RecordFieldInlineCacheEntry),
    MethodDispatch(MethodInlineCacheEntry),
    NativeCall(NativeInlineCacheEntry),
}

impl InlineCaches {
    pub(super) fn for_image(image: &RuntimeImage) -> Self {
        Self {
            entries: RefCell::new(vec![InlineCacheEntry::Empty; image.cache_site_count()]),
        }
    }

    pub(super) fn clear_for_image(&mut self, image: &RuntimeImage) {
        *self = Self::for_image(image);
    }

    pub(super) fn len(&self) -> usize {
        self.entries.borrow().len()
    }

    pub(super) fn is_empty(&self) -> bool {
        self.entries.borrow().is_empty()
    }

    pub(super) fn global_read_slot(&self, site: CacheSiteId) -> Option<GlobalSlot> {
        match self.entries.borrow().get(site.index()) {
            Some(InlineCacheEntry::GlobalRead { slot }) => Some(*slot),
            _ => None,
        }
    }

    pub(super) fn set_global_read_slot(&self, site: CacheSiteId, slot: GlobalSlot) {
        if let Some(entry) = self.entries.borrow_mut().get_mut(site.index()) {
            *entry = InlineCacheEntry::GlobalRead { slot };
        }
    }

    pub(super) fn host_access(&self, site: CacheSiteId) -> Option<HostInlineCacheEntry> {
        match self.entries.borrow().get(site.index()) {
            Some(InlineCacheEntry::HostAccess(entry)) => Some(*entry),
            _ => None,
        }
    }

    pub(super) fn set_host_access(&self, site: CacheSiteId, entry: HostInlineCacheEntry) {
        if let Some(slot) = self.entries.borrow_mut().get_mut(site.index()) {
            *slot = InlineCacheEntry::HostAccess(entry);
        }
    }

    pub(super) fn record_field(&self, site: CacheSiteId) -> Option<RecordFieldInlineCacheEntry> {
        match self.entries.borrow().get(site.index()) {
            Some(InlineCacheEntry::RecordField(entry)) => Some(*entry),
            _ => None,
        }
    }

    pub(super) fn set_record_field(&self, site: CacheSiteId, entry: RecordFieldInlineCacheEntry) {
        if let Some(slot) = self.entries.borrow_mut().get_mut(site.index()) {
            *slot = InlineCacheEntry::RecordField(entry);
        }
    }

    pub(super) fn method_dispatch(&self, site: CacheSiteId) -> Option<MethodInlineCacheEntry> {
        match self.entries.borrow().get(site.index()) {
            Some(InlineCacheEntry::MethodDispatch(entry)) => Some(*entry),
            _ => None,
        }
    }

    pub(super) fn set_method_dispatch(&self, site: CacheSiteId, entry: MethodInlineCacheEntry) {
        if let Some(slot) = self.entries.borrow_mut().get_mut(site.index()) {
            *slot = InlineCacheEntry::MethodDispatch(entry);
        }
    }
}

impl vela_vm::VmInlineCaches for InlineCaches {
    fn len(&self) -> usize {
        self.len()
    }

    fn is_empty(&self) -> bool {
        self.is_empty()
    }

    fn global_read_slot(&self, site: CacheSiteId) -> Option<GlobalSlot> {
        self.global_read_slot(site)
    }

    fn set_global_read_slot(&self, site: CacheSiteId, slot: GlobalSlot) {
        self.set_global_read_slot(site, slot);
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

    fn native_call(&self, site: CacheSiteId) -> Option<NativeInlineCacheEntry> {
        match self.entries.borrow().get(site.index()) {
            Some(InlineCacheEntry::NativeCall(entry)) => Some(entry.clone()),
            _ => None,
        }
    }

    fn set_native_call(&self, site: CacheSiteId, entry: NativeInlineCacheEntry) {
        if let Some(slot) = self.entries.borrow_mut().get_mut(site.index()) {
            *slot = InlineCacheEntry::NativeCall(entry);
        }
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
