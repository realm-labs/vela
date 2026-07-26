//! Insertion-ordered keyed storage shared by script maps and sets.
//!
//! Entries live in `slots` in first-insertion order; a raw hash table maps key
//! hashes to slot indexes, so the owned `ValueKey` is stored exactly once and
//! lookups run against borrowed [`KeyProbe`]s without cloning key payloads.
//! Removal tombstones a slot to preserve the order of the survivors and
//! compacts once tombstones outnumber live entries, keeping removal O(1)
//! amortized. Iteration reads `slots` directly and never depends on hash
//! order, so script-visible ordering stays deterministic.

use std::hash::BuildHasher;

use hashbrown::DefaultHashBuilder;
use hashbrown::hash_table::{Entry, HashTable};

use crate::value_key::{KeyProbe, ValueKey};

#[derive(Clone, Debug, Default)]
pub(crate) struct OrderedKeyed<T> {
    table: HashTable<usize>,
    slots: Vec<Option<(ValueKey, T)>>,
    live: usize,
    hasher: DefaultHashBuilder,
}

impl<T> OrderedKeyed<T> {
    pub(crate) fn new() -> Self {
        Self {
            table: HashTable::new(),
            slots: Vec::new(),
            live: 0,
            hasher: DefaultHashBuilder::default(),
        }
    }

    #[must_use]
    pub(crate) fn len(&self) -> usize {
        self.live
    }

    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        self.live == 0
    }

    pub(crate) fn clear(&mut self) {
        self.table.clear();
        self.slots.clear();
        self.live = 0;
    }

    /// Iterates live entries in first-insertion order.
    pub(crate) fn iter(&self) -> LiveEntries<'_, T> {
        LiveEntries {
            slots: self.slots.iter(),
            remaining: self.live,
        }
    }

    #[must_use]
    pub(crate) fn get(&self, key: &ValueKey) -> Option<&T> {
        let hash = self.hasher.hash_one(key);
        self.table
            .find(hash, |&slot| slot_key(&self.slots, slot) == key)
            .map(|&slot| &slot_entry(&self.slots, slot).1)
    }

    #[must_use]
    pub(crate) fn get_mut(&mut self, key: &ValueKey) -> Option<&mut T> {
        let hash = self.hasher.hash_one(key);
        let slots = &self.slots;
        let slot = *self
            .table
            .find(hash, |&slot| slot_key(slots, slot) == key)?;
        Some(&mut slot_entry_mut(&mut self.slots, slot).1)
    }

    #[must_use]
    pub(crate) fn get_probe_mut(&mut self, probe: &KeyProbe<'_>) -> Option<&mut T> {
        let hash = self.hasher.hash_one(probe);
        let slots = &self.slots;
        let slot = *self
            .table
            .find(hash, |&slot| probe.matches(slot_key(slots, slot)))?;
        Some(&mut slot_entry_mut(&mut self.slots, slot).1)
    }

    /// Finds the slot index a probe refers to.
    ///
    /// Slot indexes stay valid until the next removal, because removal is the
    /// only operation that tombstones or compacts. A caller may resolve a slot
    /// while the heap is only readable, drop the probe, and then mutate the
    /// entry by index, which is how map writes avoid cloning the key.
    #[must_use]
    pub(crate) fn slot_of_probe(&self, probe: &KeyProbe<'_>) -> Option<usize> {
        let hash = self.hasher.hash_one(probe);
        self.table
            .find(hash, |&slot| probe.matches(slot_key(&self.slots, slot)))
            .copied()
    }

    /// Mutable payload access by a slot index from [`Self::slot_of_probe`].
    #[must_use]
    pub(crate) fn payload_mut_at(&mut self, slot: usize) -> &mut T {
        &mut slot_entry_mut(&mut self.slots, slot).1
    }

    /// Removes the entry at a slot index from [`Self::slot_of_probe`].
    pub(crate) fn remove_at(&mut self, slot: usize) -> T {
        let hash = self.hasher.hash_one(slot_key(&self.slots, slot));
        if let Ok(entry) = self.table.find_entry(hash, |&other| other == slot) {
            entry.remove();
        }
        let (_, value) = self.slots[slot]
            .take()
            .expect("a resolved slot always references a live entry");
        self.live -= 1;
        self.maybe_compact();
        value
    }

    #[must_use]
    pub(crate) fn get_probe(&self, probe: &KeyProbe<'_>) -> Option<&T> {
        let hash = self.hasher.hash_one(probe);
        self.table
            .find(hash, |&slot| probe.matches(slot_key(&self.slots, slot)))
            .map(|&slot| &slot_entry(&self.slots, slot).1)
    }

    /// Inserts under an owned key, replacing only the payload when the key is
    /// already present so the first-inserted key value is retained. Returns
    /// `true` when a new entry was created.
    pub(crate) fn insert(&mut self, key: ValueKey, value: T) -> bool {
        let hash = self.hasher.hash_one(&key);
        let new_slot = self.slots.len();
        let slots = &self.slots;
        let hasher = &self.hasher;
        match self.table.entry(
            hash,
            |&slot| slot_key(slots, slot) == &key,
            |&slot| hasher.hash_one(slot_key(slots, slot)),
        ) {
            Entry::Occupied(entry) => {
                let slot = *entry.get();
                slot_entry_mut(&mut self.slots, slot).1 = value;
                false
            }
            Entry::Vacant(entry) => {
                entry.insert(new_slot);
                self.slots.push(Some((key, value)));
                self.live += 1;
                true
            }
        }
    }

    /// Inserts through a borrowed probe; the probe's payload is cloned into an
    /// owned key only when the entry is actually new.
    pub(crate) fn insert_probe(&mut self, probe: &KeyProbe<'_>, value: T) -> bool {
        let hash = self.hasher.hash_one(probe);
        let new_slot = self.slots.len();
        let slots = &self.slots;
        let hasher = &self.hasher;
        match self.table.entry(
            hash,
            |&slot| probe.matches(slot_key(slots, slot)),
            |&slot| hasher.hash_one(slot_key(slots, slot)),
        ) {
            Entry::Occupied(entry) => {
                let slot = *entry.get();
                slot_entry_mut(&mut self.slots, slot).1 = value;
                false
            }
            Entry::Vacant(entry) => {
                entry.insert(new_slot);
                self.slots.push(Some((probe.to_owned_key(), value)));
                self.live += 1;
                true
            }
        }
    }

    /// Removes by owned key, tombstoning the slot so survivor order holds.
    pub(crate) fn remove(&mut self, key: &ValueKey) -> Option<T> {
        let hash = self.hasher.hash_one(key);
        let slots = &self.slots;
        let slot = match self
            .table
            .find_entry(hash, |&slot| slot_key(slots, slot) == key)
        {
            Ok(entry) => entry.remove().0,
            Err(_) => return None,
        };
        let (_, value) = self.slots[slot]
            .take()
            .expect("a table slot always references a live entry");
        self.live -= 1;
        self.maybe_compact();
        Some(value)
    }

    /// Rebuilds dense storage once tombstones outnumber live entries.
    fn maybe_compact(&mut self) {
        if self.slots.len() < 16 || self.live * 2 >= self.slots.len() {
            return;
        }
        self.slots.retain(Option::is_some);
        let slots = &self.slots;
        let hasher = &self.hasher;
        let mut table = HashTable::with_capacity(slots.len());
        for (slot, entry) in slots.iter().enumerate() {
            let (key, _) = entry
                .as_ref()
                .expect("compaction retains only live entries");
            table.insert_unique(hasher.hash_one(key), slot, |&other| {
                hasher.hash_one(slot_key(slots, other))
            });
        }
        self.table = table;
    }
}

