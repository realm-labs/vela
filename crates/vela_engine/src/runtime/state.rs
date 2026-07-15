use std::collections::BTreeMap;
use std::sync::Weak;

use vela_bytecode::{
    CacheSiteId, DebugNameId, ExecutableGenerationId, InstructionOffset, LinkedProgram,
};
use vela_common::StateSlot;

use super::{
    RuntimeExternStateBindings, RuntimeVmStateStore, bytecode_profile::RuntimeBytecodeProfile,
    image::RuntimeImage, inline_cache::InlineCaches, next_runtime_id,
};

pub(super) struct RuntimeState {
    pub(super) id: u64,
    pub(super) extern_states: RuntimeExternStateBindings,
    pub(super) vm_states: RuntimeVmStateStore,
    pub(super) sidecars: RuntimeSidecars,
}

pub(super) struct RuntimeSidecars {
    active_generation: ExecutableGenerationId,
    generations: BTreeMap<ExecutableGenerationId, GenerationRuntimeState>,
}

struct GenerationRuntimeState {
    lifetime: Weak<()>,
    inline_caches: InlineCaches,
    bytecode_profile: RuntimeBytecodeProfile,
}

impl RuntimeState {
    pub(super) fn for_image(image: &RuntimeImage) -> Self {
        let active_generation = image.linked_program().generation();
        let mut generations = BTreeMap::new();
        generations.insert(active_generation, GenerationRuntimeState::for_image(image));
        Self {
            id: next_runtime_id(),
            extern_states: RuntimeExternStateBindings::with_state_layout(image.states()),
            vm_states: RuntimeVmStateStore::with_state_layout(image.states()),
            sidecars: RuntimeSidecars {
                active_generation,
                generations,
            },
        }
    }

    pub(super) fn set_state_layout(&mut self, states: &[vela_bytecode::StateDescriptor]) {
        self.extern_states.set_state_layout(states);
        self.vm_states.set_state_layout(states);
    }

    pub(super) fn rebind_to_image(&mut self, image: &RuntimeImage) {
        self.set_state_layout(image.states());
        let generation = image.linked_program().generation();
        self.sidecars
            .generations
            .entry(generation)
            .or_insert_with(|| GenerationRuntimeState::for_image(image));
        self.sidecars.active_generation = generation;
        self.sidecars.prune_dead_generations();
    }

    #[cfg(test)]
    pub(super) fn inline_caches(&self) -> &InlineCaches {
        &self.sidecars.active().inline_caches
    }

    #[cfg(test)]
    pub(super) fn bytecode_profile(&self) -> &RuntimeBytecodeProfile {
        &self.sidecars.active().bytecode_profile
    }
}

impl RuntimeSidecars {
    fn active(&self) -> &GenerationRuntimeState {
        self.generations
            .get(&self.active_generation)
            .expect("active runtime generation sidecar exists")
    }

    fn prune_dead_generations(&mut self) {
        let active = self.active_generation;
        self.generations
            .retain(|generation, state| *generation == active || state.lifetime.strong_count() > 0);
    }

    #[cfg(test)]
    fn generation_count(&self) -> usize {
        self.generations.len()
    }
}

impl GenerationRuntimeState {
    fn for_image(image: &RuntimeImage) -> Self {
        Self::for_program(image.linked_program())
    }

    fn for_program(program: &LinkedProgram) -> Self {
        Self {
            lifetime: program.lifetime_token(),
            inline_caches: InlineCaches::for_program(program),
            bytecode_profile: RuntimeBytecodeProfile::for_program(program),
        }
    }
}

impl vela_vm::VmInlineCaches for RuntimeSidecars {
    fn for_generation(
        &self,
        generation: ExecutableGenerationId,
    ) -> Option<&dyn vela_vm::VmInlineCaches> {
        Some(&self.generations.get(&generation)?.inline_caches)
    }

    fn len(&self) -> usize {
        self.active().inline_caches.len()
    }

    fn state_read_slot(&self, site: CacheSiteId) -> Option<StateSlot> {
        self.active().inline_caches.state_read_slot(site)
    }

    fn set_state_read_slot(&self, site: CacheSiteId, slot: StateSlot) {
        self.active().inline_caches.set_state_read_slot(site, slot);
    }

    fn host_access(&self, site: CacheSiteId) -> Option<vela_vm::HostInlineCacheEntry> {
        self.active().inline_caches.host_access(site)
    }

    fn set_host_access(&self, site: CacheSiteId, entry: vela_vm::HostInlineCacheEntry) {
        self.active().inline_caches.set_host_access(site, entry);
    }

    fn record_field(&self, site: CacheSiteId) -> Option<vela_vm::RecordFieldInlineCacheEntry> {
        self.active().inline_caches.record_field(site)
    }

    fn set_record_field(&self, site: CacheSiteId, entry: vela_vm::RecordFieldInlineCacheEntry) {
        self.active().inline_caches.set_record_field(site, entry);
    }

    fn method_dispatch(&self, site: CacheSiteId) -> Option<vela_vm::MethodInlineCacheEntry> {
        self.active().inline_caches.method_dispatch(site)
    }

    fn set_method_dispatch(&self, site: CacheSiteId, entry: vela_vm::MethodInlineCacheEntry) {
        self.active().inline_caches.set_method_dispatch(site, entry);
    }

    fn dynamic_method_dispatch(
        &self,
        site: CacheSiteId,
    ) -> Option<vela_vm::DynamicMethodInlineCacheEntry> {
        self.active().inline_caches.dynamic_method_dispatch(site)
    }

    fn set_dynamic_method_dispatch(
        &self,
        site: CacheSiteId,
        entry: vela_vm::DynamicMethodInlineCacheEntry,
    ) {
        self.active()
            .inline_caches
            .set_dynamic_method_dispatch(site, entry);
    }

    fn native_call(&self, site: CacheSiteId) -> Option<vela_vm::NativeInlineCacheEntry> {
        self.active().inline_caches.native_call(site)
    }

    fn set_native_call(&self, site: CacheSiteId, entry: vela_vm::NativeInlineCacheEntry) {
        self.active().inline_caches.set_native_call(site, entry);
    }
}

impl vela_vm::VmBytecodeProfiler for RuntimeSidecars {
    fn for_generation(
        &self,
        generation: ExecutableGenerationId,
    ) -> Option<&dyn vela_vm::VmBytecodeProfiler> {
        Some(&self.generations.get(&generation)?.bytecode_profile)
    }

    fn record_instruction(&self, function: DebugNameId, offset: InstructionOffset) {
        vela_vm::VmBytecodeProfiler::record_instruction(
            &self.active().bytecode_profile,
            function,
            offset,
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::engine::Engine;
    use crate::runtime::RuntimeImage;

    use super::RuntimeState;

    fn image(name: &str) -> RuntimeImage {
        let engine = Engine::builder().build().expect("engine should build");
        let program = engine
            .compile_source(&format!("fn {name}() {{ return 0; }}"))
            .expect("fixture compiles");
        RuntimeImage::new_compiled(engine, program)
    }

    #[test]
    fn generation_sidecars_are_weak_and_pruned_at_rebind_safe_points() {
        let old_image = image("old");
        let old_lifetime = old_image.linked_program().lifetime_token();
        let mut state = RuntimeState::for_image(&old_image);
        let new_image = image("new");

        state.rebind_to_image(&new_image);
        assert_eq!(state.sidecars.generation_count(), 2);
        drop(old_image);
        state.rebind_to_image(&new_image);

        assert!(old_lifetime.upgrade().is_none());
        assert_eq!(state.sidecars.generation_count(), 1);
    }
}
