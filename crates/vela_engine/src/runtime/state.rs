use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Weak};

use vela_bytecode::{
    CacheSiteId, DebugNameId, ExecutableGenerationId, InstructionOffset, LinkedArtifact,
    LinkedProgram,
};

use super::{
    RuntimeExternStateBindings, RuntimeVmStateStore, bytecode_profile::BytecodeProfileSnapshot,
    execution_data::SharedGenerationExecutionData, host_arena::RuntimeHostArena,
    image::RuntimeImage, next_runtime_id,
};

pub(super) struct RuntimeState {
    pub(super) id: u64,
    pub(super) extern_states: RuntimeExternStateBindings,
    pub(super) host_arena: RuntimeHostArena,
    pub(super) vm_states: RuntimeVmStateStore,
    pub(super) generations: RuntimeGenerations,
}

/// Actor-local lifetime view of adopted generations.
///
/// It owns no cache or profile arrays; entries retain only state ownership sets
/// and the shared, generation-qualified execution-data handle.
pub(super) struct RuntimeGenerations {
    active_generation: ExecutableGenerationId,
    entries: BTreeMap<ExecutableGenerationId, ActorGenerationState>,
}

struct ActorGenerationState {
    artifact: Weak<LinkedArtifact>,
    vm_states: BTreeSet<vela_def::StateId>,
    extern_states: BTreeSet<vela_def::StateId>,
    execution_data: SharedGenerationExecutionData,
}

impl RuntimeState {
    pub(super) fn for_image(image: &RuntimeImage) -> Self {
        let active_generation = image.linked_program().generation();
        let mut entries = BTreeMap::new();
        entries.insert(active_generation, ActorGenerationState::for_image(image));
        Self {
            id: next_runtime_id(),
            extern_states: RuntimeExternStateBindings::new(),
            host_arena: RuntimeHostArena::new(),
            vm_states: RuntimeVmStateStore::new(),
            generations: RuntimeGenerations {
                active_generation,
                entries,
            },
        }
    }

    pub(super) fn rebind_to_image(&mut self, image: &RuntimeImage) {
        let generation = image.linked_program().generation();
        self.generations
            .entries
            .entry(generation)
            .or_insert_with(|| ActorGenerationState::for_image(image));
        self.generations.active_generation = generation;
        self.reclaim_dead_generations();
    }

    pub(super) fn reclaim_dead_generations(&mut self) {
        self.vm_states.collect();
        self.generations.prune_dead_generations(&self.vm_states);
        let (vm_states, extern_states) = self.generations.retained_state_ids();
        self.vm_states.retain_state_ids(&vm_states);
        self.extern_states.retain_state_ids(&extern_states);
    }

    pub(super) fn bytecode_profile_snapshot(&self) -> Option<BytecodeProfileSnapshot> {
        let generation = self.generations.active_generation;
        self.generations
            .active()
            .execution_data
            .bytecode_profile()
            .map(|profile| profile.snapshot(generation))
    }

    pub(super) fn reset_bytecode_profile(&self) -> bool {
        let Some(profile) = self.generations.active().execution_data.bytecode_profile() else {
            return false;
        };
        profile.reset();
        true
    }

    #[cfg(test)]
    pub(super) fn retained_generation_count(&self) -> usize {
        self.generations.generation_count()
    }
}

impl RuntimeGenerations {
    fn active(&self) -> &ActorGenerationState {
        self.entries
            .get(&self.active_generation)
            .expect("active runtime generation sidecar exists")
    }

    fn prune_dead_generations(&mut self, vm_states: &RuntimeVmStateStore) {
        let mut live = BTreeSet::from([self.active_generation]);

        loop {
            let external_state_ids = live
                .iter()
                .filter_map(|generation| self.entries.get(generation))
                .flat_map(|state| state.vm_states.iter().copied())
                .collect::<BTreeSet<_>>();
            let inactive_state_ids = self
                .entries
                .iter()
                .filter(|(generation, _)| !live.contains(generation))
                .flat_map(|(_, state)| state.vm_states.iter().copied())
                .collect::<BTreeSet<_>>();
            let mut external_roots = vm_states.retained_roots();
            external_roots.extend(vm_states.state_roots(&external_state_ids));
            let internal_roots = vm_states.state_roots(&inactive_state_ids);
            let internal_owners = vm_states
                .heap
                .linked_owner_counts_exclusive_to_roots(&internal_roots, &external_roots);

            let newly_live = self
                .entries
                .iter()
                .filter(|(generation, _)| !live.contains(generation))
                .filter_map(|(generation, state)| {
                    let internal = internal_owners.get(generation).copied().unwrap_or(0);
                    (state.artifact.strong_count() > internal).then_some(*generation)
                })
                .collect::<Vec<_>>();
            if newly_live.is_empty() {
                break;
            }
            live.extend(newly_live);
        }

        self.entries
            .retain(|generation, _| live.contains(generation));
    }

