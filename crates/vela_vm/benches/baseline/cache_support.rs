use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;

use vela_bytecode::linked::InstructionKind;
use vela_bytecode::{
    CacheSiteId, DebugNameId, InstructionOffset, LinkedCodeObject, LinkedProgram, ProgramImage,
};
use vela_common::GlobalSlot;
use vela_vm::{
    HostInlineCacheEntry, MethodInlineCacheEntry, NativeInlineCacheEntry,
    RecordFieldInlineCacheEntry, VmBytecodeProfiler, VmInlineCaches,
};

#[derive(Debug, Default)]
pub(crate) struct BenchInlineCaches {
    global_reads: Vec<Cell<Option<GlobalSlot>>>,
    host_accesses: Vec<Cell<Option<HostInlineCacheEntry>>>,
    record_fields: Vec<Cell<Option<RecordFieldInlineCacheEntry>>>,
    method_dispatches: Vec<Cell<Option<MethodInlineCacheEntry>>>,
    native_calls: RefCell<Vec<Option<NativeInlineCacheEntry>>>,
    stats: Cell<BenchCacheStats>,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct BenchCacheStats {
    pub(crate) global_read_sets: usize,
    pub(crate) global_read_hits: usize,
    pub(crate) host_access_sets: usize,
    pub(crate) host_access_hits: usize,
    pub(crate) record_field_sets: usize,
    pub(crate) record_field_hits: usize,
    pub(crate) method_dispatch_sets: usize,
    pub(crate) method_dispatch_hits: usize,
    pub(crate) native_call_sets: usize,
    pub(crate) native_call_hits: usize,
}

impl BenchCacheStats {
    pub(crate) fn total_sets(self) -> usize {
        self.global_read_sets
            + self.host_access_sets
            + self.record_field_sets
            + self.method_dispatch_sets
            + self.native_call_sets
    }

    pub(crate) fn total_hits(self) -> usize {
        self.global_read_hits
            + self.host_access_hits
            + self.record_field_hits
            + self.method_dispatch_hits
            + self.native_call_hits
    }
}

#[derive(Clone, Copy)]
enum BenchCacheFamily {
    GlobalRead,
    HostAccess,
    RecordField,
    MethodDispatch,
    NativeCall,
}

impl BenchInlineCaches {
    pub(crate) fn new(len: usize) -> Self {
        Self {
            global_reads: empty_cell_cache(len),
            host_accesses: empty_cell_cache(len),
            record_fields: empty_cell_cache(len),
            method_dispatches: empty_cell_cache(len),
            native_calls: RefCell::new(vec![None; len]),
            stats: Cell::new(BenchCacheStats::default()),
        }
    }

    pub(crate) fn reset_measurement_counts(&self) {
        self.stats.set(BenchCacheStats::default());
    }

    pub(crate) fn stats(&self) -> BenchCacheStats {
        self.stats.get()
    }

    fn record_hit(&self, family: BenchCacheFamily) {
        let mut stats = self.stats.get();
        match family {
            BenchCacheFamily::GlobalRead => stats.global_read_hits += 1,
            BenchCacheFamily::HostAccess => stats.host_access_hits += 1,
            BenchCacheFamily::RecordField => stats.record_field_hits += 1,
            BenchCacheFamily::MethodDispatch => stats.method_dispatch_hits += 1,
            BenchCacheFamily::NativeCall => stats.native_call_hits += 1,
        }
        self.stats.set(stats);
    }

    fn record_copy_set<T: Copy>(
        &self,
        family: BenchCacheFamily,
        entries: &[Cell<Option<T>>],
        site: CacheSiteId,
        entry: T,
    ) {
        if let Some(slot) = entries.get(site.index()) {
            slot.set(Some(entry));
            self.record_set(family);
        }
    }

    fn record_native_set(&self, site: CacheSiteId, entry: NativeInlineCacheEntry) {
        if let Some(slot) = self.native_calls.borrow_mut().get_mut(site.index()) {
            *slot = Some(entry);
            self.record_set(BenchCacheFamily::NativeCall);
        }
    }

    fn record_set(&self, family: BenchCacheFamily) {
        let mut stats = self.stats.get();
        match family {
            BenchCacheFamily::GlobalRead => stats.global_read_sets += 1,
            BenchCacheFamily::HostAccess => stats.host_access_sets += 1,
            BenchCacheFamily::RecordField => stats.record_field_sets += 1,
            BenchCacheFamily::MethodDispatch => stats.method_dispatch_sets += 1,
            BenchCacheFamily::NativeCall => stats.native_call_sets += 1,
        }
        self.stats.set(stats);
    }
}

impl VmInlineCaches for BenchInlineCaches {
    fn len(&self) -> usize {
        self.global_reads.len()
    }

    fn global_read_slot(&self, site: CacheSiteId) -> Option<GlobalSlot> {
        let entry = self.global_reads.get(site.index()).and_then(Cell::get);
        if entry.is_some() {
            self.record_hit(BenchCacheFamily::GlobalRead);
        }
        entry
    }

    fn set_global_read_slot(&self, site: CacheSiteId, slot: GlobalSlot) {
        self.record_copy_set(BenchCacheFamily::GlobalRead, &self.global_reads, site, slot);
    }

    fn host_access(&self, site: CacheSiteId) -> Option<HostInlineCacheEntry> {
        let entry = self.host_accesses.get(site.index()).and_then(Cell::get);
        if entry.is_some() {
            self.record_hit(BenchCacheFamily::HostAccess);
        }
        entry
    }

