use std::collections::BTreeMap;
use std::sync::Arc;

use vela_common::{ShapeId, script_shape_id};

/// Shared description of one record or enum-variant shape.
///
/// Instances of the same shape share one `ShapeInfo` through an `Arc`, so a
/// record carries no per-instance field-name strings and no per-instance type
/// name. The `Arc` also keeps a shape alive exactly as long as any value using
/// it, which makes shapes safe across hot reload generations and persistent
/// state without any external table.
#[derive(Debug, PartialEq, Eq)]
pub struct ShapeInfo {
    shape_id: ShapeId,
    owner: String,
    field_names: Box<[String]>,
}

impl ShapeInfo {
    /// Builds a shape from an owner name and field names already in storage
    /// order.
    #[must_use]
    pub fn new(owner: impl Into<String>, field_names: impl Into<Box<[String]>>) -> Arc<Self> {
        let owner = owner.into();
        let field_names = field_names.into();
        let shape_id = script_shape_id(&owner, field_names.iter().map(String::as_str));
        Arc::new(Self {
            shape_id,
            owner,
            field_names,
        })
    }

    #[must_use]
    pub fn shape_id(&self) -> ShapeId {
        self.shape_id
    }

    /// The type name the shape was derived from.
    #[must_use]
    pub fn owner(&self) -> &str {
        &self.owner
    }

    #[must_use]
    pub fn field_names(&self) -> &[String] {
        &self.field_names
    }

    #[must_use]
    pub fn slot_of(&self, field: &str) -> Option<usize> {
        self.field_names.iter().position(|name| name == field)
    }
}

/// Content-addressed shape descriptors shared across a heap's instances.
///
/// Entries are never evicted; the table is bounded by the number of distinct
/// record and enum shapes ever constructed, each a handful of name strings.
/// The `Arc` keeps a shape valid for exactly as long as any value uses it, so
/// shapes survive hot-reload generations and persistent state without any
/// generation bookkeeping.
#[derive(Clone, Debug, Default)]
pub(crate) struct ShapeInterner {
    shapes: hashbrown::HashMap<ShapeId, Arc<ShapeInfo>>,
}

impl ShapeInterner {
    /// Interns one shape by content; `field_names` must be in storage order.
    ///
    /// A 32-bit shape-hash collision falls back to a fresh unshared
    /// descriptor instead of poisoning the table; correctness never depends
    /// on sharing.
    pub(crate) fn intern(&mut self, owner: &str, field_names: &[&str]) -> Arc<ShapeInfo> {
        let shape_id = script_shape_id(owner, field_names.iter().copied());
        if let Some(shape) = self.shapes.get(&shape_id) {
            if shape.owner() == owner
                && shape.field_names().len() == field_names.len()
                && shape
                    .field_names()
                    .iter()
                    .zip(field_names)
                    .all(|(stored, requested)| stored == requested)
            {
                return Arc::clone(shape);
            }
            return fresh_shape(owner, field_names);
        }
        let shape = fresh_shape(owner, field_names);
        self.shapes.insert(shape_id, Arc::clone(&shape));
        shape
    }
}

fn fresh_shape(owner: &str, field_names: &[&str]) -> Arc<ShapeInfo> {
    ShapeInfo::new(
        owner,
        field_names
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<Vec<_>>(),
    )
}

/// Field storage for records and enum payloads: one shared shape plus the
/// values in shape order.
#[derive(Clone, Debug)]
pub struct ScriptFields<T> {
    shape: Arc<ShapeInfo>,
    values: Vec<T>,
}

impl<T> ScriptFields<T> {
    /// Builds storage over an interned or freshly built shape. The value count
    /// must match the shape's field count.
    #[must_use]
    pub fn from_shape(shape: Arc<ShapeInfo>, values: Vec<T>) -> Self {
        debug_assert_eq!(shape.field_names.len(), values.len());
        Self { shape, values }
    }

    #[must_use]
    pub fn empty(owner: &str) -> Self {
        Self {
            shape: ShapeInfo::new(owner, Vec::new()),
            values: Vec::new(),
        }
    }

