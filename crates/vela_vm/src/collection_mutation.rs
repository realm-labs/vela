use std::collections::BTreeSet;
use std::mem;

use crate::heap::{GcRef, HeapValue};
use crate::script_map::ScriptMap;
use crate::script_set::ScriptSet;
use crate::value_key::ValueKey;
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

pub(crate) fn insert_map_slot(
    heap: &mut HeapExecution<'_>,
    reference: GcRef,
    key: Value,
    slot: Value,
    mut budget: Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<()> {
    let value_key = ValueKey::from_value(&key, Some(&*heap), operation)?;
    let values = map_slots(heap, reference, operation)?;
    let is_new_key = !values.contains_key(&value_key);
    let inserted = slot;
    if !is_new_key || !tracks_collection_growth(budget.as_deref()) {
        map_slots_mut(heap, reference, operation)?.insert_keyed(value_key, key, slot);
        if is_new_key {
            heap.heap
                .note_container_map_entry_inserted(reference, &key, &inserted);
        } else {
            heap.heap
                .note_container_value_replaced_or_removed(reference);
        }
        return Ok(());
    }

    let precharged_growth = if is_new_key {
        check_collection_len("map", values.len(), 1, budget.as_deref(), |budget| {
            budget.collection_limits().max_map_entries
        })?;
        value_key
            .payload_size_bytes()
            .saturating_add(mem::size_of::<crate::script_map::MapEntry>())
    } else {
        0
    };
    charge_growth(budget.as_deref_mut(), precharged_growth)?;

    map_slots_mut(heap, reference, operation)?.insert_keyed(value_key, key, slot);
    heap.heap
        .note_container_map_entry_inserted(reference, &key, &inserted);
    heap.heap
        .adjust_object_size_after_mutation(reference, budget, precharged_growth)
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
    let had_replacement = {
        let values = map_slots(heap, reference, operation)?;
        keyed_slots
            .iter()
            .any(|(key, _, _)| values.contains_key(key))
    };
    let inserted_entries = {
        let values = map_slots(heap, reference, operation)?;
        keyed_slots
            .iter()
            .filter(|(key, _, _)| !values.contains_key(key))
            .map(|(_, key, slot)| (*key, *slot))
            .collect::<Vec<_>>()
    };
    if !tracks_collection_growth(budget.as_deref()) {
        let values = map_slots_mut(heap, reference, operation)?;
        for (value_key, key, slot) in &keyed_slots {
            values.insert_keyed(value_key.clone(), *key, *slot);
        }
        if had_replacement {
            heap.heap
                .note_container_value_replaced_or_removed(reference);
        }
        for (key, slot) in &inserted_entries {
            heap.heap
                .note_container_map_entry_inserted(reference, key, slot);
        }
        return Ok(());
    }

    let values = map_slots(heap, reference, operation)?;
    let new_keys = keyed_slots
        .iter()
        .filter(|(key, _, _)| !values.contains_key(key))
        .collect::<Vec<_>>();
    check_collection_len(
        "map",
        values.len(),
        new_keys.len(),
        budget.as_deref(),
        |budget| budget.collection_limits().max_map_entries,
    )?;
    let precharged_growth = new_keys
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
        heap.heap
            .note_container_value_replaced_or_removed(reference);
    }
    for (key, slot) in &inserted_entries {
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
    let key = ValueKey::from_value(key, Some(&*heap), operation)?;
    let payload = map_slots_mut(heap, reference, operation)?.remove_keyed(&key);
    if payload.is_some() {
        heap.heap
            .note_container_value_replaced_or_removed(reference);
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

pub(crate) fn push_set_slot(
    heap: &mut HeapExecution<'_>,
    reference: GcRef,
    slot: Value,
    mut budget: Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<()> {
    let inserted = slot;
    let key = ValueKey::from_value(&slot, Some(&*heap), operation)?;
    if !tracks_collection_growth(budget.as_deref()) {
        set_slots_mut(heap, reference, operation)?.insert_keyed(key, slot);
        heap.heap
            .note_container_value_inserted(reference, &inserted);
        return Ok(());
    }

    let len = set_slots(heap, reference, operation)?.len();
    check_collection_len("set", len, 1, budget.as_deref(), |budget| {
        budget.collection_limits().max_set_len
    })?;
    let precharged_growth = mem::size_of::<Value>();
    charge_growth(budget.as_deref_mut(), precharged_growth)?;

    set_slots_mut(heap, reference, operation)?.insert_keyed(key, slot);
    heap.heap
        .note_container_value_inserted(reference, &inserted);
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
    let precharged_growth = additional.saturating_mul(mem::size_of::<Value>());
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

#[cfg(test)]
mod tests {
    use crate::budget::CollectionLimits;
    use crate::heap::{HeapValue, ScriptHeap};
    use crate::script_set::ScriptSet;
    use crate::{ExecutionBudget, HeapExecution, Value, VmErrorKind};

    use super::{insert_map_slot, push_array_slot, push_set_slot};

    #[test]
    fn array_push_charges_container_slot_growth() {
        let mut heap = ScriptHeap::new();
        let reference = heap.allocate(HeapValue::Array(Vec::new()));
        let initial_bytes = heap.allocated_bytes();
        let mut heap_execution = HeapExecution::new(&mut heap);
        let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX);

        push_array_slot(
            &mut heap_execution,
            reference,
            Value::I64(10),
            Some(&mut budget),
            "test array push",
        )
        .expect("array push should fit");

        assert!(heap_execution.heap.allocated_bytes() > initial_bytes);
        assert_eq!(
            heap_execution.heap.allocated_bytes() - initial_bytes,
            budget.memory_bytes_allocated()
        );
    }

    #[test]
    fn unbounded_budget_skips_collection_growth_accounting() {
        let mut heap = ScriptHeap::new();
        let reference = heap.allocate(HeapValue::Array(Vec::new()));
        let initial_bytes = heap.allocated_bytes();
        let mut heap_execution = HeapExecution::new(&mut heap);
        let mut budget = ExecutionBudget::unbounded();

        push_array_slot(
            &mut heap_execution,
            reference,
            Value::I64(10),
            Some(&mut budget),
            "test array push",
        )
        .expect("array push should fit");

        assert_eq!(heap_execution.heap.allocated_bytes(), initial_bytes);
        assert_eq!(budget.memory_bytes_allocated(), 0);
        assert_eq!(
            heap_execution.heap.get(reference),
            Some(&HeapValue::Array(vec![Value::I64(10)]))
        );
    }

    #[test]
    fn array_push_rejects_memory_growth_before_mutation() {
        let mut heap = ScriptHeap::new();
        let reference = heap.allocate(HeapValue::Array(Vec::new()));
        let initial_bytes = heap.allocated_bytes();
        let mut heap_execution = HeapExecution::new(&mut heap);
        let mut budget = ExecutionBudget::new(u64::MAX, 1, usize::MAX);

        let error = push_array_slot(
            &mut heap_execution,
            reference,
            Value::I64(10),
            Some(&mut budget),
            "test array push",
        )
        .expect_err("array push should exceed memory budget");

        assert!(matches!(
            error.kind_ref(),
            VmErrorKind::BudgetExceeded { .. }
        ));
        assert_eq!(heap_execution.heap.allocated_bytes(), initial_bytes);
        assert_eq!(
            heap_execution.heap.get(reference),
            Some(&HeapValue::Array(Vec::new()))
        );
    }

    #[test]
    fn map_insert_rejects_entry_limit_before_mutation() {
        let mut heap = ScriptHeap::new();
        let reference = heap.allocate(HeapValue::Map(Default::default()));
        let mut heap_execution = HeapExecution::new(&mut heap);
        let mut budget = ExecutionBudget::unbounded().with_collection_limits(CollectionLimits {
            max_array_len: usize::MAX,
            max_map_entries: 0,
            max_set_len: usize::MAX,
        });

        let error = insert_map_slot(
            &mut heap_execution,
            reference,
            Value::I64(1),
            Value::I64(10),
            Some(&mut budget),
            "test map set",
        )
        .expect_err("map insert should exceed entry limit");

        assert!(matches!(
            error.kind_ref(),
            VmErrorKind::CollectionLimitExceeded {
                collection: "map",
                limit: 0
            }
        ));
        assert_eq!(
            heap_execution.heap.get(reference),
            Some(&HeapValue::Map(Default::default()))
        );
    }

    #[test]
    fn set_add_rejects_length_limit_before_mutation() {
        let mut heap = ScriptHeap::new();
        let reference = heap.allocate(HeapValue::Set(ScriptSet::new()));
        let mut heap_execution = HeapExecution::new(&mut heap);
        let mut budget = ExecutionBudget::unbounded().with_collection_limits(CollectionLimits {
            max_array_len: usize::MAX,
            max_map_entries: usize::MAX,
            max_set_len: 0,
        });

        let error = push_set_slot(
            &mut heap_execution,
            reference,
            Value::I64(10),
            Some(&mut budget),
            "test set add",
        )
        .expect_err("set add should exceed length limit");

        assert!(matches!(
            error.kind_ref(),
            VmErrorKind::CollectionLimitExceeded {
                collection: "set",
                limit: 0
            }
        ));
        assert_eq!(
            heap_execution.heap.get(reference),
            Some(&HeapValue::Set(ScriptSet::new()))
        );
    }
}
