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
    entries: RefCell<Vec<BenchInlineCacheEntry>>,
    hit_count: Cell<usize>,
    set_count: Cell<usize>,
}

#[derive(Clone, Debug, Default)]
enum BenchInlineCacheEntry {
    #[default]
    Empty,
    GlobalRead(GlobalSlot),
    HostAccess(HostInlineCacheEntry),
    RecordField(RecordFieldInlineCacheEntry),
    MethodDispatch(MethodInlineCacheEntry),
    NativeCall(NativeInlineCacheEntry),
}

impl BenchInlineCaches {
    pub(crate) fn new(len: usize) -> Self {
        Self {
            entries: RefCell::new(vec![BenchInlineCacheEntry::Empty; len]),
            hit_count: Cell::new(0),
            set_count: Cell::new(0),
        }
    }

    pub(crate) fn reset_measurement_counts(&self) {
        self.hit_count.set(0);
        self.set_count.set(0);
    }

    pub(crate) fn hit_count(&self) -> usize {
        self.hit_count.get()
    }

    pub(crate) fn set_count(&self) -> usize {
        self.set_count.get()
    }

    fn record_hit(&self) {
        self.hit_count.set(self.hit_count.get() + 1);
    }

    fn update(&self, site: CacheSiteId, entry: BenchInlineCacheEntry) {
        if let Some(slot) = self.entries.borrow_mut().get_mut(site.index()) {
            *slot = entry;
            self.set_count.set(self.set_count.get() + 1);
        }
    }
}

impl VmInlineCaches for BenchInlineCaches {
    fn len(&self) -> usize {
        self.entries.borrow().len()
    }

    fn global_read_slot(&self, site: CacheSiteId) -> Option<GlobalSlot> {
        let entry = match self.entries.borrow().get(site.index()) {
            Some(BenchInlineCacheEntry::GlobalRead(slot)) => Some(*slot),
            _ => None,
        };
        if entry.is_some() {
            self.record_hit();
        }
        entry
    }

    fn set_global_read_slot(&self, site: CacheSiteId, slot: GlobalSlot) {
        self.update(site, BenchInlineCacheEntry::GlobalRead(slot));
    }

    fn host_access(&self, site: CacheSiteId) -> Option<HostInlineCacheEntry> {
        let entry = match self.entries.borrow().get(site.index()) {
            Some(BenchInlineCacheEntry::HostAccess(entry)) => Some(*entry),
            _ => None,
        };
        if entry.is_some() {
            self.record_hit();
        }
        entry
    }

    fn set_host_access(&self, site: CacheSiteId, entry: HostInlineCacheEntry) {
        self.update(site, BenchInlineCacheEntry::HostAccess(entry));
    }

    fn record_field(&self, site: CacheSiteId) -> Option<RecordFieldInlineCacheEntry> {
        let entry = match self.entries.borrow().get(site.index()) {
            Some(BenchInlineCacheEntry::RecordField(entry)) => Some(*entry),
            _ => None,
        };
        if entry.is_some() {
            self.record_hit();
        }
        entry
    }

    fn set_record_field(&self, site: CacheSiteId, entry: RecordFieldInlineCacheEntry) {
        self.update(site, BenchInlineCacheEntry::RecordField(entry));
    }

    fn method_dispatch(&self, site: CacheSiteId) -> Option<MethodInlineCacheEntry> {
        let entry = match self.entries.borrow().get(site.index()) {
            Some(BenchInlineCacheEntry::MethodDispatch(entry)) => Some(*entry),
            _ => None,
        };
        if entry.is_some() {
            self.record_hit();
        }
        entry
    }

    fn set_method_dispatch(&self, site: CacheSiteId, entry: MethodInlineCacheEntry) {
        self.update(site, BenchInlineCacheEntry::MethodDispatch(entry));
    }

    fn native_call(&self, site: CacheSiteId) -> Option<NativeInlineCacheEntry> {
        let entry = match self.entries.borrow().get(site.index()) {
            Some(BenchInlineCacheEntry::NativeCall(entry)) => Some(entry.clone()),
            _ => None,
        };
        if entry.is_some() {
            self.record_hit();
        }
        entry
    }

    fn set_native_call(&self, site: CacheSiteId, entry: NativeInlineCacheEntry) {
        self.update(site, BenchInlineCacheEntry::NativeCall(entry));
    }
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
