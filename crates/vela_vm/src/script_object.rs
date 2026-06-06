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
    pub fn empty(owner: &str) -> Self {
        Self {
            shape_id: shape_id(owner, std::iter::empty()),
            slots: Vec::new(),
        }
    }

    #[must_use]
    pub fn single(owner: &str, name: impl Into<String>, value: T) -> Self {
        let name = name.into();
        Self {
            shape_id: shape_id(owner, std::iter::once(name.as_str())),
            slots: vec![FieldSlot::new(name, value)],
        }
    }

    #[must_use]
    pub fn two(
        owner: &str,
        first_name: impl Into<String>,
        first_value: T,
        second_name: impl Into<String>,
        second_value: T,
    ) -> Self {
        let first_name = first_name.into();
        let second_name = second_name.into();
        if first_name == second_name {
            return Self::single(owner, second_name, second_value);
        }
        let (first_slot, second_slot) = if first_name < second_name {
            (
                FieldSlot::new(first_name, first_value),
                FieldSlot::new(second_name, second_value),
            )
        } else {
            (
                FieldSlot::new(second_name, second_value),
                FieldSlot::new(first_name, first_value),
            )
        };
        Self {
            shape_id: shape_id(
                owner,
                [first_slot.name.as_str(), second_slot.name.as_str()].into_iter(),
            ),
            slots: vec![first_slot, second_slot],
        }
    }

    #[must_use]
    pub fn three(
        owner: &str,
        first_name: impl Into<String>,
        first_value: T,
        second_name: impl Into<String>,
        second_value: T,
        third_name: impl Into<String>,
        third_value: T,
    ) -> Self {
        let first_name = first_name.into();
        let second_name = second_name.into();
        let third_name = third_name.into();
        if first_name == second_name || first_name == third_name || second_name == third_name {
            return Self::from_pairs(
                owner,
                [
                    (first_name, first_value),
                    (second_name, second_value),
                    (third_name, third_value),
                ],
            );
        }
        let mut slots = vec![
            FieldSlot::new(first_name, first_value),
            FieldSlot::new(second_name, second_value),
            FieldSlot::new(third_name, third_value),
        ];
        slots.sort_by(|left, right| left.name.cmp(&right.name));
        Self {
            shape_id: shape_id(owner, slots.iter().map(|slot| slot.name.as_str())),
            slots,
        }
    }

    #[must_use]
    pub fn four(owner: &str, fields: [(String, T); 4]) -> Self {
        let [
            (first_name, first_value),
            (second_name, second_value),
            (third_name, third_value),
            (fourth_name, fourth_value),
        ] = fields;
        if first_name == second_name
            || first_name == third_name
            || first_name == fourth_name
            || second_name == third_name
            || second_name == fourth_name
            || third_name == fourth_name
        {
            return Self::from_pairs(
                owner,
                [
                    (first_name, first_value),
                    (second_name, second_value),
                    (third_name, third_value),
                    (fourth_name, fourth_value),
                ],
            );
        }
        let mut slots = vec![
            FieldSlot::new(first_name, first_value),
            FieldSlot::new(second_name, second_value),
            FieldSlot::new(third_name, third_value),
            FieldSlot::new(fourth_name, fourth_value),
        ];
        slots.sort_by(|left, right| left.name.cmp(&right.name));
        Self {
            shape_id: shape_id(owner, slots.iter().map(|slot| slot.name.as_str())),
            slots,
        }
    }

    #[must_use]
    pub fn five(owner: &str, fields: [(String, T); 5]) -> Self {
        let [
            (first_name, first_value),
            (second_name, second_value),
            (third_name, third_value),
            (fourth_name, fourth_value),
            (fifth_name, fifth_value),
        ] = fields;
        if has_duplicate_names([
            first_name.as_str(),
            second_name.as_str(),
            third_name.as_str(),
            fourth_name.as_str(),
            fifth_name.as_str(),
        ]) {
            return Self::from_pairs(
                owner,
                [
                    (first_name, first_value),
                    (second_name, second_value),
                    (third_name, third_value),
                    (fourth_name, fourth_value),
                    (fifth_name, fifth_value),
                ],
            );
        }
        let mut slots = vec![
            FieldSlot::new(first_name, first_value),
            FieldSlot::new(second_name, second_value),
            FieldSlot::new(third_name, third_value),
            FieldSlot::new(fourth_name, fourth_value),
            FieldSlot::new(fifth_name, fifth_value),
        ];
        slots.sort_by(|left, right| left.name.cmp(&right.name));
        Self {
            shape_id: shape_id(owner, slots.iter().map(|slot| slot.name.as_str())),
            slots,
        }
    }

    #[must_use]
    pub fn six(owner: &str, fields: [(String, T); 6]) -> Self {
        let [
            (first_name, first_value),
            (second_name, second_value),
            (third_name, third_value),
            (fourth_name, fourth_value),
            (fifth_name, fifth_value),
            (sixth_name, sixth_value),
        ] = fields;
        if has_duplicate_names([
            first_name.as_str(),
            second_name.as_str(),
            third_name.as_str(),
            fourth_name.as_str(),
            fifth_name.as_str(),
            sixth_name.as_str(),
        ]) {
            return Self::from_pairs(
                owner,
                [
                    (first_name, first_value),
                    (second_name, second_value),
                    (third_name, third_value),
                    (fourth_name, fourth_value),
                    (fifth_name, fifth_value),
                    (sixth_name, sixth_value),
                ],
            );
        }
        let mut slots = vec![
            FieldSlot::new(first_name, first_value),
            FieldSlot::new(second_name, second_value),
            FieldSlot::new(third_name, third_value),
            FieldSlot::new(fourth_name, fourth_value),
            FieldSlot::new(fifth_name, fifth_value),
            FieldSlot::new(sixth_name, sixth_value),
        ];
        slots.sort_by(|left, right| left.name.cmp(&right.name));
        Self {
            shape_id: shape_id(owner, slots.iter().map(|slot| slot.name.as_str())),
            slots,
        }
    }

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