    #[must_use]
    pub fn single(owner: &str, name: impl Into<String>, value: T) -> Self {
        Self {
            shape: ShapeInfo::new(owner, vec![name.into()]),
            values: vec![value],
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
        Self::small(
            owner,
            [
                (first_name.into(), first_value),
                (second_name.into(), second_value),
            ],
        )
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
        Self::small(
            owner,
            [
                (first_name.into(), first_value),
                (second_name.into(), second_value),
                (third_name.into(), third_value),
            ],
        )
    }

    #[must_use]
    pub fn four(owner: &str, fields: [(String, T); 4]) -> Self {
        Self::small(owner, fields)
    }

    #[must_use]
    pub fn five(owner: &str, fields: [(String, T); 5]) -> Self {
        Self::small(owner, fields)
    }

    #[must_use]
    pub fn six(owner: &str, fields: [(String, T); 6]) -> Self {
        Self::small(owner, fields)
    }

    #[inline]
    fn small<const N: usize>(owner: &str, fields: [(String, T); N]) -> Self {
        if has_duplicate_field_names(&fields) {
            return Self::from_pairs(owner, fields);
        }
        let mut fields = Vec::from(fields);
        fields.sort_by(|left, right| left.0.cmp(&right.0));
        let (names, values): (Vec<String>, Vec<T>) = fields.into_iter().unzip();
        Self {
            shape: ShapeInfo::new(owner, names),
            values,
        }
    }

    #[must_use]
    pub fn from_pairs(owner: &str, fields: impl IntoIterator<Item = (String, T)>) -> Self {
        let fields = fields.into_iter().collect::<BTreeMap<_, _>>();
        let (names, values): (Vec<String>, Vec<T>) = fields.into_iter().unzip();
        Self {
            shape: ShapeInfo::new(owner, names),
            values,
        }
    }

    #[must_use]
    pub fn shape(&self) -> &Arc<ShapeInfo> {
        &self.shape
    }

    #[must_use]
    pub fn shape_id(&self) -> ShapeId {
        self.shape.shape_id
    }

    /// The type name the shape was derived from.
    #[must_use]
    pub fn owner_name(&self) -> &str {
        &self.shape.owner
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    #[must_use]
    pub fn get(&self, field: &str) -> Option<&T> {
        self.values.get(self.shape.slot_of(field)?)
    }

    #[must_use]
    #[inline]
    pub fn get_slot(&self, slot: usize, expected_field: &str) -> Option<&T> {
        let name = self.shape.field_names.get(slot)?;
        (name == expected_field).then(|| &self.values[slot])
    }

    #[must_use]
    #[inline]
    pub fn get_slot_at(&self, slot: usize) -> Option<&T> {
        self.values.get(slot)
    }

    #[must_use]
    pub fn get_mut(&mut self, field: &str) -> Option<&mut T> {
        let slot = self.shape.slot_of(field)?;
        self.values.get_mut(slot)
    }

    #[must_use]
    #[inline]
    pub fn get_slot_mut(&mut self, slot: usize, expected_field: &str) -> Option<&mut T> {
        let name = self.shape.field_names.get(slot)?;
        (name == expected_field).then(|| &mut self.values[slot])
    }

    #[must_use]
    pub fn contains_key(&self, field: &str) -> bool {
        self.shape.slot_of(field).is_some()
    }

    pub fn set_existing(&mut self, field: &str, value: T) -> Result<(), T> {
        let Some(slot) = self.get_mut(field) else {
            return Err(value);
        };
        *slot = value;
        Ok(())
    }

    #[inline]
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
        self.values.iter()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &T)> {
        self.shape
            .field_names
            .iter()
            .map(String::as_str)
            .zip(self.values.iter())
    }

    /// Yields owned pairs; names are cloned out of the shared shape.
    pub fn into_pairs(self) -> impl Iterator<Item = (String, T)> {
        Vec::from(self.shape.field_names.clone())
            .into_iter()
            .zip(self.values)
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

/// Field names and values decide equality, exactly as the former per-instance
/// slot storage did; the shape owner and its hash deliberately do not
/// participate. A shared shape allocation is a fast path, not a requirement.
impl<T: PartialEq> PartialEq for ScriptFields<T> {
    fn eq(&self, other: &Self) -> bool {
        if !Arc::ptr_eq(&self.shape, &other.shape)
            && self.shape.field_names != other.shape.field_names
        {
            return false;
        }
        self.values == other.values
    }
}

#[inline]
fn has_duplicate_field_names<T, const N: usize>(fields: &[(String, T); N]) -> bool {
    for left in 0..N {
        for right in (left + 1)..N {
            if fields[left].0 == fields[right].0 {
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
    fn slot_at_reads_sorted_slot_without_name_guard() {
        let fields = ScriptFields::three("Reward", "item_id", 1, "bonus", 3, "count", 2);

        assert_eq!(fields.get_slot(0, "bonus"), Some(&3));
        assert_eq!(fields.get_slot(0, "item_id"), None);
        assert_eq!(fields.get_slot_at(0), Some(&3));
        assert_eq!(fields.get_slot_at(3), None);
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
