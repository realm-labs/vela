use std::collections::BTreeMap;

use crate::value_key::ValueKey;
use crate::{ExecutionBudget, HeapExecution, Value, VmResult, array_methods, map_methods};

use super::state::{GroupEntries, GroupValues};

pub(super) fn accept_value(
    groups: &mut BTreeMap<ValueKey, GroupValues>,
    awaiting: &mut Option<Value>,
    returned: Value,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<()> {
    let value = awaiting.take().ok_or_else(super::incomplete_callback)?;
    let key = ValueKey::from_value(&returned, heap, "method group_by")?;
    match groups.entry(key) {
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(GroupValues {
                key: returned,
                values: vec![value],
            });
        }
        std::collections::btree_map::Entry::Occupied(mut entry) => {
            entry.get_mut().values.push(value);
        }
    }
    Ok(())
}

pub(super) fn accept_entry(
    groups: &mut BTreeMap<ValueKey, GroupEntries>,
    awaiting: &mut Option<(Value, Value)>,
    returned: Value,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<()> {
    let entry = awaiting.take().ok_or_else(super::incomplete_callback)?;
    let key = ValueKey::from_value(&returned, heap, "method group_by")?;
    match groups.entry(key) {
        std::collections::btree_map::Entry::Vacant(group) => {
            group.insert(GroupEntries {
                key: returned,
                entries: vec![entry],
            });
        }
        std::collections::btree_map::Entry::Occupied(mut group) => {
            group.get_mut().entries.push(entry);
        }
    }
    Ok(())
}

pub(super) fn finish_values(
    groups: BTreeMap<ValueKey, GroupValues>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<Value> {
    let mut entries = Vec::with_capacity(groups.len());
    for group in groups.into_values() {
        let values = array_methods::make_array_value(group.values, heap, budget, operation)?;
        entries.push((group.key, values));
    }
    map_methods::make_map_from_entries(entries, heap, budget, operation)
}

pub(super) fn finish_entries(
    groups: BTreeMap<ValueKey, GroupEntries>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<Value> {
    let mut entries = Vec::with_capacity(groups.len());
    for group in groups.into_values() {
        let values = map_methods::make_map_from_entries(group.entries, heap, budget, operation)?;
        entries.push((group.key, values));
    }
    map_methods::make_map_from_entries(entries, heap, budget, operation)
}

pub(super) fn protect_values(
    values: &[Value],
    host_sequence: Option<&crate::iteration::IteratorState>,
    groups: &BTreeMap<ValueKey, GroupValues>,
    awaiting: Option<&Value>,
    heap: &mut HeapExecution<'_>,
) {
    heap.protect_values(values);
    if let Some(iterator) = host_sequence {
        heap.protect_values(&iterator.protected_values());
    }
    for group in groups.values() {
        heap.protect_values(&[group.key]);
        heap.protect_values(&group.values);
    }
    if let Some(value) = awaiting {
        heap.protect_values(&[*value]);
    }
}

pub(super) fn protect_entries(
    entries: &[(Value, Value)],
    host_sequence: Option<&crate::iteration::IteratorState>,
    groups: &BTreeMap<ValueKey, GroupEntries>,
    awaiting: Option<&(Value, Value)>,
    heap: &mut HeapExecution<'_>,
) {
    if let Some(iterator) = host_sequence {
        heap.protect_values(&iterator.protected_values());
    }
    for (key, value) in entries {
        heap.protect_values(&[*key, *value]);
    }
    for group in groups.values() {
        heap.protect_values(&[group.key]);
        for (key, value) in &group.entries {
            heap.protect_values(&[*key, *value]);
        }
    }
    if let Some((key, value)) = awaiting {
        heap.protect_values(&[*key, *value]);
    }
}
