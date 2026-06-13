use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use vela_vm::budget::ExecutionBudget;
use vela_vm::error::VmResult;
use vela_vm::heap::ScriptHeap;
use vela_vm::owned_value::OwnedValue;
use vela_vm::value::Value;
use vela_vm::{ScriptGlobalValues, owned_to_persistent_value, persistent_value_to_owned};

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

pub trait IntoGlobalValue {
    fn insert_global<I>(self, runtime: &mut RuntimeImpl<I>, name: String) -> VmResult<()>
    where
        I: RuntimeImageStorage;
}

#[cfg(not(feature = "serde"))]
impl<T> IntoGlobalValue for T
where
    T: Into<OwnedValue>,
{
    fn insert_global<I>(self, runtime: &mut RuntimeImpl<I>, name: String) -> VmResult<()>
    where
        I: RuntimeImageStorage,
    {
        runtime.insert_owned_global(name, self.into())
    }
}

#[cfg(feature = "serde")]
impl IntoGlobalValue for OwnedValue {
    fn insert_global<I>(self, runtime: &mut RuntimeImpl<I>, name: String) -> VmResult<()>
    where
        I: RuntimeImageStorage,
    {
        runtime.insert_owned_global(name, self)
    }
}

#[cfg(feature = "serde")]
macro_rules! impl_owned_global_value {
    ($($ty:ty),* $(,)?) => {
        $(
            impl IntoGlobalValue for $ty {
                fn insert_global<I>(self, runtime: &mut RuntimeImpl<I>, name: String) -> VmResult<()>
                where
                    I: RuntimeImageStorage,
                {
                    runtime.insert_owned_global(name, OwnedValue::from(self))
                }
            }
        )*
    };
}

#[cfg(feature = "serde")]
impl_owned_global_value!(bool, char, i32, i64, f64, String, vela_host::path::HostRef);

impl IntoGlobalValue for VelaValue {
    fn insert_global<I>(self, runtime: &mut RuntimeImpl<I>, name: String) -> VmResult<()>
    where
        I: RuntimeImageStorage,
    {
        runtime.check_vela_value_runtime(&self)?;
        let value = persistent_value_to_owned(&self.value, &mut runtime.state.script_globals.heap)?;
        runtime.insert_owned_global(name, value)
    }
}

impl IntoGlobalValue for &VelaValue {
    fn insert_global<I>(self, runtime: &mut RuntimeImpl<I>, name: String) -> VmResult<()>
    where
        I: RuntimeImageStorage,
    {
        runtime.check_vela_value_runtime(self)?;
        let value = persistent_value_to_owned(&self.value, &mut runtime.state.script_globals.heap)?;
        runtime.insert_owned_global(name, value)
    }
}

#[cfg(feature = "serde")]
impl<T> IntoGlobalValue for &T
where
    T: serde::Serialize + ?Sized,
{
    fn insert_global<I>(self, runtime: &mut RuntimeImpl<I>, name: String) -> VmResult<()>
    where
        I: RuntimeImageStorage,
    {
        runtime.insert_owned_global(name, vela_vm::serde::to_owned_value(self)?)
    }
}

#[derive(Debug, Default)]
struct RuntimeValueRoots {
    next_id: u64,
    values: BTreeMap<u64, RuntimeValueRoot>,
}

#[derive(Debug)]
struct RuntimeValueRoot {
    value: Value,
    refs: usize,
}

impl RuntimeValueRoots {
    fn retain(roots: &Arc<Mutex<Self>>, runtime_id: u64, value: Value) -> VelaValue {
        let mut roots_mut = roots.lock().expect("runtime value roots mutex poisoned");
        let root_id = roots_mut.next_id;
        roots_mut.next_id = roots_mut.next_id.saturating_add(1);
        roots_mut
            .values
            .insert(root_id, RuntimeValueRoot { value, refs: 1 });
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
        }
    }

    fn values(&self) -> impl Iterator<Item = Value> + '_ {
        self.values.values().map(|root| root.value)
    }
}

#[derive(Debug, Default)]
pub struct RuntimeScriptGlobalStore {
    pub(super) heap: ScriptHeap,
    pub(super) values: ScriptGlobalValues,
    retained_values: Arc<Mutex<RuntimeValueRoots>>,
}

impl RuntimeScriptGlobalStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_global_layout(names: &[String]) -> Self {
        Self {
            heap: ScriptHeap::default(),
            values: ScriptGlobalValues::with_layout(names),
            retained_values: Arc::new(Mutex::new(RuntimeValueRoots::default())),
        }
    }

    pub fn set_global_layout(&mut self, names: &[String]) {
        self.values.set_layout(names);
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn insert(&mut self, name: impl Into<String>, value: OwnedValue) -> VmResult<()> {
        let mut budget = ExecutionBudget::unbounded();
        let value = owned_to_persistent_value(value, &mut self.heap, Some(&mut budget))?;
        self.values.insert(name.into(), value);
        self.collect();
        Ok(())
    }

    pub fn value(&mut self, name: &str) -> VmResult<Option<OwnedValue>> {
        let Some(value) = self.values.get(name) else {
            return Ok(None);
        };
        persistent_value_to_owned(&value, &mut self.heap).map(Some)
    }

    #[cfg(feature = "serde")]
    pub fn value_as<T>(&self, name: &str) -> VmResult<Option<T>>
    where
        T: serde::de::DeserializeOwned,
    {
        let Some(value) = self.values.get(name) else {
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

    fn collect(&mut self) {
        let mut roots = Vec::new();
        self.roots()
            .into_iter()
            .for_each(|value| value.trace_heap_refs(&mut roots));
        self.heap.collect_full(&roots);
    }
}
