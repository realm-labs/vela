use std::collections::{BTreeMap, BTreeSet};
use std::mem;

use crate::heap::{GcRef, HeapValue};
use crate::script_map::ScriptMap;
use crate::script_set::ScriptSet;
use crate::value_key::{KeyProbe, ValueKey};
use crate::{
    ExecutionBudget, HeapExecution, Value, VmError, VmErrorKind, VmResult, stored_runtime_value,
};

pub(crate) fn push_array_slot(
    heap: &mut HeapExecution<'_>,
    reference: GcRef,
    slot: Value,
    mut budget: Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<()> {
    let inserted = slot;
    if !tracks_collection_growth(budget.as_deref()) {
        array_slots_mut(heap, reference, operation)?.push(slot);
        heap.heap
            .note_container_value_inserted(reference, &inserted);
        return Ok(());
    }

    let len = array_slots(heap, reference, operation)?.len();
    check_collection_len("array", len, 1, budget.as_deref(), |budget| {
        budget.collection_limits().max_array_len
    })?;
    reserve_vec_slot(heap, reference, 1, operation)?;
    let precharged_growth = mem::size_of::<Value>();
    charge_growth(budget.as_deref_mut(), precharged_growth)?;

    array_slots_mut(heap, reference, operation)?.push(slot);
    heap.heap
        .note_container_value_inserted(reference, &inserted);
    heap.heap
        .adjust_object_size_after_mutation(reference, budget, precharged_growth)
}

pub(crate) fn insert_array_slot(
    heap: &mut HeapExecution<'_>,
    reference: GcRef,
    index: usize,
    slot: Value,
    mut budget: Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<()> {
    let inserted = slot;
    if !tracks_collection_growth(budget.as_deref()) {
        array_slots_mut(heap, reference, operation)?.insert(index, slot);
        heap.heap
            .note_container_value_inserted(reference, &inserted);
        return Ok(());
    }

    let len = array_slots(heap, reference, operation)?.len();
    check_collection_len("array", len, 1, budget.as_deref(), |budget| {
        budget.collection_limits().max_array_len
    })?;
    reserve_vec_slot(heap, reference, 1, operation)?;
    let precharged_growth = mem::size_of::<Value>();
    charge_growth(budget.as_deref_mut(), precharged_growth)?;

    let slots = array_slots_mut(heap, reference, operation)?;
    slots.insert(index, slot);
    heap.heap
        .note_container_value_inserted(reference, &inserted);
    heap.heap
        .adjust_object_size_after_mutation(reference, budget, precharged_growth)
}

pub(crate) fn extend_array_slots(
    heap: &mut HeapExecution<'_>,
    reference: GcRef,
    slots: impl IntoIterator<Item = Value>,
    mut budget: Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<()> {
    let slots = slots.into_iter().collect::<Vec<_>>();
    if !tracks_collection_growth(budget.as_deref()) {
        array_slots_mut(heap, reference, operation)?.extend(slots.iter().copied());
        for slot in &slots {
            heap.heap.note_container_value_inserted(reference, slot);
        }
        return Ok(());
    }

    let additional = slots.len();
    let len = array_slots(heap, reference, operation)?.len();
    check_collection_len("array", len, additional, budget.as_deref(), |budget| {
        budget.collection_limits().max_array_len
    })?;
    reserve_vec_slot(heap, reference, additional, operation)?;
    let precharged_growth = additional.saturating_mul(mem::size_of::<Value>());
    charge_growth(budget.as_deref_mut(), precharged_growth)?;

    array_slots_mut(heap, reference, operation)?.extend(slots.iter().copied());
    for slot in &slots {
        heap.heap.note_container_value_inserted(reference, slot);
    }
    heap.heap
        .adjust_object_size_after_mutation(reference, budget, precharged_growth)
}

pub(crate) fn pop_array_slot(
    heap: &mut HeapExecution<'_>,
    reference: GcRef,
    budget: Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<Option<Value>> {
    let payload = array_slots_mut(heap, reference, operation)?.pop();
    if payload.is_some() {
        heap.heap
            .note_container_value_replaced_or_removed(reference);
    }
    if !tracks_collection_growth(budget.as_deref()) {
        return Ok(payload.map(|slot| stored_runtime_value(&slot)));
    }
    heap.heap
        .adjust_object_size_after_mutation(reference, budget, 0)?;
    Ok(payload.map(|slot| stored_runtime_value(&slot)))
}

pub(crate) fn remove_array_slot(
    heap: &mut HeapExecution<'_>,
    reference: GcRef,
    index: usize,
    budget: Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<Value> {
    let slot = array_slots_mut(heap, reference, operation)?.remove(index);
    heap.heap
        .note_container_value_replaced_or_removed(reference);
    if !tracks_collection_growth(budget.as_deref()) {
        return Ok(stored_runtime_value(&slot));
    }
    heap.heap
        .adjust_object_size_after_mutation(reference, budget, 0)?;
    Ok(stored_runtime_value(&slot))
}

pub(crate) fn clear_array(
    heap: &mut HeapExecution<'_>,
    reference: GcRef,
    budget: Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<()> {
    array_slots_mut(heap, reference, operation)?.clear();
    heap.heap.note_container_cleared(reference);
    if !tracks_collection_growth(budget.as_deref()) {
        return Ok(());
    }
    heap.heap
        .adjust_object_size_after_mutation(reference, budget, 0)
}

pub(crate) fn retain_array_slots(
    heap: &mut HeapExecution<'_>,
    reference: GcRef,
    expected_len: usize,
    keep: &[bool],
    mut budget: Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<()> {
    if keep.len() != expected_len {
        return type_error(operation);
    }
    if array_slots(heap, reference, operation)?.len() != expected_len {
        return collection_changed(operation);
    }
    if let Some(budget) = budget.as_deref_mut() {
        budget.charge_execution_units(u64::try_from(expected_len).unwrap_or(u64::MAX))?;
    }
    let changed = keep.iter().any(|keep| !keep);
    let mut index = 0;
    array_slots_mut(heap, reference, operation)?.retain(|_| {
        let retain = keep[index];
        index += 1;
        retain
    });
    if changed {
        heap.heap
            .note_container_value_replaced_or_removed(reference);
    }
    if !tracks_collection_growth(budget.as_deref()) {
        return Ok(());
    }
    heap.heap
        .adjust_object_size_after_mutation(reference, budget, 0)
}

pub(crate) fn insert_map_slot(
    heap: &mut HeapExecution<'_>,
    reference: GcRef,
    key: Value,
    slot: Value,
    mut budget: Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<()> {
    let probe = KeyProbe::from_value(&key, Some(&*heap), operation)?;
    let existing_slot = map_slots(heap, reference, operation)?.slot_of_probe(&probe);
    let inserted = slot;
    if let Some(existing) = existing_slot {
        map_slots_mut(heap, reference, operation)?.replace_value_at(existing, slot);
        heap.heap.note_container_map_value_replaced(reference);
        return Ok(());
    }
    let value_key = ValueKey::from_value(&key, Some(&*heap), operation)?;
    if !tracks_collection_growth(budget.as_deref()) {
        map_slots_mut(heap, reference, operation)?.insert_keyed(value_key, key, slot);
        heap.heap
            .note_container_map_entry_inserted(reference, &key, &inserted);
        return Ok(());
    }

    let len = map_slots(heap, reference, operation)?.len();
    check_collection_len("map", len, 1, budget.as_deref(), |budget| {
        budget.collection_limits().max_map_entries
    })?;
    let precharged_growth = value_key
        .payload_size_bytes()
        .saturating_add(mem::size_of::<crate::script_map::MapEntry>());
    charge_growth(budget.as_deref_mut(), precharged_growth)?;

    map_slots_mut(heap, reference, operation)?.insert_keyed(value_key, key, slot);
    heap.heap
        .note_container_map_entry_inserted(reference, &key, &inserted);
    heap.heap
        .adjust_object_size_after_mutation(reference, budget, precharged_growth)
}

/// Writes through a constant string key.
///
/// Replacing an existing entry needs neither a heap key string nor an owned
/// `ValueKey`; only a genuinely new entry materializes the key, through the
/// same budgeted path as a dynamic-key insert.
pub(crate) fn insert_map_str_slot(
    heap: &mut HeapExecution<'_>,
    reference: GcRef,
    key: &str,
    slot: Value,
    mut budget: Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<()> {
    let existing_slot = map_slots(heap, reference, operation)?.slot_of_str_key(key);
    if let Some(existing) = existing_slot {
        map_slots_mut(heap, reference, operation)?.replace_value_at(existing, slot);
        heap.heap.note_container_map_value_replaced(reference);
        return Ok(());
    }
    let key_value = crate::heap_values::allocate_heap_value(
        crate::heap::HeapValue::String(key.to_owned()),
        heap,
        budget.as_deref_mut(),
    )?;
    insert_map_slot(heap, reference, key_value, slot, budget, operation)
}

pub(crate) fn extend_map_slots(
    heap: &mut HeapExecution<'_>,
    reference: GcRef,
    slots: Vec<(Value, Value)>,
    mut budget: Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<()> {
    let keyed_slots = slots
        .into_iter()
        .map(|(key, slot)| {
            Ok((
                ValueKey::from_value(&key, Some(&*heap), operation)?,
                key,
                slot,
            ))
        })
        .collect::<VmResult<Vec<_>>>()?;
    let existing_keys = {
        let values = map_slots(heap, reference, operation)?;
        keyed_slots
            .iter()
            .filter(|(key, _, _)| values.contains_key(key))
            .map(|(key, _, _)| key.clone())
            .collect::<BTreeSet<_>>()
    };
    let had_replacement = !existing_keys.is_empty();
    let inserted_entries = unique_new_map_entries(&keyed_slots, &existing_keys);
    if !tracks_collection_growth(budget.as_deref()) {
        let values = map_slots_mut(heap, reference, operation)?;
        for (value_key, key, slot) in &keyed_slots {
            values.insert_keyed(value_key.clone(), *key, *slot);
        }
        if had_replacement {
            heap.heap.note_container_map_value_replaced(reference);
        }
        for (_, key, slot) in &inserted_entries {
            heap.heap
                .note_container_map_entry_inserted(reference, key, slot);
        }
        return Ok(());
    }

    let values = map_slots(heap, reference, operation)?;
    check_collection_len(
        "map",
        values.len(),
        inserted_entries.len(),
        budget.as_deref(),
        |budget| budget.collection_limits().max_map_entries,
    )?;
    let precharged_growth = inserted_entries
        .iter()
        .map(|(key, _, _)| {
            key.payload_size_bytes()
                .saturating_add(mem::size_of::<crate::script_map::MapEntry>())
        })
        .sum::<usize>();
    charge_growth(budget.as_deref_mut(), precharged_growth)?;

    let values = map_slots_mut(heap, reference, operation)?;
    for (value_key, key, slot) in &keyed_slots {
        values.insert_keyed(value_key.clone(), *key, *slot);
    }
    if had_replacement {
        heap.heap.note_container_map_value_replaced(reference);
    }
    for (_, key, slot) in &inserted_entries {
        heap.heap
            .note_container_map_entry_inserted(reference, key, slot);
    }
    heap.heap
        .adjust_object_size_after_mutation(reference, budget, precharged_growth)
}

pub(crate) fn remove_map_slot(
    heap: &mut HeapExecution<'_>,
    reference: GcRef,
    key: &Value,
    budget: Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<Option<Value>> {
    let probe = KeyProbe::from_value(key, Some(&*heap), operation)?;
    let existing_slot = map_slots(heap, reference, operation)?.slot_of_probe(&probe);
    let payload = existing_slot
        .map(|slot| {
            Ok::<_, crate::VmError>(
                map_slots_mut(heap, reference, operation)?.remove_value_at(slot),
            )
        })
        .transpose()?;
    if payload.is_some() {
        heap.heap.note_container_map_entry_removed(reference);
    }
    if !tracks_collection_growth(budget.as_deref()) {
        return Ok(payload);
    }
    heap.heap
        .adjust_object_size_after_mutation(reference, budget, 0)?;
    Ok(payload)
}

pub(crate) fn clear_map(
    heap: &mut HeapExecution<'_>,
    reference: GcRef,
    budget: Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<()> {
    map_slots_mut(heap, reference, operation)?.clear();
    heap.heap.note_container_cleared(reference);
    if !tracks_collection_growth(budget.as_deref()) {
        return Ok(());
    }
    heap.heap
        .adjust_object_size_after_mutation(reference, budget, 0)
}

pub(crate) fn retain_map_slots(
    heap: &mut HeapExecution<'_>,
    reference: GcRef,
    expected: &BTreeSet<ValueKey>,
    keep: &BTreeSet<ValueKey>,
    mut budget: Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<()> {
    let current = map_slots(heap, reference, operation)?
        .key_order()
        .into_iter()
        .collect::<BTreeSet<_>>();
    if current != *expected || !keep.is_subset(expected) {
        return collection_changed(operation);
    }
    if let Some(budget) = budget.as_deref_mut() {
        budget.charge_execution_units(u64::try_from(expected.len()).unwrap_or(u64::MAX))?;
    }
    let removed = expected
        .difference(keep)
        .cloned()
        .collect::<Vec<ValueKey>>();
    if !removed.is_empty() {
        let values = map_slots_mut(heap, reference, operation)?;
        for key in &removed {
            values.remove_keyed(key);
        }
        heap.heap.note_container_map_entry_removed(reference);
    }
    if !tracks_collection_growth(budget.as_deref()) {
        return Ok(());
    }
    heap.heap
        .adjust_object_size_after_mutation(reference, budget, 0)
}

pub(crate) fn push_set_slot(
    heap: &mut HeapExecution<'_>,
    reference: GcRef,
    slot: Value,
    mut budget: Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<()> {
    let inserted = slot;
    let key = ValueKey::from_value(&slot, Some(&*heap), operation)?;
    let is_new_key = !set_slots(heap, reference, operation)?.contains_key(&key);
    if !is_new_key {
        return Ok(());
    }
    if !tracks_collection_growth(budget.as_deref()) {
        if set_slots_mut(heap, reference, operation)?.insert_keyed(key, slot) {
            heap.heap
                .note_container_value_inserted(reference, &inserted);
        }
        return Ok(());
    }

    let len = set_slots(heap, reference, operation)?.len();
    check_collection_len("set", len, 1, budget.as_deref(), |budget| {
        budget.collection_limits().max_set_len
    })?;
    let precharged_growth = key
        .payload_size_bytes()
        .saturating_add(mem::size_of::<Value>());
    charge_growth(budget.as_deref_mut(), precharged_growth)?;

    if set_slots_mut(heap, reference, operation)?.insert_keyed(key, slot) {
        heap.heap
            .note_container_value_inserted(reference, &inserted);
    }
    heap.heap
        .adjust_object_size_after_mutation(reference, budget, precharged_growth)
}

pub(crate) fn extend_set_slots(
    heap: &mut HeapExecution<'_>,
    reference: GcRef,
    slots: impl IntoIterator<Item = Value>,
    mut budget: Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<()> {
    let mut slots = slots
        .into_iter()
        .map(|slot| Ok((ValueKey::from_value(&slot, Some(&*heap), operation)?, slot)))
        .collect::<VmResult<Vec<_>>>()?;
    slots.retain(|(key, _)| {
        !set_slots(heap, reference, operation).is_ok_and(|set| set.contains_key(key))
    });
    dedup_keyed_slots(&mut slots);
    if !tracks_collection_growth(budget.as_deref()) {
        let set = set_slots_mut(heap, reference, operation)?;
        for (key, slot) in &slots {
            set.insert_keyed(key.clone(), *slot);
        }
        for (_, slot) in &slots {
            heap.heap.note_container_value_inserted(reference, slot);
        }
        return Ok(());
    }

    let additional = slots.len();
    let len = set_slots(heap, reference, operation)?.len();
    check_collection_len("set", len, additional, budget.as_deref(), |budget| {
        budget.collection_limits().max_set_len
    })?;
    let precharged_growth = slots
        .iter()
        .map(|(key, _)| {
            key.payload_size_bytes()
                .saturating_add(mem::size_of::<Value>())
        })
        .sum::<usize>();
    charge_growth(budget.as_deref_mut(), precharged_growth)?;

    let set = set_slots_mut(heap, reference, operation)?;
    for (key, slot) in &slots {
        set.insert_keyed(key.clone(), *slot);
    }
    for (_, slot) in &slots {
        heap.heap.note_container_value_inserted(reference, slot);
    }
    heap.heap
        .adjust_object_size_after_mutation(reference, budget, precharged_growth)
}

pub(crate) fn remove_set_slot(
    heap: &mut HeapExecution<'_>,
    reference: GcRef,
    key: &ValueKey,
    budget: Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<bool> {
    let before = set_slots(heap, reference, operation)?.len();
    let changed = set_slots_mut(heap, reference, operation)?.remove_keyed(key);
    let changed = changed && set_slots(heap, reference, operation)?.len() != before;
    if changed {
        heap.heap
            .note_container_value_replaced_or_removed(reference);
    }
    if !tracks_collection_growth(budget.as_deref()) {
        return Ok(changed);
    }
    heap.heap
        .adjust_object_size_after_mutation(reference, budget, 0)?;
    Ok(changed)
}

pub(crate) fn clear_set(
    heap: &mut HeapExecution<'_>,
    reference: GcRef,
    budget: Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<()> {
    set_slots_mut(heap, reference, operation)?.clear();
    heap.heap.note_container_cleared(reference);
    if !tracks_collection_growth(budget.as_deref()) {
        return Ok(());
    }
    heap.heap
        .adjust_object_size_after_mutation(reference, budget, 0)
}

pub(crate) fn retain_set_slots(
    heap: &mut HeapExecution<'_>,
    reference: GcRef,
    expected: &BTreeSet<ValueKey>,
    keep: &BTreeSet<ValueKey>,
    mut budget: Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<()> {
    let current = set_slots(heap, reference, operation)?
        .values()
        .map(|value| ValueKey::from_value(value, Some(&*heap), operation))
        .collect::<VmResult<BTreeSet<_>>>()?;
    if current != *expected || !keep.is_subset(expected) {
        return collection_changed(operation);
    }
    if let Some(budget) = budget.as_deref_mut() {
        budget.charge_execution_units(u64::try_from(expected.len()).unwrap_or(u64::MAX))?;
    }
    let removed = expected
        .difference(keep)
        .cloned()
        .collect::<Vec<ValueKey>>();
    if !removed.is_empty() {
        let values = set_slots_mut(heap, reference, operation)?;
        for key in &removed {
            values.remove_keyed(key);
        }
        heap.heap
            .note_container_value_replaced_or_removed(reference);
    }
    if !tracks_collection_growth(budget.as_deref()) {
        return Ok(());
    }
    heap.heap
        .adjust_object_size_after_mutation(reference, budget, 0)
}

#[inline(always)]
fn tracks_collection_growth(budget: Option<&ExecutionBudget>) -> bool {
    budget.is_some_and(ExecutionBudget::tracks_collection_growth)
}

pub(crate) fn check_collection_len(
    collection: &'static str,
    current_len: usize,
    additional: usize,
    budget: Option<&ExecutionBudget>,
    limit: impl FnOnce(&ExecutionBudget) -> usize,
) -> VmResult<()> {
    let Some(budget) = budget else {
        return Ok(());
    };
    if !budget.limits_collections() {
        return Ok(());
    }
    let limit = limit(budget);
    if current_len.saturating_add(additional) > limit {
        return Err(VmError::new(VmErrorKind::CollectionLimitExceeded {
            collection,
            limit,
        }));
    }
    Ok(())
}

fn charge_growth(budget: Option<&mut ExecutionBudget>, bytes: usize) -> VmResult<()> {
    if bytes == 0 {
        return Ok(());
    }
    if let Some(budget) = budget
        && budget.charges_memory()
    {
        budget.charge_memory(bytes)?;
    }
    Ok(())
}

fn reserve_vec_slot(
    heap: &mut HeapExecution<'_>,
    reference: GcRef,
    additional: usize,
    operation: &'static str,
) -> VmResult<()> {
    let value = heap
        .heap
        .get_mut(reference)
        .map_err(|_| VmError::new(VmErrorKind::TypeMismatch { operation }))?;
    let values = match value {
        HeapValue::Array(values) => values,
        _ => return type_error(operation),
    };
    values
        .try_reserve(additional)
        .map_err(|_| VmError::new(VmErrorKind::AllocationFailed { operation }))
}

fn array_slots<'a>(
    heap: &'a HeapExecution<'_>,
    reference: GcRef,
    operation: &'static str,
) -> VmResult<&'a [Value]> {
    let Some(HeapValue::Array(values)) = heap.heap.get(reference) else {
        return type_error(operation);
    };
    Ok(values)
}

fn array_slots_mut<'a>(
    heap: &'a mut HeapExecution<'_>,
    reference: GcRef,
    operation: &'static str,
) -> VmResult<&'a mut Vec<Value>> {
    let Some(HeapValue::Array(values)) = heap.heap.get_mut(reference).ok() else {
        return type_error(operation);
    };
    Ok(values)
}

fn dedup_keyed_slots(slots: &mut Vec<(ValueKey, Value)>) {
    let mut keys = BTreeSet::new();
    slots.retain(|(key, _)| keys.insert(key.clone()));
}

fn unique_new_map_entries(
    slots: &[(ValueKey, Value, Value)],
    existing_keys: &BTreeSet<ValueKey>,
) -> Vec<(ValueKey, Value, Value)> {
    let mut entries = BTreeMap::new();
    for (value_key, key, value) in slots {
        if existing_keys.contains(value_key) {
            continue;
        }
        entries
            .entry(value_key.clone())
            .and_modify(|(_, stored_value)| *stored_value = *value)
            .or_insert((*key, *value));
    }
    entries
        .into_iter()
        .map(|(value_key, (key, value))| (value_key, key, value))
        .collect()
}

fn map_slots<'a>(
    heap: &'a HeapExecution<'_>,
    reference: GcRef,
    operation: &'static str,
) -> VmResult<&'a ScriptMap> {
    let Some(HeapValue::Map(values)) = heap.heap.get(reference) else {
        return type_error(operation);
    };
    Ok(values)
}

fn map_slots_mut<'a>(
    heap: &'a mut HeapExecution<'_>,
    reference: GcRef,
    operation: &'static str,
) -> VmResult<&'a mut ScriptMap> {
    let Some(HeapValue::Map(values)) = heap.heap.get_mut(reference).ok() else {
        return type_error(operation);
    };
    Ok(values)
}

fn set_slots<'a>(
    heap: &'a HeapExecution<'_>,
    reference: GcRef,
    operation: &'static str,
) -> VmResult<&'a ScriptSet> {
    let Some(HeapValue::Set(values)) = heap.heap.get(reference) else {
        return type_error(operation);
    };
    Ok(values)
}

fn set_slots_mut<'a>(
    heap: &'a mut HeapExecution<'_>,
    reference: GcRef,
    operation: &'static str,
) -> VmResult<&'a mut ScriptSet> {
    let Some(HeapValue::Set(values)) = heap.heap.get_mut(reference).ok() else {
        return type_error(operation);
    };
    Ok(values)
}

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}

fn collection_changed<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::CollectionChangedDuringCallback {
        operation,
    }))
}

#[cfg(test)]
mod tests;
