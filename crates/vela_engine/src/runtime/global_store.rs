use std::collections::BTreeMap;

use vela_common::{HostObjectId, StateSlot};
use vela_host::adapter::GlobalBinding;
use vela_host::error::{HostError, HostErrorKind, HostResult};
use vela_host::object::ScriptHostObject;
use vela_host::path::HostRef;

const GLOBAL_HOST_OBJECT_ID_BASE: u64 = 1 << 62;

pub struct RuntimeGlobalStore {
    globals: BTreeMap<String, HostGlobalBinding>,
    slots: Vec<Option<HostRef>>,
    slot_by_name: BTreeMap<String, StateSlot>,
    next_host_object_id: u64,
}

impl Default for RuntimeGlobalStore {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeGlobalStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            globals: BTreeMap::new(),
            slots: Vec::new(),
            slot_by_name: BTreeMap::new(),
            next_host_object_id: GLOBAL_HOST_OBJECT_ID_BASE,
        }
    }

    #[must_use]
    pub fn with_state_layout(states: &[vela_bytecode::StateDescriptor]) -> Self {
        let mut store = Self::new();
        store.set_state_layout(states);
        store
    }

    pub fn set_state_layout(&mut self, states: &[vela_bytecode::StateDescriptor]) {
        self.slot_by_name.clear();
        self.slots.clear();
        self.slots.resize(states.len(), None);
        for (index, state) in states.iter().enumerate() {
            let name = &state.qualified_name;
            let slot = StateSlot::new(index);
            self.slot_by_name.insert(name.clone(), slot);
            if let Some(host_ref) = self.host_ref(name) {
                self.slots[index] = Some(host_ref);
            }
        }
    }

    pub fn insert_host<T>(&mut self, name: impl Into<String>, value: T) -> HostRef
    where
        T: ScriptHostObject + Send + 'static,
    {
        let name = name.into();
        let host_ref = HostRef::new(
            value.host_type_id(),
            HostObjectId::new(self.next_host_object_id),
            1,
        );
        self.next_host_object_id = self.next_host_object_id.saturating_add(1);
        if let Some(slot) = self.slot_by_name.get(&name).copied() {
            self.slots[slot.get()] = Some(host_ref);
        }
        self.globals.insert(
            name,
            HostGlobalBinding {
                host_ref,
                object: Box::new(value),
            },
        );
        host_ref
    }

    #[must_use]
    pub fn host_ref(&self, name: &str) -> Option<HostRef> {
        self.globals.get(name).map(|global| global.host_ref)
    }

    #[must_use]
    pub fn host_ref_by_slot(&self, slot: StateSlot) -> Option<HostRef> {
        self.slots.get(slot.get()).and_then(|host_ref| *host_ref)
    }

    pub(super) fn binding(&self, root: HostRef) -> Option<&HostGlobalBinding> {
        self.globals.values().find(|global| global.host_ref == root)
    }

    pub(super) fn binding_mut(&mut self, root: HostRef) -> Option<&mut HostGlobalBinding> {
        self.globals
            .values_mut()
            .find(|global| global.host_ref == root)
    }

    pub(super) fn binding_by_type(
        &self,
        type_id: vela_common::HostTypeId,
    ) -> Option<&HostGlobalBinding> {
        self.globals
            .values()
            .find(|global| global.host_ref.type_id == type_id)
    }

    pub(super) fn host_ref_for_binding(&self, global: GlobalBinding<'_>) -> HostResult<HostRef> {
        global
            .slot
            .and_then(|slot| self.host_ref_by_slot(slot))
            .or_else(|| self.host_ref(global.name))
            .ok_or_else(|| HostError {
                kind: HostErrorKind::MissingGlobal {
                    name: global.name.to_owned(),
                },
                source_span: None,
            })
    }
}

pub(super) struct HostGlobalBinding {
    pub(super) host_ref: HostRef,
    pub(super) object: Box<dyn ScriptHostObject + Send>,
}
