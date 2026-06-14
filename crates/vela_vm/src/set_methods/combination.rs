use crate::heap_values::make_set_value;
use crate::script_set::ScriptSet;
use crate::{ExecutionBudget, HeapExecution, Value, VmResult};

use super::{SetKey, SetRelation, expect_arity, relation_matches, set_slots};

pub(crate) fn union(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("union", args, 1)?;
    let mut combined = ScriptSet::new();
    for value in set_slots(receiver, heap.as_deref(), "method union")?.values() {
        combined.insert(*value, heap.as_deref(), "method union")?;
    }
    for value in set_slots(&args[0], heap.as_deref(), "method union")?.values() {
        combined.insert(*value, heap.as_deref(), "method union")?;
    }
    make_result_set(combined.values_vec(), heap, budget, "method union")
}

pub(crate) fn intersection(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("intersection", args, 1)?;
    let right = set_slots(&args[0], heap.as_deref(), "method intersection")?;
    let mut result = ScriptSet::new();
    for value in set_slots(receiver, heap.as_deref(), "method intersection")?.values() {
        let key = SetKey::from_value(value, heap.as_deref(), "method intersection")?;
        if right.contains_key(&key) {
            result.insert_keyed(key, *value);
        }
    }
    make_result_set(result.values_vec(), heap, budget, "method intersection")
}

pub(crate) fn difference(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("difference", args, 1)?;
    let right = set_slots(&args[0], heap.as_deref(), "method difference")?;
    let mut result = ScriptSet::new();
    for value in set_slots(receiver, heap.as_deref(), "method difference")?.values() {
        let key = SetKey::from_value(value, heap.as_deref(), "method difference")?;
        if !right.contains_key(&key) {
            result.insert_keyed(key, *value);
        }
    }
    make_result_set(result.values_vec(), heap, budget, "method difference")
}

pub(crate) fn symmetric_difference(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("symmetric_difference", args, 1)?;
    let left = set_slots(receiver, heap.as_deref(), "method symmetric_difference")?;
    let right = set_slots(&args[0], heap.as_deref(), "method symmetric_difference")?;

    let mut result = ScriptSet::new();
    for value in left.values() {
        let key = SetKey::from_value(value, heap.as_deref(), "method symmetric_difference")?;
        if !right.contains_key(&key) {
            result.insert_keyed(key, *value);
        }
    }
    for value in right.values() {
        let key = SetKey::from_value(value, heap.as_deref(), "method symmetric_difference")?;
        if !left.contains_key(&key) {
            result.insert_keyed(key, *value);
        }
    }
    make_result_set(
        result.values_vec(),
        heap,
        budget,
        "method symmetric_difference",
    )
}

pub(crate) fn is_subset(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    expect_arity("is_subset", args, 1)?;
    let operation = "method is_subset";
    let Some(heap) = heap else {
        return super::type_error(operation);
    };
    let receiver_values = set_slots(receiver, Some(heap), operation)?;
    relation_matches(
        receiver_values,
        &args[0],
        heap,
        SetRelation::Subset,
        operation,
    )
}

pub(crate) fn is_superset(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    expect_arity("is_superset", args, 1)?;
    let operation = "method is_superset";
    let Some(heap) = heap else {
        return super::type_error(operation);
    };
    let receiver_values = set_slots(receiver, Some(heap), operation)?;
    relation_matches(
        receiver_values,
        &args[0],
        heap,
        SetRelation::Superset,
        operation,
    )
}

pub(crate) fn is_disjoint(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    expect_arity("is_disjoint", args, 1)?;
    let operation = "method is_disjoint";
    let Some(heap) = heap else {
        return super::type_error(operation);
    };
    let receiver_values = set_slots(receiver, Some(heap), operation)?;
    relation_matches(
        receiver_values,
        &args[0],
        heap,
        SetRelation::Disjoint,
        operation,
    )
}

fn make_result_set(
    values: Vec<Value>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<Value> {
    let Some(heap) = heap.as_deref_mut() else {
        return super::type_error(operation);
    };
    make_set_value(values, heap, budget.as_deref_mut())
}
