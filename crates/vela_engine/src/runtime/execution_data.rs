use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock, Weak};

use vela_bytecode::{ExecutableGenerationId, LinkedArtifact};

use super::bytecode_profile::GenerationBytecodeProfile;
use super::inline_cache::GenerationInlineCaches;

/// Mutable execution metadata qualified by one immutable executable generation.
///
/// The deployment Engine owns weak registry entries and Runtime generation
/// views retain this value while they can execute the matching artifact.
/// Ordinary construction leaves every optional data family unallocated.
pub(crate) struct GenerationExecutionData {
    generation: ExecutableGenerationId,
    artifact: Weak<LinkedArtifact>,
    inline_caches: GenerationInlineCaches,
    bytecode_profile: OnceLock<GenerationBytecodeProfile>,
}

impl GenerationExecutionData {
    fn new(artifact: &Arc<LinkedArtifact>) -> Self {
        Self {
            generation: artifact.generation(),
            artifact: Arc::downgrade(artifact),
            inline_caches: GenerationInlineCaches::for_layout(artifact.cache_layout()),
            bytecode_profile: OnceLock::new(),
        }
    }

    fn enable_bytecode_profile(&self) -> Option<&GenerationBytecodeProfile> {
        let artifact = self.artifact.upgrade()?;
        assert_eq!(
            artifact.generation(),
            self.generation,
            "execution data must use its owner generation's profile layout"
        );
        Some(
            self.bytecode_profile
                .get_or_init(|| GenerationBytecodeProfile::for_layout(artifact.profile_layout())),
        )
    }

    pub(crate) fn bytecode_profile(&self) -> Option<&GenerationBytecodeProfile> {
        self.bytecode_profile.get()
    }

    pub(super) fn inline_caches(&self) -> &GenerationInlineCaches {
        &self.inline_caches
    }

    #[cfg(test)]
    pub(crate) fn has_bytecode_profile(&self) -> bool {
        self.bytecode_profile.get().is_some()
    }
}

impl vela_vm::VmInlineCaches for GenerationExecutionData {
    fn for_generation(
        &self,
        generation: ExecutableGenerationId,
    ) -> Option<&dyn vela_vm::VmInlineCaches> {
        (generation == self.generation).then_some(self)
    }

    fn len(&self) -> usize {
        self.inline_caches.len()
    }

    fn host_access(
        &self,
        site: vela_bytecode::CacheSiteId,
    ) -> Option<vela_vm::HostInlineCacheEntry> {
        self.inline_caches.host_access(site)
    }

    fn set_host_access(
        &self,
        site: vela_bytecode::CacheSiteId,
        entry: vela_vm::HostInlineCacheEntry,
    ) {
        self.inline_caches.set_host_access(site, entry);
    }

    fn record_field(
        &self,
        site: vela_bytecode::CacheSiteId,
    ) -> Option<vela_vm::RecordFieldInlineCacheEntry> {
        self.inline_caches.record_field(site)
    }

    fn set_record_field(
        &self,
        site: vela_bytecode::CacheSiteId,
        entry: vela_vm::RecordFieldInlineCacheEntry,
    ) {
        self.inline_caches.set_record_field(site, entry);
    }

    fn method_dispatch(
        &self,
        site: vela_bytecode::CacheSiteId,
    ) -> Option<vela_vm::MethodInlineCacheEntry> {
        self.inline_caches.method_dispatch(site)
    }

    fn set_method_dispatch(
        &self,
        site: vela_bytecode::CacheSiteId,
        entry: vela_vm::MethodInlineCacheEntry,
    ) {
        self.inline_caches.set_method_dispatch(site, entry);
    }

    fn dynamic_method_dispatch(
        &self,
        site: vela_bytecode::CacheSiteId,
    ) -> Option<vela_vm::DynamicMethodInlineCacheEntry> {
        self.inline_caches.dynamic_method_dispatch(site)
    }

    fn set_dynamic_method_dispatch(
        &self,
        site: vela_bytecode::CacheSiteId,
        entry: vela_vm::DynamicMethodInlineCacheEntry,
    ) {
        self.inline_caches.set_dynamic_method_dispatch(site, entry);
    }

    fn native_call(
        &self,
        site: vela_bytecode::CacheSiteId,
    ) -> Option<vela_vm::NativeInlineCacheEntry> {
        self.inline_caches.native_call(site)
    }

    fn set_native_call(
        &self,
        site: vela_bytecode::CacheSiteId,
        entry: vela_vm::NativeInlineCacheEntry,
    ) {
        self.inline_caches.set_native_call(site, entry);
    }
}

impl vela_vm::VmBytecodeProfiler for GenerationExecutionData {
    fn for_generation(
        &self,
        generation: ExecutableGenerationId,
    ) -> Option<&dyn vela_vm::VmBytecodeProfiler> {
        (generation == self.generation && self.bytecode_profile().is_some()).then_some(self)
    }

    fn record_instruction(
        &self,
        function: vela_bytecode::DebugNameId,
        offset: vela_bytecode::InstructionOffset,
    ) {
        if let Some(profile) = self.bytecode_profile() {
            vela_vm::VmBytecodeProfiler::record_instruction(profile, function, offset);
        }
    }
}

pub(crate) type SharedGenerationExecutionData = Arc<GenerationExecutionData>;

pub(crate) type SharedGenerationExecutionRegistry = Arc<Mutex<GenerationExecutionRegistry>>;

pub(crate) struct GenerationExecutionRegistry {
    bytecode_profile_enabled: bool,
    generations: BTreeMap<ExecutableGenerationId, Weak<GenerationExecutionData>>,
}

impl GenerationExecutionRegistry {
    pub(crate) fn new() -> Self {
        Self {
            bytecode_profile_enabled: false,
            generations: BTreeMap::new(),
        }
    }

    pub(crate) fn data_for(
        &mut self,
        artifact: &Arc<LinkedArtifact>,
    ) -> SharedGenerationExecutionData {
        self.generations.retain(|_, data| data.strong_count() != 0);
        let generation = artifact.generation();
        let data = self
            .generations
            .get(&generation)
            .and_then(Weak::upgrade)
            .unwrap_or_else(|| {
                let data = Arc::new(GenerationExecutionData::new(artifact));
                self.generations.insert(generation, Arc::downgrade(&data));
                data
            });
        if self.bytecode_profile_enabled {
            let _ = data.enable_bytecode_profile();
        }
        data
    }

    pub(crate) fn enable_bytecode_profile(&mut self) {
        self.bytecode_profile_enabled = true;
        self.generations.retain(|_, data| {
            let Some(data) = data.upgrade() else {
                return false;
            };
            let _ = data.enable_bytecode_profile();
            true
        });
    }
}
