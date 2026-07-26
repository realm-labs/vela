use std::mem;

use crate::ordered_keyed::OrderedKeyed;
use crate::value_key::{KeyProbe, ValueKey};
use crate::{HeapExecution, Value, VmResult, stored_runtime_value};

/// Script map with deterministic first-insertion iteration order.
///
/// Lookups hash a borrowed [`KeyProbe`] against the stored keys, so reading or
/// updating an existing entry clones no key payload; only inserting a new
/// entry copies the key out of the heap.
#[derive(Clone, Debug)]
pub struct ScriptMap {
    entries: OrderedKeyed<MapEntry>,
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
            entries: OrderedKeyed::new(),
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
        self.entries.iter().map(|(_, entry)| &entry.key)
    }

    pub(crate) fn values(&self) -> impl ExactSizeIterator<Item = &Value> {
        self.entries.iter().map(|(_, entry)| &entry.value)
    }

    pub(crate) fn entries(&self) -> impl ExactSizeIterator<Item = &MapEntry> {
        self.entries.iter().map(|(_, entry)| entry)
    }

    pub(crate) fn key_order(&self) -> Vec<ValueKey> {
        self.entries.iter().map(|(key, _)| key.clone()).collect()
    }

    pub(crate) fn entry_for_key(&self, key: &ValueKey) -> Option<&MapEntry> {
        self.entries.get(key)
    }

    #[must_use]
    pub(crate) fn contains_key(&self, key: &ValueKey) -> bool {
        self.entries.get(key).is_some()
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
        let probe = KeyProbe::from_value(key, heap, operation)?;
        Ok(self.entries.get_probe(&probe).is_some())
    }

    pub(crate) fn get(
        &self,
        key: &Value,
        heap: Option<&HeapExecution<'_>>,
        operation: &'static str,
    ) -> VmResult<Option<Value>> {
        let probe = KeyProbe::from_value(key, heap, operation)?;
        Ok(self
            .entries
            .get_probe(&probe)
            .map(|entry| stored_runtime_value(&entry.value)))
    }

    pub(crate) fn insert(
        &mut self,
        key: Value,
        value: Value,
        heap: Option<&HeapExecution<'_>>,
        operation: &'static str,
    ) -> VmResult<bool> {
        let probe = KeyProbe::from_value(&key, heap, operation)?;
        if let Some(entry) = self.entries.get_probe_mut(&probe) {
            entry.value = value;
            return Ok(false);
        }
        Ok(self.entries.insert_probe(&probe, MapEntry { key, value }))
    }

    pub(crate) fn insert_keyed(&mut self, value_key: ValueKey, key: Value, value: Value) -> bool {
        if let Some(entry) = self.entries.get_mut(&value_key) {
            entry.value = value;
            return false;
        }
        self.entries.insert(value_key, MapEntry { key, value })
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
                .iter()
                .map(|(key, _)| key.payload_size_bytes() + mem::size_of::<MapEntry>())
                .sum::<usize>()
    }
}

/// Content equality independent of insertion order.
///
/// The previous sorted storage made derived equality content-based; two maps
/// holding the same entries must stay equal even when they were built in
/// different orders.
impl PartialEq for ScriptMap {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .entries
                .iter()
                .all(|(key, entry)| other.entries.get(key) == Some(entry))
    }
}

impl Default for ScriptMap {
    fn default() -> Self {
        Self::new()
    }
}
