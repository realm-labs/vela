use std::collections::{BTreeMap, btree_map};
use std::mem;

use crate::value_key::ValueKey;
use crate::{HeapExecution, Value, VmResult};

#[derive(Clone, Debug, PartialEq)]
pub struct ScriptSet {
    entries: BTreeMap<ValueKey, Value>,
}

impl ScriptSet {
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    pub(crate) fn from_values(
        values: impl IntoIterator<Item = Value>,
        heap: Option<&HeapExecution<'_>>,
        operation: &'static str,
    ) -> VmResult<Self> {
        let mut set = Self::new();
        for value in values {
            set.insert(value, heap, operation)?;
        }
        Ok(set)
    }

    #[must_use]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn values(&self) -> impl ExactSizeIterator<Item = &Value> {
        self.entries.values()
    }

    #[cfg_attr(not(feature = "serde"), allow(dead_code))]
    pub(crate) fn iter_values(&self) -> btree_map::Values<'_, ValueKey, Value> {
        self.entries.values()
    }

    pub(crate) fn values_vec(&self) -> Vec<Value> {
        self.values().copied().collect()
    }

    pub(crate) fn contains_value(
        &self,
        value: &Value,
        heap: Option<&HeapExecution<'_>>,
        operation: &'static str,
    ) -> VmResult<bool> {
        let key = ValueKey::from_value(value, heap, operation)?;
        Ok(self.entries.contains_key(&key))
    }

    #[must_use]
    pub(crate) fn contains_key(&self, key: &ValueKey) -> bool {
        self.entries.contains_key(key)
    }

    pub(crate) fn insert(
        &mut self,
        value: Value,
        heap: Option<&HeapExecution<'_>>,
        operation: &'static str,
    ) -> VmResult<bool> {
        let key = ValueKey::from_value(&value, heap, operation)?;
        Ok(self.insert_keyed(key, value))
    }

    pub(crate) fn insert_keyed(&mut self, key: ValueKey, value: Value) -> bool {
        match self.entries.entry(key) {
            btree_map::Entry::Vacant(entry) => {
                entry.insert(value);
                true
            }
            btree_map::Entry::Occupied(_) => false,
        }
    }

    pub(crate) fn remove_keyed(&mut self, key: &ValueKey) -> bool {
        self.entries.remove(key).is_some()
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }

    #[must_use]
    pub(crate) fn shallow_size_bytes(&self) -> usize {
        mem::size_of::<Self>()
            + self
                .entries
                .keys()
                .map(|key| value_key_size_bytes(key) + mem::size_of::<Value>())
                .sum::<usize>()
    }
}

impl Default for ScriptSet {
    fn default() -> Self {
        Self::new()
    }
}

fn value_key_size_bytes(key: &ValueKey) -> usize {
    key.payload_size_bytes()
}
