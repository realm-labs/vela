use crate::heap_values::make_set_value;
use crate::script_set::ScriptSet;
use crate::value_key::ValueKey;
use crate::{ExecutionBudget, HeapExecution, Value, VmResult};

use super::{SetRelation, expect_arity, relation_matches, set_slots};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SetCombination {
    Union,
    Intersection,
    Difference,
    SymmetricDifference,
}

pub(crate) fn union(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("union", args, 1)?;
    let operation = "method union";
    let values = combination_values(
        receiver,
        &args[0],
        heap.as_deref(),
        SetCombination::Union,
        operation,
    )?;
    make_result_set(values, heap, budget, operation)
}

pub(crate) fn intersection(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("intersection", args, 1)?;
    let operation = "method intersection";
    let values = combination_values(
        receiver,
        &args[0],
        heap.as_deref(),
        SetCombination::Intersection,
        operation,
    )?;
    make_result_set(values, heap, budget, operation)
}

pub(crate) fn difference(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("difference", args, 1)?;
    let operation = "method difference";
    let values = combination_values(
        receiver,
        &args[0],
        heap.as_deref(),
        SetCombination::Difference,
        operation,
    )?;
    make_result_set(values, heap, budget, operation)
}

pub(crate) fn symmetric_difference(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("symmetric_difference", args, 1)?;
    let operation = "method symmetric_difference";
    let values = combination_values(
        receiver,
        &args[0],
        heap.as_deref(),
        SetCombination::SymmetricDifference,
        operation,
    )?;
    make_result_set(values, heap, budget, operation)
}

fn combination_values(
    receiver: &Value,
    other: &Value,
    heap: Option<&HeapExecution<'_>>,
    combination: SetCombination,
    operation: &'static str,
) -> VmResult<Vec<Value>> {
    let left = set_slots(receiver, heap, operation)?;
    let right = set_slots(other, heap, operation)?;
    combination_payload(left, right, heap, combination, operation)
}

pub(crate) fn combination_payload(
    left: &ScriptSet,
    right: &ScriptSet,
    heap: Option<&HeapExecution<'_>>,
    combination: SetCombination,
    operation: &'static str,
) -> VmResult<Vec<Value>> {
    match combination {
        SetCombination::Union => {
            let mut combined = ScriptSet::new();
            for value in left.values().chain(right.values()) {
                combined.insert(*value, heap, operation)?;
            }
            Ok(combined.values_vec())
        }
        SetCombination::Intersection => {
            let mut result = ScriptSet::new();
            for value in left.values() {
                let key = ValueKey::from_value(value, heap, operation)?;
                if right.contains_key(&key) {
                    result.insert_keyed(key, *value);
                }
            }
            Ok(result.values_vec())
        }
        SetCombination::Difference => {
            let mut result = ScriptSet::new();
            for value in left.values() {
                let key = ValueKey::from_value(value, heap, operation)?;
                if !right.contains_key(&key) {
                    result.insert_keyed(key, *value);
                }
            }
            Ok(result.values_vec())
        }
        SetCombination::SymmetricDifference => {
            let mut result = ScriptSet::new();
            for value in left.values() {
                let key = ValueKey::from_value(value, heap, operation)?;
                if !right.contains_key(&key) {
                    result.insert_keyed(key, *value);
                }
            }
            for value in right.values() {
                let key = ValueKey::from_value(value, heap, operation)?;
                if !left.contains_key(&key) {
                    result.insert_keyed(key, *value);
                }
            }
            Ok(result.values_vec())
        }
    }
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
