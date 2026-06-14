use crate::heap_values::make_set_value;
use crate::{ExecutionBudget, HeapExecution, Value, VmResult};

use super::{
    SetKey, SetRelation, expect_arity, push_unique, relation_matches, set_keys, set_slots,
};

pub(crate) fn union(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("union", args, 1)?;
    let mut combined = Vec::new();
    for value in set_slots(receiver, heap.as_deref(), "method union")?.values() {
        push_unique(&mut combined, *value, heap.as_deref(), "method union")?;
    }
    for value in set_slots(&args[0], heap.as_deref(), "method union")?.values() {
        push_unique(&mut combined, *value, heap.as_deref(), "method union")?;
    }
    make_result_set(combined, heap, budget, "method union")
}

pub(crate) fn intersection(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("intersection", args, 1)?;
    let right = set_keys(
        &set_slots(&args[0], heap.as_deref(), "method intersection")?.values_vec(),
        heap.as_deref(),
        "method intersection",
    )?;
    let mut result = Vec::new();
    for value in set_slots(receiver, heap.as_deref(), "method intersection")?.values() {
        let key = SetKey::from_value(value, heap.as_deref(), "method intersection")?;
        if right.contains(&key) {
            push_unique(&mut result, *value, heap.as_deref(), "method intersection")?;
        }
    }
    make_result_set(result, heap, budget, "method intersection")
}

pub(crate) fn difference(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("difference", args, 1)?;
    let right = set_keys(
        &set_slots(&args[0], heap.as_deref(), "method difference")?.values_vec(),
        heap.as_deref(),
        "method difference",
    )?;
    let mut result = Vec::new();
    for value in set_slots(receiver, heap.as_deref(), "method difference")?.values() {
        let key = SetKey::from_value(value, heap.as_deref(), "method difference")?;
        if !right.contains(&key) {
            push_unique(&mut result, *value, heap.as_deref(), "method difference")?;
        }
    }
    make_result_set(result, heap, budget, "method difference")
}

pub(crate) fn symmetric_difference(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("symmetric_difference", args, 1)?;
    let left_values =
        set_slots(receiver, heap.as_deref(), "method symmetric_difference")?.values_vec();
    let right_values =
        set_slots(&args[0], heap.as_deref(), "method symmetric_difference")?.values_vec();
    let left_keys = set_keys(&left_values, heap.as_deref(), "method symmetric_difference")?;
    let right_keys = set_keys(
        &right_values,
        heap.as_deref(),
        "method symmetric_difference",
    )?;

    let mut result = Vec::new();
    for (value, key) in left_values.iter().zip(left_keys.iter()) {
        if !right_keys.contains(key) {
            push_unique(
                &mut result,
                *value,
                heap.as_deref(),
                "method symmetric_difference",
            )?;
        }
    }
    for (value, key) in right_values.iter().zip(right_keys.iter()) {
        if !left_keys.contains(key) {
            push_unique(
                &mut result,
                *value,
                heap.as_deref(),
                "method symmetric_difference",
            )?;
        }
    }
    make_result_set(result, heap, budget, "method symmetric_difference")
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
