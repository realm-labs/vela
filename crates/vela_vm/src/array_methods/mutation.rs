use crate::heap::HeapValue;
use crate::{
    ExecutionBudget, HeapExecution, Value, VmResult, value_from_heap_slot, value_to_heap_slot,
};

use super::{
    expect_arity, index_out_of_bounds, index_value, materialize_array_values, option_value,
    type_error,
};

pub(crate) fn push(
    receiver: &mut Value,
    args: &[Value],
    heap: Option<&mut HeapExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("push", args, 1)?;
    match receiver {
        Value::HeapRef(reference) => {
            let Some(heap) = heap else {
                return type_error("method push");
            };
            let slot = value_to_heap_slot(&args[0], heap, budget)?;
            let Some(HeapValue::Array(values)) = heap.heap.get_mut(*reference).ok() else {
                return type_error("method push");
            };
            values.push(slot);
            Ok(Value::Null)
        }
        _ => type_error("method push"),
    }
}

pub(crate) fn pop(
    receiver: &mut Value,
    args: &[Value],
    heap: Option<&mut HeapExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("pop", args, 0)?;
    let mut heap = heap;
    let mut budget = budget;
    match receiver {
        Value::HeapRef(reference) => {
            let Some(heap_ref) = heap.as_deref_mut() else {
                return type_error("method pop");
            };
            let Some(HeapValue::Array(values)) = heap_ref.heap.get_mut(*reference).ok() else {
                return type_error("method pop");
            };
            let payload = values.pop().map(|slot| value_from_heap_slot(&slot));
            if payload.is_some() {
                option_value("Some", payload, &mut heap, &mut budget)
            } else {
                option_value("None", None, &mut heap, &mut budget)
            }
        }
        _ => type_error("method pop"),
    }
}

pub(crate) fn remove_at(
    receiver: &mut Value,
    args: &[Value],
    heap: Option<&mut HeapExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("remove_at", args, 1)?;
    let index = index_value(&args[0], "method remove_at")?;
    let mut heap = heap;
    let mut budget = budget;
    match receiver {
        Value::HeapRef(reference) => {
            let Some(heap_ref) = heap.as_deref_mut() else {
                return type_error("method remove_at");
            };
            let Some(HeapValue::Array(values)) = heap_ref.heap.get_mut(*reference).ok() else {
                return type_error("method remove_at");
            };
            if index >= values.len() {
                return option_value("None", None, &mut heap, &mut budget);
            }
            let value = value_from_heap_slot(&values.remove(index));
            option_value("Some", Some(value), &mut heap, &mut budget)
        }
        _ => type_error("method remove_at"),
    }
}

pub(crate) fn insert(
    receiver: &mut Value,
    args: &[Value],
    heap: Option<&mut HeapExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("insert", args, 2)?;
    let index = index_value(&args[0], "method insert")?;
    match receiver {
        Value::HeapRef(reference) => {
            let Some(heap) = heap else {
                return type_error("method insert");
            };
            let len = match heap.heap.get(*reference) {
                Some(HeapValue::Array(values)) => values.len(),
                _ => return type_error("method insert"),
            };
            if index > len {
                return Err(index_out_of_bounds(index, len));
            }
            let slot = value_to_heap_slot(&args[1], heap, budget)?;
            let Some(HeapValue::Array(values)) = heap.heap.get_mut(*reference).ok() else {
                return type_error("method insert");
            };
            values.insert(index, slot);
            Ok(Value::Null)
        }
        _ => type_error("method insert"),
    }
}

pub(crate) fn extend(
    receiver: &mut Value,
    args: &[Value],
    heap: Option<&mut HeapExecution<'_>>,
    mut budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("extend", args, 1)?;
    let extension = materialize_array_values(&args[0], heap.as_deref(), "method extend")?;
    match receiver {
        Value::HeapRef(reference) => {
            let Some(heap) = heap else {
                return type_error("method extend");
            };
            let mut slots = Vec::with_capacity(extension.len());
            for value in &extension {
                slots.push(value_to_heap_slot(value, heap, budget.as_deref_mut())?);
            }
            let Some(HeapValue::Array(values)) = heap.heap.get_mut(*reference).ok() else {
                return type_error("method extend");
            };
            values.extend(slots);
            Ok(Value::Null)
        }
        _ => type_error("method extend"),
    }
}

pub(crate) fn clear(
    receiver: &mut Value,
    args: &[Value],
    heap: Option<&mut HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("clear", args, 0)?;
    match receiver {
        Value::HeapRef(reference) => {
            let Some(heap) = heap else {
                return type_error("method clear");
            };
            let Some(HeapValue::Array(values)) = heap.heap.get_mut(*reference).ok() else {
                return type_error("method clear");
            };
            values.clear();
            Ok(Value::Null)
        }
        _ => type_error("method clear"),
    }
}
