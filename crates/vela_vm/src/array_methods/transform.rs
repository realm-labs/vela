use crate::heap::{HeapSlot, HeapValue};
use crate::{HeapExecution, Value, VmResult, value_from_heap_slot, values_equal};

use super::{
    array_values, expect_arity, index_out_of_bounds, index_value, string_value, type_error,
};

pub(crate) fn join(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("join", args, 1)?;
    let values = array_values(receiver, heap, "method join")?;
    let separator = string_value(&args[0], heap, "method join")?;
    let mut parts = Vec::with_capacity(values.len());
    for value in values {
        parts.push(string_value(&value, heap, "method join")?.to_owned());
    }
    Ok(Value::String(parts.join(separator)))
}

pub(crate) fn distinct(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("distinct", args, 0)?;
    let values = array_values(receiver, heap, "method distinct")?;
    let mut distinct = Vec::new();
    'values: for value in values {
        for existing in &distinct {
            if values_equal(existing, &value, heap)? {
                continue 'values;
            }
        }
        distinct.push(value);
    }
    Ok(Value::Array(distinct))
}

pub(crate) fn reverse(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("reverse", args, 0)?;
    let mut values = array_values(receiver, heap, "method reverse")?;
    values.reverse();
    Ok(Value::Array(values))
}

pub(crate) fn slice(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("slice", args, 2)?;
    if let Value::HeapRef(reference) = receiver {
        let Some(HeapValue::Array(values)) = heap.and_then(|heap| heap.heap.get(*reference)) else {
            return type_error("method slice");
        };
        return slice_heap_slots(values, args);
    }
    let values = array_values(receiver, heap, "method slice")?;
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
    Ok(Value::Array(values[start..end].to_vec()))
}

fn slice_heap_slots(values: &[HeapSlot], args: &[Value]) -> VmResult<Value> {
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
    Ok(Value::Array(
        values[start..end]
            .iter()
            .map(value_from_heap_slot)
            .collect(),
    ))
}
