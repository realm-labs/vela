use std::collections::BTreeMap;

use vela_common::ShapeId;

#[derive(Clone, Debug, PartialEq)]
pub struct FieldSlot<T> {
    pub name: String,
    pub value: T,
}

impl<T> FieldSlot<T> {
    #[must_use]
    pub fn new(name: impl Into<String>, value: T) -> Self {
        Self {
            name: name.into(),
            value,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ScriptFields<T> {
    shape_id: ShapeId,
    slots: Vec<FieldSlot<T>>,
}

impl<T> ScriptFields<T> {
    #[must_use]
    pub fn from_pairs(owner: &str, fields: impl IntoIterator<Item = (String, T)>) -> Self {
        let fields = fields.into_iter().collect::<BTreeMap<_, _>>();
        let shape_id = shape_id(owner, fields.keys().map(String::as_str));
        Self {
            shape_id,
            slots: fields
                .into_iter()
                .map(|(name, value)| FieldSlot::new(name, value))
                .collect(),
        }
    }

    #[must_use]
    pub fn shape_id(&self) -> ShapeId {
        self.shape_id
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    #[must_use]
    pub fn get(&self, field: &str) -> Option<&T> {
        self.slots
            .iter()
            .find(|slot| slot.name == field)
            .map(|slot| &slot.value)
    }

    #[must_use]
    pub fn get_slot(&self, slot: usize, expected_field: &str) -> Option<&T> {
        let field = self.slots.get(slot)?;
        (field.name == expected_field).then_some(&field.value)
    }

    #[must_use]
    pub fn get_mut(&mut self, field: &str) -> Option<&mut T> {
        self.slots
            .iter_mut()
            .find(|slot| slot.name == field)
            .map(|slot| &mut slot.value)
    }

    #[must_use]
    pub fn get_slot_mut(&mut self, slot: usize, expected_field: &str) -> Option<&mut T> {
        let field = self.slots.get_mut(slot)?;
        (field.name == expected_field).then_some(&mut field.value)
    }

    #[must_use]
    pub fn contains_key(&self, field: &str) -> bool {
        self.get(field).is_some()
    }

    pub fn set_existing(&mut self, field: &str, value: T) -> Result<(), T> {
        let Some(slot) = self.get_mut(field) else {
            return Err(value);
        };
        *slot = value;
        Ok(())
    }

    pub fn set_slot_existing(
        &mut self,
        slot: usize,
        expected_field: &str,
        value: T,
    ) -> Result<(), T> {
        let Some(field) = self.get_slot_mut(slot, expected_field) else {
            return Err(value);
        };
        *field = value;
        Ok(())
    }

    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.slots.iter().map(|slot| &slot.value)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &T)> {
        self.slots
            .iter()
            .map(|slot| (slot.name.as_str(), &slot.value))
    }

    pub fn into_pairs(self) -> impl Iterator<Item = (String, T)> {
        self.slots.into_iter().map(|slot| (slot.name, slot.value))
    }
}

impl<T> From<BTreeMap<String, T>> for ScriptFields<T> {
    fn from(fields: BTreeMap<String, T>) -> Self {
        Self::from_pairs("", fields)
    }
}

impl<T, const N: usize> From<[(String, T); N]> for ScriptFields<T> {
    fn from(fields: [(String, T); N]) -> Self {
        Self::from_pairs("", fields)
    }
}

impl<T: PartialEq> PartialEq for ScriptFields<T> {
    fn eq(&self, other: &Self) -> bool {
        self.slots == other.slots
    }
}

fn shape_id<'a>(owner: &str, field_names: impl Iterator<Item = &'a str>) -> ShapeId {
    let mut hash = 0x811c_9dc5;
    hash_bytes(&mut hash, owner.as_bytes());
    hash_bytes(&mut hash, &[0]);
    for name in field_names {
        hash_bytes(&mut hash, name.as_bytes());
        hash_bytes(&mut hash, &[0]);
    }
    ShapeId::new(if hash == 0 { 1 } else { hash })
}

fn hash_bytes(hash: &mut u32, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u32::from(*byte);
        *hash = hash.wrapping_mul(0x0100_0193);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_slots_have_stable_shape_ids_across_source_order() {
        let first = ScriptFields::from_pairs(
            "Reward",
            [("count".to_owned(), 2), ("item_id".to_owned(), 1)],
        );
        let second = ScriptFields::from_pairs(
            "Reward",
            [("item_id".to_owned(), 1), ("count".to_owned(), 2)],
        );

        assert_eq!(first.shape_id(), second.shape_id());
        assert_eq!(
            first.iter().map(|(name, _)| name).collect::<Vec<_>>(),
            ["count", "item_id"]
        );
    }
}
