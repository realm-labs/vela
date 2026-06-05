use crate::heap::{HeapSlot, HeapValue};
use crate::{ExecutionBudget, HeapExecution, Value, VmResult, value_from_heap_slot, values_equal};

use super::{
    array_values, expect_arity, index_out_of_bounds, index_value, make_array_value,
    make_string_value, string_value, type_error,
};

pub(crate) fn join(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("join", args, 1)?;
    let separator = string_value(&args[0], heap.as_deref(), "method join")?.to_owned();
    if let Value::HeapRef(reference) = receiver {
        let Some(HeapValue::Array(values)) =
            heap.as_deref().and_then(|heap| heap.heap.get(*reference))
        else {
            return type_error("method join");
        };
        let joined = join_heap_slots(values, heap.as_deref(), &separator)?;
        return make_string_value(joined, heap, budget, "method join");
    }
    let values = array_values(receiver, heap.as_deref(), "method join")?;
    let mut parts = Vec::with_capacity(values.len());
    for value in values {
        parts.push(string_value(&value, heap.as_deref(), "method join")?.to_owned());
    }
    make_string_value(parts.join(&separator), heap, budget, "method join")
}

pub(crate) fn distinct(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("distinct", args, 0)?;
    let values = array_values(receiver, heap.as_deref(), "method distinct")?;
    let mut distinct = Vec::new();
    'values: for value in values {
        for existing in &distinct {
            if values_equal(existing, &value, heap.as_deref())? {
                continue 'values;
            }
        }
        distinct.push(value);
    }
    make_array_value(distinct, heap, budget, "method distinct")
}

pub(crate) fn reverse(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("reverse", args, 0)?;
    if let Value::HeapRef(reference) = receiver {
        let Some(HeapValue::Array(values)) =
            heap.as_deref().and_then(|heap| heap.heap.get(*reference))
        else {
            return type_error("method reverse");
        };
        let values = reverse_heap_slots(values);
        return make_array_value(values, heap, budget, "method reverse");
    }
    let mut values = array_values(receiver, heap.as_deref(), "method reverse")?;
    values.reverse();
    make_array_value(values, heap, budget, "method reverse")
}

pub(crate) fn slice(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("slice", args, 2)?;
    if let Value::HeapRef(reference) = receiver {
        let Some(HeapValue::Array(values)) =
            heap.as_deref().and_then(|heap| heap.heap.get(*reference))
        else {
            return type_error("method slice");
        };
        let values = slice_heap_slots(values, args)?;
        return make_array_value(values, heap, budget, "method slice");
    }
    let values = array_values(receiver, heap.as_deref(), "method slice")?;
    let start = index_value(&args[0], "method slice")?;
    let end = index_value(&args[1], "method slice")?;
    if start > end {
        return type_error("method slice");
    }
    if start > values.len() {
        return Err(index_out_of_bounds(start, values.len()));
    }
    if end > values.len() {
        return Err(index_out_of_bounds(end, values.len()));
    }
    make_array_value(values[start..end].to_vec(), heap, budget, "method slice")
}

fn slice_heap_slots(values: &[HeapSlot], args: &[Value]) -> VmResult<Vec<Value>> {
    let start = index_value(&args[0], "method slice")?;
    let end = index_value(&args[1], "method slice")?;
    if start > end {
        return type_error("method slice");
    }
    if start > values.len() {
        return Err(index_out_of_bounds(start, values.len()));
    }
    if end > values.len() {
        return Err(index_out_of_bounds(end, values.len()));
    }
    Ok(values[start..end]
        .iter()
        .map(value_from_heap_slot)
        .collect())
}

fn reverse_heap_slots(values: &[HeapSlot]) -> Vec<Value> {
    values.iter().rev().map(value_from_heap_slot).collect()
}

fn join_heap_slots(
    values: &[HeapSlot],
    heap: Option<&HeapExecution<'_>>,
    separator: &str,
) -> VmResult<String> {
    let mut capacity = separator
        .len()
        .saturating_mul(values.len().saturating_sub(1));
    for value in values {
        capacity = capacity.saturating_add(heap_slot_string_value(value, heap)?.len());
    }

    let mut joined = String::with_capacity(capacity);
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            joined.push_str(separator);
        }
        joined.push_str(heap_slot_string_value(value, heap)?);
    }
    Ok(joined)
}

fn heap_slot_string_value<'a>(
    value: &'a HeapSlot,
    heap: Option<&'a HeapExecution<'_>>,
) -> VmResult<&'a str> {
    match value {
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::String(value)) => Ok(value),
            _ => type_error("method join"),
        },
        _ => type_error("method join"),
    }
}