    fn retained_state_ids(&self) -> (BTreeSet<vela_def::StateId>, BTreeSet<vela_def::StateId>) {
        let mut vm_states = BTreeSet::new();
        let mut extern_states = BTreeSet::new();
        for state in self.entries.values() {
            vm_states.extend(state.vm_states.iter().copied());
            extern_states.extend(state.extern_states.iter().copied());
        }
        (vm_states, extern_states)
    }

    #[cfg(test)]
    fn generation_count(&self) -> usize {
        self.entries.len()
    }
}

impl ActorGenerationState {
    fn for_image(image: &RuntimeImage) -> Self {
        Self::for_program(
            image.linked_program(),
            image.linked_artifact(),
            image.execution_data(),
        )
    }

    fn for_program(
        program: &LinkedProgram,
        artifact: &Arc<LinkedArtifact>,
        execution_data: &SharedGenerationExecutionData,
    ) -> Self {
        Self {
            artifact: Arc::downgrade(artifact),
            vm_states: program
                .states()
                .iter()
                .filter(|state| state.storage == vela_bytecode::StateStorage::Vm)
                .map(|state| state.id)
                .collect(),
            extern_states: program
                .states()
                .iter()
                .filter(|state| state.storage == vela_bytecode::StateStorage::Extern)
                .map(|state| state.id)
                .collect(),
            execution_data: Arc::clone(execution_data),
        }
    }
}

impl vela_vm::VmInlineCaches for RuntimeGenerations {
    fn for_generation(
        &self,
        generation: ExecutableGenerationId,
    ) -> Option<&dyn vela_vm::VmInlineCaches> {
        Some(self.entries.get(&generation)?.execution_data.as_ref())
    }

    fn len(&self) -> usize {
        self.active().execution_data.inline_caches().len()
    }

    fn host_access(&self, site: CacheSiteId) -> Option<vela_vm::HostInlineCacheEntry> {
        vela_vm::VmInlineCaches::host_access(self.active().execution_data.as_ref(), site)
    }

    fn set_host_access(&self, site: CacheSiteId, entry: vela_vm::HostInlineCacheEntry) {
        vela_vm::VmInlineCaches::set_host_access(
            self.active().execution_data.as_ref(),
            site,
            entry,
        );
    }

    fn record_field(&self, site: CacheSiteId) -> Option<vela_vm::RecordFieldInlineCacheEntry> {
        vela_vm::VmInlineCaches::record_field(self.active().execution_data.as_ref(), site)
    }

    fn set_record_field(&self, site: CacheSiteId, entry: vela_vm::RecordFieldInlineCacheEntry) {
        vela_vm::VmInlineCaches::set_record_field(
            self.active().execution_data.as_ref(),
            site,
            entry,
        );
    }

    fn method_dispatch(&self, site: CacheSiteId) -> Option<vela_vm::MethodInlineCacheEntry> {
        vela_vm::VmInlineCaches::method_dispatch(self.active().execution_data.as_ref(), site)
    }

    fn set_method_dispatch(&self, site: CacheSiteId, entry: vela_vm::MethodInlineCacheEntry) {
        vela_vm::VmInlineCaches::set_method_dispatch(
            self.active().execution_data.as_ref(),
            site,
            entry,
        );
    }

    fn dynamic_method_dispatch(
        &self,
        site: CacheSiteId,
    ) -> Option<vela_vm::DynamicMethodInlineCacheEntry> {
        vela_vm::VmInlineCaches::dynamic_method_dispatch(
            self.active().execution_data.as_ref(),
            site,
        )
    }

    fn set_dynamic_method_dispatch(
        &self,
        site: CacheSiteId,
        entry: vela_vm::DynamicMethodInlineCacheEntry,
    ) {
        vela_vm::VmInlineCaches::set_dynamic_method_dispatch(
            self.active().execution_data.as_ref(),
            site,
            entry,
        );
    }

    fn native_call(&self, site: CacheSiteId) -> Option<vela_vm::NativeInlineCacheEntry> {
        vela_vm::VmInlineCaches::native_call(self.active().execution_data.as_ref(), site)
    }

