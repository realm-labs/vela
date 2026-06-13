use std::cell::{Cell, RefCell};

use vela_bytecode::CacheSiteId;
use vela_common::GlobalSlot;
use vela_vm::{
    DynamicMethodInlineCacheEntry, HostInlineCacheEntry, MethodInlineCacheEntry,
    NativeInlineCacheEntry, RecordFieldInlineCacheEntry,
};

use super::image::RuntimeImage;

#[derive(Debug, Default)]
pub(super) struct InlineCaches {
    global_reads: Vec<Cell<Option<GlobalSlot>>>,
    host_accesses: Vec<Cell<Option<HostInlineCacheEntry>>>,
    record_fields: Vec<Cell<Option<RecordFieldInlineCacheEntry>>>,
    method_dispatches: Vec<Cell<Option<MethodInlineCacheEntry>>>,
    dynamic_method_dispatches: RefCell<Vec<Option<DynamicMethodInlineCacheEntry>>>,
    native_calls: RefCell<Vec<Option<NativeInlineCacheEntry>>>,
}

impl InlineCaches {
    pub(super) fn for_image(image: &RuntimeImage) -> Self {
        let len = image.cache_site_count();
        Self {
            global_reads: empty_cell_cache(len),
            host_accesses: empty_cell_cache(len),
            record_fields: empty_cell_cache(len),
            method_dispatches: empty_cell_cache(len),
            dynamic_method_dispatches: RefCell::new(vec![None; len]),
            native_calls: RefCell::new(vec![None; len]),
        }
    }

    pub(super) fn clear_for_image(&mut self, image: &RuntimeImage) {
        *self = Self::for_image(image);
    }

    pub(super) fn len(&self) -> usize {
        self.global_reads.len()
    }

    pub(super) fn is_empty(&self) -> bool {
        self.global_reads.is_empty()
    }

    pub(super) fn global_read_slot(&self, site: CacheSiteId) -> Option<GlobalSlot> {
        self.global_reads.get(site.index()).and_then(Cell::get)
    }

    pub(super) fn set_global_read_slot(&self, site: CacheSiteId, slot: GlobalSlot) {
        if let Some(entry) = self.global_reads.get(site.index()) {
            entry.set(Some(slot));
        }
    }

    pub(super) fn host_access(&self, site: CacheSiteId) -> Option<HostInlineCacheEntry> {
        self.host_accesses.get(site.index()).and_then(Cell::get)
    }

    pub(super) fn set_host_access(&self, site: CacheSiteId, entry: HostInlineCacheEntry) {
        if let Some(slot) = self.host_accesses.get(site.index()) {
            slot.set(Some(entry));
        }
    }

    pub(super) fn record_field(&self, site: CacheSiteId) -> Option<RecordFieldInlineCacheEntry> {
        self.record_fields.get(site.index()).and_then(Cell::get)
    }

    pub(super) fn set_record_field(&self, site: CacheSiteId, entry: RecordFieldInlineCacheEntry) {
        if let Some(slot) = self.record_fields.get(site.index()) {
            slot.set(Some(entry));
        }
    }

    pub(super) fn method_dispatch(&self, site: CacheSiteId) -> Option<MethodInlineCacheEntry> {
        self.method_dispatches.get(site.index()).and_then(Cell::get)
    }

    pub(super) fn set_method_dispatch(&self, site: CacheSiteId, entry: MethodInlineCacheEntry) {
        if let Some(slot) = self.method_dispatches.get(site.index()) {
            slot.set(Some(entry));
        }
    }

    pub(super) fn dynamic_method_dispatch(
        &self,
        site: CacheSiteId,
    ) -> Option<DynamicMethodInlineCacheEntry> {
        self.dynamic_method_dispatches
            .borrow()
            .get(site.index())
            .cloned()
            .flatten()
    }

    pub(super) fn set_dynamic_method_dispatch(
        &self,
        site: CacheSiteId,
        entry: DynamicMethodInlineCacheEntry,
    ) {
        if let Some(slot) = self
            .dynamic_method_dispatches
            .borrow_mut()
            .get_mut(site.index())
        {
            *slot = Some(entry);
        }
    }

    pub(super) fn native_call(&self, site: CacheSiteId) -> Option<NativeInlineCacheEntry> {
        self.native_calls
            .borrow()
            .get(site.index())
            .cloned()
            .flatten()
    }

    pub(super) fn set_native_call(&self, site: CacheSiteId, entry: NativeInlineCacheEntry) {
        if let Some(slot) = self.native_calls.borrow_mut().get_mut(site.index()) {
            *slot = Some(entry);
        }
    }
}

fn empty_cell_cache<T: Copy>(len: usize) -> Vec<Cell<Option<T>>> {
    (0..len).map(|_| Cell::new(None)).collect()
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