fn slot_key<T>(slots: &[Option<(ValueKey, T)>], slot: usize) -> &ValueKey {
    &slots[slot]
        .as_ref()
        .expect("a table slot always references a live entry")
        .0
}

fn slot_entry<T>(slots: &[Option<(ValueKey, T)>], slot: usize) -> &(ValueKey, T) {
    slots[slot]
        .as_ref()
        .expect("a table slot always references a live entry")
}

fn slot_entry_mut<T>(slots: &mut [Option<(ValueKey, T)>], slot: usize) -> &mut (ValueKey, T) {
    slots[slot]
        .as_mut()
        .expect("a table slot always references a live entry")
}

/// Live-entry iterator in insertion order with an exact length.
#[derive(Clone)]
pub(crate) struct LiveEntries<'a, T> {
    slots: std::slice::Iter<'a, Option<(ValueKey, T)>>,
    remaining: usize,
}

impl<'a, T> Iterator for LiveEntries<'a, T> {
    type Item = &'a (ValueKey, T);

    fn next(&mut self) -> Option<Self::Item> {
        for slot in self.slots.by_ref() {
            if let Some(entry) = slot.as_ref() {
                self.remaining -= 1;
                return Some(entry);
            }
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<T> ExactSizeIterator for LiveEntries<'_, T> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_and_owned_key_hash_identically() {
        let hasher = DefaultHashBuilder::default();
        let cases = [
            (ValueKey::Unit, KeyProbe::Unit),
            (ValueKey::Bool(true), KeyProbe::Bool(true)),
            (ValueKey::I64(-7), KeyProbe::I64(-7)),
            (ValueKey::U8(7), KeyProbe::U8(7)),
            (ValueKey::F64(7f64.to_bits()), KeyProbe::F64(7f64.to_bits())),
            (
                ValueKey::String("player".to_owned()),
                KeyProbe::String("player"),
            ),
            (ValueKey::Bytes(vec![1, 2, 3]), KeyProbe::Bytes(&[1, 2, 3])),
        ];
        for (key, probe) in &cases {
            assert_eq!(
                hasher.hash_one(key),
                hasher.hash_one(probe),
                "owned key and probe must agree for {key:?}"
            );
            assert!(probe.matches(key));
        }
    }

    #[test]
    fn iteration_keeps_first_insertion_order_across_removal() {
        let mut keyed = OrderedKeyed::new();
        for value in 0..20i64 {
            assert!(keyed.insert(ValueKey::I64(value), value));
        }
        assert!(!keyed.insert(ValueKey::I64(3), 30));
        for value in (0..20i64).step_by(2) {
            assert_eq!(keyed.remove(&ValueKey::I64(value)), Some(value));
        }
        let order = keyed.iter().map(|(_, value)| *value).collect::<Vec<_>>();
        let expected = (0..20i64)
            .filter(|value| value % 2 == 1)
            .map(|value| if value == 3 { 30 } else { value })
            .collect::<Vec<_>>();
        assert_eq!(order, expected);
        assert_eq!(keyed.len(), 10);
        assert_eq!(keyed.iter().len(), 10);
        assert_eq!(keyed.get(&ValueKey::I64(3)), Some(&30));
        assert_eq!(keyed.get(&ValueKey::I64(2)), None);
    }
}