    fn set_native_call(&self, site: CacheSiteId, entry: vela_vm::NativeInlineCacheEntry) {
        vela_vm::VmInlineCaches::set_native_call(
            self.active().execution_data.as_ref(),
            site,
            entry,
        );
    }
}

impl vela_vm::VmBytecodeProfiler for RuntimeGenerations {
    fn for_generation(
        &self,
        generation: ExecutableGenerationId,
    ) -> Option<&dyn vela_vm::VmBytecodeProfiler> {
        let data = self.entries.get(&generation)?.execution_data.as_ref();
        data.bytecode_profile()
            .map(|_| data as &dyn vela_vm::VmBytecodeProfiler)
    }

    fn record_instruction(&self, function: DebugNameId, offset: InstructionOffset) {
        vela_vm::VmBytecodeProfiler::record_instruction(
            self.active().execution_data.as_ref(),
            function,
            offset,
        );
    }
}

impl RuntimeGenerations {
    pub(super) fn bytecode_profiler(&self) -> Option<&dyn vela_vm::VmBytecodeProfiler> {
        self.active().execution_data.bytecode_profile()?;
        Some(self)
    }

    #[cfg(test)]
    pub(super) fn active_bytecode_profile(
        &self,
    ) -> Option<&super::bytecode_profile::GenerationBytecodeProfile> {
        self.active().execution_data.bytecode_profile()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

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
    fn actor_generation_entries_are_weak_and_pruned_at_later_safe_points() {
        let old_image = image("old");
        let old_lifetime = old_image.linked_program().lifetime_token();
        let mut state = RuntimeState::for_image(&old_image);
        let new_image = image("new");

        state.rebind_to_image(&new_image);
        assert_eq!(state.generations.generation_count(), 2);
        drop(old_image);
        state.reclaim_dead_generations();

        assert!(old_lifetime.upgrade().is_none());
        assert_eq!(state.generations.generation_count(), 1);
    }

    #[test]
    fn actor_generation_state_retains_shared_execution_data_without_cache_arrays() {
        let engine = Engine::builder().build().expect("engine should build");
        let mut source = String::new();
        for index in 0..32 {
            source.push_str(&format!(
                "struct Item{index} {{ value: i64 }} fn read_{index}(item: Item{index}) {{ return item.value; }}\n"
            ));
        }
        let program = engine
            .compile_source(&source)
            .expect("large cache fixture should compile");
        let artifact = engine
            .link_compiled_program(program)
            .expect("large cache fixture should link");
        let first_image = RuntimeImage::from_linked_artifact(engine.clone(), artifact.clone());
        let second_image = RuntimeImage::from_linked_artifact(engine, artifact);
        let first = RuntimeState::for_image(&first_image);
        let second = RuntimeState::for_image(&second_image);
        let first_generation = first.generations.active();
        let second_generation = second.generations.active();

        assert!(Arc::ptr_eq(
            &first_generation.execution_data,
            &second_generation.execution_data
        ));
        assert_eq!(first_generation.vm_states.len(), 0);
        assert_eq!(first_generation.extern_states.len(), 0);
        assert_eq!(
            first_generation.execution_data.inline_caches().len(),
            first_image.cache_site_count()
        );
        assert!(!first_generation.execution_data.has_bytecode_profile());
    }

    #[test]
    fn removed_vm_state_is_retained_until_old_generation_lifetime_expires() {
        let engine = Engine::builder().build().expect("engine should build");
        let old_program = engine
            .compile_source("state retired: i64 = 5; fn read() { return retired; }")
            .expect("old fixture compiles");
        let old_image = RuntimeImage::new_compiled(engine.clone(), old_program);
        let state_id = old_image.states()[0].id;
        let old_lifetime = old_image.linked_program().lifetime_token();
        let mut state = RuntimeState::for_image(&old_image);
        let value = state
            .vm_states
            .prepare_value(vela_vm::owned_value::OwnedValue::from(5_i64))
            .expect("persistent state value");
        state.vm_states.insert_prepared(state_id, value);
        let new_program = engine
            .compile_source("fn read() { return 0; }")
            .expect("new fixture compiles");
        let new_image = RuntimeImage::new_compiled(engine, new_program);

        state.rebind_to_image(&new_image);
        assert!(state.vm_states.values.get(state_id).is_some());
        drop(old_image);
        state.reclaim_dead_generations();

        assert!(old_lifetime.upgrade().is_none());
        assert!(state.vm_states.values.get(state_id).is_none());
    }
}