    fn set_host_access(&self, site: CacheSiteId, entry: HostInlineCacheEntry) {
        self.record_copy_set(
            BenchCacheFamily::HostAccess,
            &self.host_accesses,
            site,
            entry,
        );
    }

    fn record_field(&self, site: CacheSiteId) -> Option<RecordFieldInlineCacheEntry> {
        let entry = self.record_fields.get(site.index()).and_then(Cell::get);
        if entry.is_some() {
            self.record_hit(BenchCacheFamily::RecordField);
        }
        entry
    }

    fn set_record_field(&self, site: CacheSiteId, entry: RecordFieldInlineCacheEntry) {
        self.record_copy_set(
            BenchCacheFamily::RecordField,
            &self.record_fields,
            site,
            entry,
        );
    }

    fn method_dispatch(&self, site: CacheSiteId) -> Option<MethodInlineCacheEntry> {
        let entry = self.method_dispatches.get(site.index()).and_then(Cell::get);
        if entry.is_some() {
            self.record_hit(BenchCacheFamily::MethodDispatch);
        }
        entry
    }

    fn set_method_dispatch(&self, site: CacheSiteId, entry: MethodInlineCacheEntry) {
        self.record_copy_set(
            BenchCacheFamily::MethodDispatch,
            &self.method_dispatches,
            site,
            entry,
        );
    }

    fn native_call(&self, site: CacheSiteId) -> Option<NativeInlineCacheEntry> {
        let entry = self
            .native_calls
            .borrow()
            .get(site.index())
            .cloned()
            .flatten();
        if entry.is_some() {
            self.record_hit(BenchCacheFamily::NativeCall);
        }
        entry
    }

    fn set_native_call(&self, site: CacheSiteId, entry: NativeInlineCacheEntry) {
        self.record_native_set(site, entry);
    }
}

fn empty_cell_cache<T: Copy>(len: usize) -> Vec<Cell<Option<T>>> {
    (0..len).map(|_| Cell::new(None)).collect()
}

#[derive(Debug, Default)]
pub(crate) struct BenchBytecodeProfiler {
    hits: Cell<u64>,
}

impl BenchBytecodeProfiler {
    pub(crate) fn reset(&self) {
        self.hits.set(0);
    }

    pub(crate) fn hit_count(&self) -> u64 {
        self.hits.get()
    }
}

impl VmBytecodeProfiler for BenchBytecodeProfiler {
    fn record_instruction(&self, _function: DebugNameId, _offset: InstructionOffset) {
        self.hits.set(self.hits.get().saturating_add(1));
    }
}

pub(crate) fn rebase_linked_cache_sites(linked_program: &mut LinkedProgram, image: &ProgramImage) {
    let mut image_cache_sites_by_name = BTreeMap::<String, Vec<_>>::new();
    for (_, image_code) in image.functions() {
        image_cache_sites_by_name
            .entry(image_code.name.clone())
            .or_default()
            .push(image_code.cache_sites.clone());
    }

    let function_names = linked_program
        .functions()
        .map(|(_, code)| linked_program.debug_name(code.debug_name).to_owned())
        .collect::<Vec<_>>();
    for ((_, linked_code), function_name) in linked_program.functions_mut().zip(function_names) {
        let Some(image_cache_sites) = image_cache_sites_by_name
            .get_mut(&function_name)
            .and_then(|sites| (!sites.is_empty()).then(|| sites.remove(0)))
        else {
            continue;
        };
        let local_sites = linked_code.cache_sites.sites().to_vec();
        let image_sites = image_cache_sites.sites().to_vec();
        let mut remapped = vec![None; local_sites.len()];
        for (local, image) in local_sites.iter().zip(image_sites.iter()) {
            if let Some(slot) = remapped.get_mut(local.id.index()) {
                *slot = Some(image.id);
            }
        }
        rewrite_linked_instruction_cache_sites(linked_code, &remapped);
        linked_code.cache_sites = image_cache_sites;
    }
}

fn rewrite_linked_instruction_cache_sites(
    code: &mut LinkedCodeObject,
    remapped: &[Option<CacheSiteId>],
) {
    for instruction in &mut code.instructions {
        match &mut instruction.kind {
            InstructionKind::LoadGlobal {
                cache_site: Some(site),
                ..
            }
            | InstructionKind::CallNative {
                cache_site: Some(site),
                ..
            }
            | InstructionKind::GetRecordSlot {
                cache_site: Some(site),
                ..
            }
            | InstructionKind::SetRecordSlot {
                cache_site: Some(site),
                ..
            }
            | InstructionKind::CallMethod {
                cache_site: Some(site),
                ..
            } => remap_cache_site(site, remapped),
            InstructionKind::HostRead { cache_site, .. }
            | InstructionKind::HostWrite { cache_site, .. }
            | InstructionKind::HostMutate { cache_site, .. }
            | InstructionKind::HostRemove { cache_site, .. }
            | InstructionKind::HostCall { cache_site, .. } => {
                remap_cache_site(cache_site, remapped);
            }
            _ => {}
        }
    }
}

fn remap_cache_site(site: &mut CacheSiteId, remapped: &[Option<CacheSiteId>]) {
    if let Some(Some(rebased)) = remapped.get(site.index()) {
        *site = *rebased;
    }
}
