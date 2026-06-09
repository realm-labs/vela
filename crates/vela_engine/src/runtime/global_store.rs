use std::collections::BTreeMap;

use vela_common::{GlobalSlot, HostObjectId};
use vela_host::adapter::{GlobalBinding, ScriptStateAdapter};
use vela_host::error::{HostError, HostErrorKind, HostResult};
use vela_host::object::ScriptHostObject;
use vela_host::path::HostRef;
use vela_host::resolved::{HostAccessSpec, HostMutationOp, ResolvedHostAccess};
use vela_host::target::HostTargetInstance;
use vela_host::value::HostValue;

const GLOBAL_HOST_OBJECT_ID_BASE: u64 = 1 << 62;

pub struct RuntimeGlobalStore {
    globals: BTreeMap<String, HostGlobalBinding>,
    slots: Vec<Option<HostRef>>,
    slot_by_name: BTreeMap<String, GlobalSlot>,
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
    pub fn with_global_layout(names: &[String]) -> Self {
        let mut store = Self::new();
        store.set_global_layout(names);
        store
    }

    pub fn set_global_layout(&mut self, names: &[String]) {
        self.slot_by_name.clear();
        self.slots.clear();
        self.slots.resize(names.len(), None);
        for (index, name) in names.iter().enumerate() {
            let slot = GlobalSlot::new(index);
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
    pub fn host_ref_by_slot(&self, slot: GlobalSlot) -> Option<HostRef> {
        self.slots.get(slot.get()).and_then(|host_ref| *host_ref)
    }

    fn binding(&self, root: HostRef) -> Option<&HostGlobalBinding> {
        self.globals.values().find(|global| global.host_ref == root)
    }

    fn binding_mut(&mut self, root: HostRef) -> Option<&mut HostGlobalBinding> {
        self.globals
            .values_mut()
            .find(|global| global.host_ref == root)
    }

    fn binding_by_type(&self, type_id: vela_common::HostTypeId) -> Option<&HostGlobalBinding> {
        self.globals
            .values()
            .find(|global| global.host_ref.type_id == type_id)
    }
}

struct HostGlobalBinding {
    host_ref: HostRef,
    object: Box<dyn ScriptHostObject + Send>,
}

pub(super) struct GlobalStoreAdapter<'call> {
    globals: &'call mut RuntimeGlobalStore,
    fallback: &'call mut dyn ScriptStateAdapter,
}

impl<'call> GlobalStoreAdapter<'call> {
    pub(super) fn new(
        globals: &'call mut RuntimeGlobalStore,
        fallback: &'call mut dyn ScriptStateAdapter,
    ) -> Self {
        Self { globals, fallback }
    }
}

impl ScriptStateAdapter for GlobalStoreAdapter<'_> {
    fn global_ref(&self, global: GlobalBinding<'_>) -> HostResult<HostRef> {
        global
            .slot
            .and_then(|slot| self.globals.host_ref_by_slot(slot))
            .or_else(|| self.globals.host_ref(global.name))
            .or_else(|| self.fallback.global_ref(global).ok())
            .ok_or_else(|| HostError {
                kind: HostErrorKind::MissingGlobal {
                    name: global.name.to_owned(),
                },
                source_span: None,
            })
    }

    fn resolve_host_access(&self, spec: HostAccessSpec<'_>) -> HostResult<ResolvedHostAccess> {
        if let Some(global) = self.globals.binding_by_type(spec.plan.root_type) {
            return global.object.resolve_host_target(spec);
        }
        self.fallback.resolve_host_access(spec)
    }

    fn read_host(
        &self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<HostValue> {
        if let Some(global) = self.globals.binding(target.root) {
            return global.object.read_resolved_host(access, target);
        }
        self.fallback.read_host(access, target)
    }

    fn write_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        value: HostValue,
    ) -> HostResult<()> {
        if let Some(global) = self.globals.binding_mut(target.root) {
            return global.object.write_resolved_host(access, target, value);
        }
        self.fallback.write_host(access, target, value)
    }

    fn mutate_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        op: HostMutationOp,
        rhs: HostValue,
    ) -> HostResult<()> {
        if let Some(global) = self.globals.binding_mut(target.root) {
            return global.object.mutate_resolved_host(access, target, op, rhs);
        }
        self.fallback.mutate_host(access, target, op, rhs)
    }

    fn remove_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<()> {
        if let Some(global) = self.globals.binding_mut(target.root) {
            return global.object.remove_resolved_host(access, target);
        }
        self.fallback.remove_host(access, target)
    }

    fn call_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        method: vela_common::HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue> {
        if let Some(global) = self.globals.binding_mut(target.root) {
            return global
                .object
                .call_resolved_host(access, target, method, args);
        }
        self.fallback.call_host(access, target, method, args)
    }
}
