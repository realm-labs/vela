use crate::heap::HeapValue;
use crate::{
    EqualityRuntime, ExecutionBudget, HeapExecution, Value, VmResult, stored_runtime_value,
    values_equal, values_equal_with_traits,
};

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
        let joined = join_runtime_values(values, heap.as_deref(), &separator)?;
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

pub(crate) fn distinct_with_equality(
    receiver: &Value,
    args: &[Value],
    runtime: &mut EqualityRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("distinct", args, 0)?;
    let values = array_values(receiver, runtime.heap.as_deref(), "method distinct")?;
    let mut distinct = Vec::new();
    'values: for value in values {
        for existing in &distinct {
            if values_equal_with_traits(existing, &value, runtime)? {
                continue 'values;
            }
        }
        distinct.push(value);
    }
    make_array_value(
        distinct,
        &mut runtime.heap,
        &mut runtime.budget,
        "method distinct",
    )
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
        let values = reverse_runtime_values(values);
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
        let values = slice_runtime_values(values, args)?;
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

fn slice_runtime_values(values: &[Value], args: &[Value]) -> VmResult<Vec<Value>> {
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
        .map(stored_runtime_value)
        .collect())
}

fn reverse_runtime_values(values: &[Value]) -> Vec<Value> {
    values.iter().rev().map(stored_runtime_value).collect()
}

fn join_runtime_values(
    values: &[Value],
    heap: Option<&HeapExecution<'_>>,
    separator: &str,
) -> VmResult<String> {
    let mut capacity = separator
        .len()
        .saturating_mul(values.len().saturating_sub(1));
    for value in values {
        capacity = capacity.saturating_add(runtime_string_value(value, heap)?.len());
    }

    let mut joined = String::with_capacity(capacity);
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            joined.push_str(separator);
        }
        joined.push_str(runtime_string_value(value, heap)?);
    }
    Ok(joined)
}

fn runtime_string_value<'a>(
    value: &'a Value,
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
