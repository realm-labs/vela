use std::mem;

use crate::ordered_keyed::{LiveEntries, OrderedKeyed};
use crate::value_key::{KeyProbe, ValueKey};
use crate::{HeapExecution, Value, VmResult};

/// Script set with deterministic first-insertion iteration order.
///
/// Membership checks hash a borrowed [`KeyProbe`] against the stored keys, so
/// they clone no key payload; only inserting a new element copies the key out
/// of the heap.
#[derive(Clone, Debug)]
pub struct ScriptSet {
    entries: OrderedKeyed<Value>,
}

impl ScriptSet {
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            entries: OrderedKeyed::new(),
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
        self.entries.iter().map(|(_, value)| value)
    }

    #[cfg_attr(not(feature = "serde"), allow(dead_code))]
    pub(crate) fn iter_values(&self) -> SetValues<'_> {
        SetValues {
            entries: self.entries.iter(),
        }
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
        let probe = KeyProbe::from_value(value, heap, operation)?;
        Ok(self.entries.get_probe(&probe).is_some())
    }

    #[must_use]
    pub(crate) fn contains_key(&self, key: &ValueKey) -> bool {
        self.entries.get(key).is_some()
    }

    pub(crate) fn insert(
        &mut self,
        value: Value,
        heap: Option<&HeapExecution<'_>>,
        operation: &'static str,
    ) -> VmResult<bool> {
        let probe = KeyProbe::from_value(&value, heap, operation)?;
        if self.entries.get_probe(&probe).is_some() {
            return Ok(false);
        }
        Ok(self.entries.insert_probe(&probe, value))
    }

    pub(crate) fn insert_keyed(&mut self, key: ValueKey, value: Value) -> bool {
        if self.entries.get(&key).is_some() {
            return false;
        }
        self.entries.insert(key, value)
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
                .iter()
                .map(|(key, _)| key.payload_size_bytes() + mem::size_of::<Value>())
                .sum::<usize>()
    }
}

/// Content equality independent of insertion order, matching the equality the
/// previous sorted storage derived.
impl PartialEq for ScriptSet {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .entries
                .iter()
                .all(|(key, _)| other.entries.get(key).is_some())
    }
}

/// Insertion-ordered iterator over stored element values.
#[derive(Clone)]
pub(crate) struct SetValues<'a> {
    entries: LiveEntries<'a, Value>,
}

impl<'a> Iterator for SetValues<'a> {
    type Item = &'a Value;

    fn next(&mut self) -> Option<Self::Item> {
        self.entries.next().map(|(_, value)| value)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.entries.size_hint()
    }
}

impl ExactSizeIterator for SetValues<'_> {}

impl Default for ScriptSet {
    fn default() -> Self {
        Self::new()
    }
}