fn has_duplicate_names<const N: usize>(names: [&str; N]) -> bool {
    for left in 0..N {
        for right in (left + 1)..N {
            if names[left] == names[right] {
                return true;
            }
        }
    }
    false
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

    #[test]
    fn single_field_constructor_matches_pair_shape() {
        let from_pairs = ScriptFields::from_pairs("Option::Some", [("0".to_owned(), 7)]);
        let single = ScriptFields::single("Option::Some", "0", 7);

        assert_eq!(from_pairs.shape_id(), single.shape_id());
        assert_eq!(from_pairs, single);
    }

    #[test]
    fn empty_field_constructor_matches_empty_pair_shape() {
        let from_pairs = ScriptFields::<i32>::from_pairs("Option::None", []);
        let empty = ScriptFields::empty("Option::None");

        assert_eq!(from_pairs.shape_id(), empty.shape_id());
        assert_eq!(from_pairs, empty);
    }

    #[test]
    fn two_field_constructor_matches_pair_shape_and_order() {
        let from_pairs =
            ScriptFields::from_pairs("MapEntry", [("value".to_owned(), 8), ("key".to_owned(), 2)]);
        let two = ScriptFields::two("MapEntry", "value", 8, "key", 2);

        assert_eq!(from_pairs.shape_id(), two.shape_id());
        assert_eq!(from_pairs, two);
        assert_eq!(
            two.iter().map(|(name, _)| name).collect::<Vec<_>>(),
            ["key", "value"]
        );
    }

    #[test]
    fn two_field_constructor_matches_duplicate_pair_semantics() {
        let from_pairs = ScriptFields::from_pairs(
            "Duplicate",
            [("value".to_owned(), 1), ("value".to_owned(), 2)],
        );
        let two = ScriptFields::two("Duplicate", "value", 1, "value", 2);

        assert_eq!(from_pairs.shape_id(), two.shape_id());
        assert_eq!(from_pairs, two);
        assert_eq!(two.len(), 1);
        assert_eq!(two.get("value"), Some(&2));
    }

    #[test]
    fn three_field_constructor_matches_pair_shape_and_order() {
        let from_pairs = ScriptFields::from_pairs(
            "Reward",
            [
                ("item_id".to_owned(), 1),
                ("bonus".to_owned(), 3),
                ("count".to_owned(), 2),
            ],
        );
        let three = ScriptFields::three("Reward", "item_id", 1, "bonus", 3, "count", 2);

        assert_eq!(from_pairs.shape_id(), three.shape_id());
        assert_eq!(from_pairs, three);
        assert_eq!(
            three.iter().map(|(name, _)| name).collect::<Vec<_>>(),
            ["bonus", "count", "item_id"]
        );
    }

    #[test]
    fn three_field_constructor_matches_duplicate_pair_semantics() {
        let from_pairs = ScriptFields::from_pairs(
            "Duplicate",
            [
                ("left".to_owned(), 1),
                ("value".to_owned(), 2),
                ("value".to_owned(), 3),
            ],
        );
        let three = ScriptFields::three("Duplicate", "left", 1, "value", 2, "value", 3);

        assert_eq!(from_pairs.shape_id(), three.shape_id());
        assert_eq!(from_pairs, three);
        assert_eq!(three.len(), 2);
        assert_eq!(three.get("value"), Some(&3));
    }

    #[test]
    fn four_field_constructor_matches_pair_shape_and_order() {
        let from_pairs = ScriptFields::from_pairs(
            "Reward",
            [
                ("rarity".to_owned(), 4),
                ("item_id".to_owned(), 1),
                ("bonus".to_owned(), 3),
                ("count".to_owned(), 2),
            ],
        );
        let four = ScriptFields::four(
            "Reward",
            [
                ("rarity".to_owned(), 4),
                ("item_id".to_owned(), 1),
                ("bonus".to_owned(), 3),
                ("count".to_owned(), 2),
            ],
        );

        assert_eq!(from_pairs.shape_id(), four.shape_id());
        assert_eq!(from_pairs, four);
        assert_eq!(
            four.iter().map(|(name, _)| name).collect::<Vec<_>>(),
            ["bonus", "count", "item_id", "rarity"]
        );
    }

    #[test]
    fn four_field_constructor_matches_duplicate_pair_semantics() {
        let from_pairs = ScriptFields::from_pairs(
            "Duplicate",
            [
                ("left".to_owned(), 1),
                ("value".to_owned(), 2),
                ("right".to_owned(), 4),
                ("value".to_owned(), 3),
            ],
        );
        let four = ScriptFields::four(
            "Duplicate",
            [
                ("left".to_owned(), 1),
                ("value".to_owned(), 2),
                ("right".to_owned(), 4),
                ("value".to_owned(), 3),
            ],
        );

        assert_eq!(from_pairs.shape_id(), four.shape_id());
        assert_eq!(from_pairs, four);
        assert_eq!(four.len(), 3);
        assert_eq!(four.get("value"), Some(&3));
    }

    #[test]
    fn five_field_constructor_matches_pair_shape_and_order() {
        let from_pairs = ScriptFields::from_pairs(
            "Reward",
            [
                ("quality".to_owned(), 5),
                ("rarity".to_owned(), 4),
                ("item_id".to_owned(), 1),
                ("bonus".to_owned(), 3),
                ("count".to_owned(), 2),
            ],
        );
        let five = ScriptFields::five(
            "Reward",
            [
                ("quality".to_owned(), 5),
                ("rarity".to_owned(), 4),
                ("item_id".to_owned(), 1),
                ("bonus".to_owned(), 3),
                ("count".to_owned(), 2),
            ],
        );

        assert_eq!(from_pairs.shape_id(), five.shape_id());
        assert_eq!(from_pairs, five);
        assert_eq!(
            five.iter().map(|(name, _)| name).collect::<Vec<_>>(),
            ["bonus", "count", "item_id", "quality", "rarity"]
        );
    }

    #[test]
    fn five_field_constructor_matches_duplicate_pair_semantics() {
        let from_pairs = ScriptFields::from_pairs(
            "Duplicate",
            [
                ("left".to_owned(), 1),
                ("value".to_owned(), 2),
                ("right".to_owned(), 4),
                ("extra".to_owned(), 5),
                ("value".to_owned(), 3),
            ],
        );
        let five = ScriptFields::five(
            "Duplicate",
            [
                ("left".to_owned(), 1),
                ("value".to_owned(), 2),
                ("right".to_owned(), 4),
                ("extra".to_owned(), 5),
                ("value".to_owned(), 3),
            ],
        );

        assert_eq!(from_pairs.shape_id(), five.shape_id());
        assert_eq!(from_pairs, five);
        assert_eq!(five.len(), 4);
        assert_eq!(five.get("value"), Some(&3));
    }

    #[test]
    fn six_field_constructor_matches_pair_shape_and_order() {
        let from_pairs = ScriptFields::from_pairs(
            "Reward",
            [
                ("weight".to_owned(), 6),
                ("quality".to_owned(), 5),
                ("rarity".to_owned(), 4),
                ("item_id".to_owned(), 1),
                ("bonus".to_owned(), 3),
                ("count".to_owned(), 2),
            ],
        );
        let six = ScriptFields::six(
            "Reward",
            [
                ("weight".to_owned(), 6),
                ("quality".to_owned(), 5),
                ("rarity".to_owned(), 4),
                ("item_id".to_owned(), 1),
                ("bonus".to_owned(), 3),
                ("count".to_owned(), 2),
            ],
        );

        assert_eq!(from_pairs.shape_id(), six.shape_id());
        assert_eq!(from_pairs, six);
        assert_eq!(
            six.iter().map(|(name, _)| name).collect::<Vec<_>>(),
            ["bonus", "count", "item_id", "quality", "rarity", "weight"]
        );
    }

    #[test]
    fn six_field_constructor_matches_duplicate_pair_semantics() {
        let from_pairs = ScriptFields::from_pairs(
            "Duplicate",
            [
                ("left".to_owned(), 1),
                ("value".to_owned(), 2),
                ("right".to_owned(), 4),
                ("extra".to_owned(), 5),
                ("tail".to_owned(), 6),
                ("value".to_owned(), 3),
            ],
        );
        let six = ScriptFields::six(
            "Duplicate",
            [
                ("left".to_owned(), 1),
                ("value".to_owned(), 2),
                ("right".to_owned(), 4),
                ("extra".to_owned(), 5),
                ("tail".to_owned(), 6),
                ("value".to_owned(), 3),
            ],
        );

        assert_eq!(from_pairs.shape_id(), six.shape_id());
        assert_eq!(from_pairs, six);
        assert_eq!(six.len(), 5);
        assert_eq!(six.get("value"), Some(&3));
    }
}
