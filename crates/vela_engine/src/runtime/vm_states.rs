use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use vela_def::StateId;
use vela_vm::error::VmResult;
use vela_vm::heap::ScriptHeap;
use vela_vm::heap_execution::ActiveExecutionRoot;
use vela_vm::owned_value::OwnedValue;
use vela_vm::value::Value;
use vela_vm::{VmStateValues, persistent_value_to_owned, persistent_value_to_owned_with_slots};

use super::{RuntimeImageStorage, RuntimeImpl};

pub struct VelaValue {
    pub(super) runtime_id: u64,
    pub(super) value: Value,
    root_id: u64,
    roots: Arc<Mutex<RuntimeValueRoots>>,
}

impl VelaValue {
    pub(super) const fn runtime_id(&self) -> u64 {
        self.runtime_id
    }

    pub(super) const fn value(&self) -> Value {
        self.value
    }
}

impl Clone for VelaValue {
    fn clone(&self) -> Self {
        self.roots
            .lock()
            .expect("runtime value roots mutex poisoned")
            .clone_root(self.root_id);
        Self {
            runtime_id: self.runtime_id,
            value: self.value,
            root_id: self.root_id,
            roots: Arc::clone(&self.roots),
        }
    }
}

impl Drop for VelaValue {
    fn drop(&mut self) {
        self.roots
            .lock()
            .expect("runtime value roots mutex poisoned")
            .release(self.root_id);
    }
}

impl std::fmt::Debug for VelaValue {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VelaValue")
            .field("value", &self.value)
            .finish()
    }
}

impl PartialEq for VelaValue {
    fn eq(&self, other: &Self) -> bool {
        self.runtime_id == other.runtime_id && self.value == other.value
    }
}

pub trait IntoStateValue {
    fn set_state<I>(self, runtime: &mut RuntimeImpl<I>, name: String) -> VmResult<()>
    where
        I: RuntimeImageStorage;
}

#[cfg(not(feature = "serde"))]
impl<T> IntoStateValue for T
where
    T: Into<OwnedValue>,
{
    fn set_state<I>(self, runtime: &mut RuntimeImpl<I>, name: String) -> VmResult<()>
    where
        I: RuntimeImageStorage,
    {
        runtime.set_owned_state(name, self.into())
    }
}

#[cfg(feature = "serde")]
impl IntoStateValue for OwnedValue {
    fn set_state<I>(self, runtime: &mut RuntimeImpl<I>, name: String) -> VmResult<()>
    where
        I: RuntimeImageStorage,
    {
        runtime.set_owned_state(name, self)
    }
}

#[cfg(feature = "serde")]
macro_rules! impl_owned_state_value {
    ($($ty:ty),* $(,)?) => {
        $(
            impl IntoStateValue for $ty {
                fn set_state<I>(self, runtime: &mut RuntimeImpl<I>, name: String) -> VmResult<()>
                where
                    I: RuntimeImageStorage,
                {
                    runtime.set_owned_state(name, OwnedValue::from(self))
                }
            }
        )*
    };
}

#[cfg(feature = "serde")]
impl_owned_state_value!(bool, char, i32, i64, f64, String, vela_host::path::HostRef);

impl IntoStateValue for VelaValue {
    fn set_state<I>(self, runtime: &mut RuntimeImpl<I>, name: String) -> VmResult<()>
    where
        I: RuntimeImageStorage,
    {
        runtime.check_vela_value_runtime(&self)?;
        let value = persistent_value_to_owned_with_slots(
            &self.value,
            &mut runtime.state.vm_states.heap,
            &runtime.state.host_slots,
        )?;
        runtime.set_owned_state(name, value)
    }
}

impl IntoStateValue for &VelaValue {
    fn set_state<I>(self, runtime: &mut RuntimeImpl<I>, name: String) -> VmResult<()>
    where
        I: RuntimeImageStorage,
    {
        runtime.check_vela_value_runtime(self)?;
        let value = persistent_value_to_owned_with_slots(
            &self.value,
            &mut runtime.state.vm_states.heap,
            &runtime.state.host_slots,
        )?;
        runtime.set_owned_state(name, value)
    }
}

#[cfg(feature = "serde")]
impl<T> IntoStateValue for &T
where
    T: serde::Serialize + ?Sized,
{
    fn set_state<I>(self, runtime: &mut RuntimeImpl<I>, name: String) -> VmResult<()>
    where
        I: RuntimeImageStorage,
    {
        runtime.set_owned_state(name, vela_vm::serde::to_owned_value(self)?)
    }
}

#[derive(Debug, Default)]
pub(super) struct RuntimeValueRoots {
    next_id: u64,
    values: BTreeMap<u64, RuntimeValueRoot>,
    active_roots: BTreeMap<u64, ActiveExecutionRoot>,
}

