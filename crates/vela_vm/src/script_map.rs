use std::collections::BTreeMap;
use std::mem;

use crate::value_key::ValueKey;
use crate::{HeapExecution, Value, VmResult, stored_runtime_value};

#[derive(Clone, Debug, PartialEq)]
pub struct ScriptMap {
    entries: BTreeMap<ValueKey, MapEntry>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MapEntry {
    pub(crate) key: Value,
    pub(crate) value: Value,
}

impl ScriptMap {
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    pub(crate) fn from_entries(
        entries: impl IntoIterator<Item = (Value, Value)>,
        heap: Option<&HeapExecution<'_>>,
        operation: &'static str,
    ) -> VmResult<Self> {
        let mut map = Self::new();
        for (key, value) in entries {
            map.insert(key, value, heap, operation)?;
        }
        Ok(map)
    }

    #[must_use]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn keys(&self) -> impl ExactSizeIterator<Item = &Value> {
        self.entries.values().map(|entry| &entry.key)
    }

    pub(crate) fn values(&self) -> impl ExactSizeIterator<Item = &Value> {
        self.entries.values().map(|entry| &entry.value)
    }

    pub(crate) fn entries(&self) -> impl ExactSizeIterator<Item = &MapEntry> {
        self.entries.values()
    }

    pub(crate) fn key_order(&self) -> Vec<ValueKey> {
        self.entries.keys().cloned().collect()
    }

    pub(crate) fn entry_for_key(&self, key: &ValueKey) -> Option<&MapEntry> {
        self.entries.get(key)
    }

    #[must_use]
    pub(crate) fn contains_key(&self, key: &ValueKey) -> bool {
        self.entries.contains_key(key)
    }

    pub(crate) fn get_keyed(&self, key: &ValueKey) -> Option<Value> {
        self.entries
            .get(key)
            .map(|entry| stored_runtime_value(&entry.value))
    }

    pub(crate) fn contains_key_value(
        &self,
        key: &Value,
        heap: Option<&HeapExecution<'_>>,
        operation: &'static str,
    ) -> VmResult<bool> {
        let key = ValueKey::from_value(key, heap, operation)?;
        Ok(self.entries.contains_key(&key))
    }

    pub(crate) fn get(
        &self,
        key: &Value,
        heap: Option<&HeapExecution<'_>>,
        operation: &'static str,
    ) -> VmResult<Option<Value>> {
        let key = ValueKey::from_value(key, heap, operation)?;
        Ok(self
            .entries
            .get(&key)
            .map(|entry| stored_runtime_value(&entry.value)))
    }

    pub(crate) fn insert(
        &mut self,
        key: Value,
        value: Value,
        heap: Option<&HeapExecution<'_>>,
        operation: &'static str,
    ) -> VmResult<bool> {
        let value_key = ValueKey::from_value(&key, heap, operation)?;
        Ok(self.insert_keyed(value_key, key, value))
    }

    pub(crate) fn insert_keyed(&mut self, value_key: ValueKey, key: Value, value: Value) -> bool {
        match self.entries.get_mut(&value_key) {
            Some(entry) => {
                entry.value = value;
                false
            }
            None => {
                self.entries.insert(value_key, MapEntry { key, value });
                true
            }
        }
    }

    pub(crate) fn remove_keyed(&mut self, key: &ValueKey) -> Option<Value> {
        self.entries
            .remove(key)
            .map(|entry| stored_runtime_value(&entry.value))
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }

    pub(crate) fn values_vec(&self) -> Vec<Value> {
        self.values().copied().collect()
    }

    pub(crate) fn entries_vec(&self) -> Vec<(Value, Value)> {
        self.entries()
            .map(|entry| (entry.key, stored_runtime_value(&entry.value)))
            .collect()
    }

    #[must_use]
    pub(crate) fn shallow_size_bytes(&self) -> usize {
        mem::size_of::<Self>()
            + self
                .entries
                .keys()
                .map(|key| value_key_size_bytes(key) + mem::size_of::<MapEntry>())
                .sum::<usize>()
    }
}

impl Default for ScriptMap {
    fn default() -> Self {
        Self::new()
    }
}

fn value_key_size_bytes(key: &ValueKey) -> usize {
    key.payload_size_bytes()
}
