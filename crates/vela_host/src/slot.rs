use smallvec::SmallVec;

use crate::path::{HostRef, HostSlotRef};

const INLINE_HOST_SLOTS: usize = 8;

/// Dense root-local metadata storage addressed by compact generational handles.
///
/// Copying a [`HostSlotRef`] does not touch this table. Removing an entry
/// invalidates every copied handle by advancing the slot generation before the
/// slot can be reused.
#[derive(Clone, Debug, PartialEq)]
pub struct HostSlotTable<T> {
    slots: SmallVec<[HostSlot<T>; INLINE_HOST_SLOTS]>,
    free: SmallVec<[u32; INLINE_HOST_SLOTS]>,
    len: usize,
}

#[derive(Clone, Debug, PartialEq)]
struct HostSlot<T> {
    generation: u32,
    metadata: Option<T>,
}

/// Canonical HostRef registry for one root execution.
///
/// Equal canonical references intern to one metadata entry, so every copied
/// script handle shares identity and generation validation. The linear intern
/// scan is confined to boundary admission; handle resolution remains O(1).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HostRefSlots {
    table: HostSlotTable<HostRef>,
}

impl HostRefSlots {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn intern(&mut self, reference: HostRef) -> HostSlotRef {
        if let Some((handle, _)) = self
            .table
            .iter()
            .find(|(_, current)| **current == reference)
        {
            return handle;
        }
        self.table.insert(reference)
    }

    #[must_use]
    pub fn resolve(&self, handle: HostSlotRef) -> Option<HostRef> {
        self.table.get(handle).copied()
    }

    pub fn release(&mut self, handle: HostSlotRef) -> Option<HostRef> {
        self.table.remove(handle)
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.table.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.table.is_empty()
    }

    #[must_use]
    pub fn spilled(&self) -> bool {
        self.table.spilled()
    }
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

    #[test]
    fn canonical_host_refs_intern_once_and_release_as_one_alias_group() {
        let mut slots = HostRefSlots::new();
        let reference = HostRef::new(
            vela_common::HostTypeId::new(7),
            vela_common::HostObjectId::new(11),
            3,
        );

        let handle = slots.intern(reference);
        let alias = slots.intern(reference);
        assert_eq!(alias, handle);
        assert_eq!(slots.len(), 1);
        assert_eq!(slots.resolve(alias), Some(reference));
        assert!(!slots.spilled());

        assert_eq!(slots.release(handle), Some(reference));
        assert_eq!(slots.resolve(alias), None);
        let replacement = slots.intern(reference);
        assert_eq!(replacement.slot(), handle.slot());
        assert_ne!(replacement.generation(), handle.generation());
    }

    #[test]
    fn canonical_host_ref_identity_keeps_exact_type_and_generation() {
        let mut slots = HostRefSlots::new();
        let player = HostRef::new(
            vela_common::HostTypeId::new(7),
            vela_common::HostObjectId::new(11),
            3,
        );
        let wrong_type = HostRef::new(
            vela_common::HostTypeId::new(8),
            player.object_id,
            player.generation,
        );
        let next_generation = HostRef::new(player.type_id, player.object_id, 4);

        let player_handle = slots.intern(player);
        let wrong_type_handle = slots.intern(wrong_type);
        let next_generation_handle = slots.intern(next_generation);

        assert_ne!(wrong_type_handle, player_handle);
        assert_ne!(next_generation_handle, player_handle);
        assert_eq!(slots.resolve(player_handle), Some(player));
        assert_eq!(slots.resolve(wrong_type_handle), Some(wrong_type));
        assert_eq!(slots.resolve(next_generation_handle), Some(next_generation));
    }
}