#[derive(Debug)]
struct RuntimeValueRoot {
    value: Value,
    refs: usize,
}

impl RuntimeValueRoots {
    pub(super) fn retain(roots: &Arc<Mutex<Self>>, runtime_id: u64, value: Value) -> VelaValue {
        Self::retain_with_active(roots, runtime_id, value, None)
    }

    pub(super) fn retain_active(
        roots: &Arc<Mutex<Self>>,
        runtime_id: u64,
        value: Value,
        active_root: ActiveExecutionRoot,
    ) -> VelaValue {
        Self::retain_with_active(roots, runtime_id, value, Some(active_root))
    }

    fn retain_with_active(
        roots: &Arc<Mutex<Self>>,
        runtime_id: u64,
        value: Value,
        active_root: Option<ActiveExecutionRoot>,
    ) -> VelaValue {
        let mut roots_mut = roots.lock().expect("runtime value roots mutex poisoned");
        let root_id = roots_mut.next_id;
        roots_mut.next_id = roots_mut.next_id.saturating_add(1);
        roots_mut
            .values
            .insert(root_id, RuntimeValueRoot { value, refs: 1 });
        if let Some(active_root) = active_root {
            roots_mut.active_roots.insert(root_id, active_root);
        }
        drop(roots_mut);
        VelaValue {
            runtime_id,
            value,
            root_id,
            roots: Arc::clone(roots),
        }
    }

    fn clone_root(&mut self, root_id: u64) {
        if let Some(root) = self.values.get_mut(&root_id) {
            root.refs = root.refs.saturating_add(1);
        }
    }

    fn release(&mut self, root_id: u64) {
        let Some(root) = self.values.get_mut(&root_id) else {
            return;
        };
        root.refs = root.refs.saturating_sub(1);
        if root.refs == 0 {
            self.values.remove(&root_id);
            self.active_roots.remove(&root_id);
        }
    }

    fn values(&self) -> impl Iterator<Item = Value> + '_ {
        self.values.values().map(|root| root.value)
    }
}

#[derive(Debug, Default)]
pub struct RuntimeVmStateStore {
    pub(super) heap: ScriptHeap,
    pub(super) values: VmStateValues,
    pub(super) retained_values: Arc<Mutex<RuntimeValueRoots>>,
}

impl RuntimeVmStateStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub(super) fn insert_prepared(&mut self, state: StateId, value: Value) {
        self.values.insert(state, value);
    }

    #[cfg(test)]
    pub(super) fn prepare_value(&mut self, value: OwnedValue) -> VmResult<Value> {
        let mut budget = vela_vm::budget::ExecutionBudget::unbounded();
        vela_vm::owned_to_persistent_value(value, &mut self.heap, Some(&mut budget))
    }

    pub(super) fn persistent_value_by_id(&self, state: StateId) -> Option<Value> {
        self.values.get(state)
    }

    pub fn value(&mut self, state: StateId) -> VmResult<Option<OwnedValue>> {
        let Some(value) = self.values.get(state) else {
            return Ok(None);
        };
        persistent_value_to_owned(&value, &mut self.heap).map(Some)
    }

    #[cfg(feature = "serde")]
    pub fn value_as<T>(&self, state: StateId) -> VmResult<Option<T>>
    where
        T: serde::de::DeserializeOwned,
    {
        let Some(value) = self.values.get(state) else {
            return Ok(None);
        };
        vela_vm::serde::from_runtime_value(&value, &self.heap).map(Some)
    }

    pub(super) fn retain(&mut self, runtime_id: u64, value: Value) -> VelaValue {
        RuntimeValueRoots::retain(&self.retained_values, runtime_id, value)
    }

    pub(super) fn roots(&self) -> Vec<Value> {
        let mut roots = self.values.values().collect::<Vec<_>>();
        roots.extend(
            self.retained_values
                .lock()
                .expect("runtime value roots mutex poisoned")
                .values(),
        );
        roots
    }

    pub(super) fn retained_roots(&self) -> Vec<Value> {
        self.retained_values
            .lock()
            .expect("runtime value roots mutex poisoned")
            .values()
            .collect()
    }

    pub(super) fn state_roots(&self, states: &BTreeSet<StateId>) -> Vec<Value> {
        states
            .iter()
            .filter_map(|state| self.values.get(*state))
            .collect()
    }

    pub(super) fn retain_state_ids(&mut self, retained: &BTreeSet<StateId>) {
        self.values.retain(|state, _| retained.contains(&state));
        self.collect();
    }

    pub(super) fn collect(&mut self) {
        let mut roots = Vec::new();
        self.roots()
            .into_iter()
            .for_each(|value| value.trace_heap_refs(&mut roots));
        self.heap.collect_full(&roots);
    }
}
