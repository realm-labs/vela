use smallvec::SmallVec;

use crate::path::HostSlotRef;

const INLINE_HOST_SLOTS: usize = 8;

/// Dense root-local metadata storage addressed by compact generational handles.
///
/// Copying a [`HostSlotRef`] does not touch this table. Removing an entry
/// invalidates every copied handle by advancing the slot generation before the
/// slot can be reused.
pub struct HostSlotTable<T> {
    slots: SmallVec<[HostSlot<T>; INLINE_HOST_SLOTS]>,
    free: SmallVec<[u32; INLINE_HOST_SLOTS]>,
    len: usize,
}

struct HostSlot<T> {
    generation: u32,
    metadata: Option<T>,
}

impl<T> Default for HostSlotTable<T> {
    fn default() -> Self {
        Self {
            slots: SmallVec::new(),
            free: SmallVec::new(),
            len: 0,
        }
    }
}

impl<T> HostSlotTable<T> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn insert(&mut self, metadata: T) -> HostSlotRef {
        self.len = self.len.saturating_add(1);
        if let Some(slot) = self.free.pop() {
            let entry = &mut self.slots[slot as usize];
            debug_assert!(entry.metadata.is_none());
            entry.metadata = Some(metadata);
            return HostSlotRef::new(slot, entry.generation);
        }

        let slot = u32::try_from(self.slots.len())
            .expect("a root host-slot table cannot contain more than u32::MAX entries");
        self.slots.push(HostSlot {
            generation: 1,
            metadata: Some(metadata),
        });
        HostSlotRef::new(slot, 1)
    }

    #[must_use]
    pub fn get(&self, handle: HostSlotRef) -> Option<&T> {
        let entry = self.slots.get(handle.slot() as usize)?;
        (entry.generation == handle.generation())
            .then_some(entry.metadata.as_ref())
            .flatten()
    }

    #[must_use]
    pub fn get_mut(&mut self, handle: HostSlotRef) -> Option<&mut T> {
        let entry = self.slots.get_mut(handle.slot() as usize)?;
        (entry.generation == handle.generation())
            .then_some(entry.metadata.as_mut())
            .flatten()
    }

    pub fn remove(&mut self, handle: HostSlotRef) -> Option<T> {
        let entry = self.slots.get_mut(handle.slot() as usize)?;
        if entry.generation != handle.generation() {
            return None;
        }
        let metadata = entry.metadata.take()?;
        self.len -= 1;
        if entry.generation != u32::MAX {
            entry.generation += 1;
            self.free.push(handle.slot());
        }
        Some(metadata)
    }

    pub fn iter(&self) -> impl Iterator<Item = (HostSlotRef, &T)> {
        self.slots.iter().enumerate().filter_map(|(slot, entry)| {
            let metadata = entry.metadata.as_ref()?;
            Some((
                HostSlotRef::new(
                    u32::try_from(slot).expect("stored host-slot indexes fit u32"),
                    entry.generation,
                ),
                metadata,
            ))
        })
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[must_use]
    pub fn spilled(&self) -> bool {
        self.slots.spilled()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copied_handles_share_one_metadata_entry() {
        let mut table = HostSlotTable::new();
        let handle = table.insert(String::from("player"));
        let alias = handle;

        assert_eq!(table.get(handle).map(String::as_str), Some("player"));
        assert_eq!(table.get(alias).map(String::as_str), Some("player"));
        assert_eq!(table.len(), 1);
        assert!(!table.spilled());
    }

    #[test]
    fn removal_invalidates_aliases_before_slot_reuse() {
        let mut table = HostSlotTable::new();
        let handle = table.insert(10);
        let stale_alias = handle;

        assert_eq!(table.remove(handle), Some(10));
        assert_eq!(table.get(stale_alias), None);

        let replacement = table.insert(20);
        assert_eq!(replacement.slot(), stale_alias.slot());
        assert_ne!(replacement.generation(), stale_alias.generation());
        assert_eq!(table.get(stale_alias), None);
        assert_eq!(table.get(replacement), Some(&20));
    }

    #[test]
    fn common_arity_metadata_stays_inline() {
        let mut table = HostSlotTable::new();
        let handles = (0..INLINE_HOST_SLOTS)
            .map(|value| table.insert(value))
            .collect::<Vec<_>>();

        assert!(!table.spilled());
        assert_eq!(
            table.iter().map(|(_, value)| *value).collect::<Vec<_>>(),
            (0..INLINE_HOST_SLOTS).collect::<Vec<_>>()
        );
        assert!(
            handles
                .iter()
                .enumerate()
                .all(|(value, handle)| table.get(*handle) == Some(&value))
        );

        let _ = table.insert(INLINE_HOST_SLOTS);
        assert!(table.spilled());
    }
}
