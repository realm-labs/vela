use crate::heap::HeapValue;
use crate::{
    EqualityRuntime, ExecutionBudget, HeapExecution, Value, VmError, VmErrorKind, VmResult,
    stored_runtime_value, values_equal, values_equal_with_traits,
};

use super::{expect_arity, option_value, type_error};

pub(crate) fn first(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("first", args, 0)?;
    first_value(receiver, heap, budget)
}

pub(crate) fn last(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("last", args, 0)?;
    last_value(receiver, heap, budget)
}

pub(crate) fn contains(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    expect_arity("contains", args, 1)?;
    array_contains(receiver, &args[0], heap)
}

pub(crate) fn contains_with_equality(
    receiver: &Value,
    args: &[Value],
    runtime: &mut EqualityRuntime<'_, '_, '_>,
) -> VmResult<bool> {
    expect_arity("contains", args, 1)?;
    let values = super::array_values(receiver, runtime.heap.as_deref(), "method contains")?;
    for value in values {
        if values_equal_with_traits(&value, &args[0], runtime)? {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn index_of(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("index_of", args, 1)?;
    array_index_of(receiver, &args[0], heap, budget)
}

pub(crate) fn index_of_with_equality(
    receiver: &Value,
    args: &[Value],
    runtime: &mut EqualityRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("index_of", args, 1)?;
    let values = super::array_values(receiver, runtime.heap.as_deref(), "method index_of")?;
    for (index, value) in values.iter().enumerate() {
        if values_equal_with_traits(value, &args[0], runtime)? {
            return index_option(index, &mut runtime.heap, &mut runtime.budget);
        }
    }
    option_value("None", None, &mut runtime.heap, &mut runtime.budget)
}

fn first_value(
    receiver: &Value,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    match receiver {
        Value::HeapRef(reference) => {
            let Some(HeapValue::Array(values)) =
                heap.as_deref().and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error("method first");
            };
            let payload = values.first().map(stored_runtime_value);
            if payload.is_some() {
                option_value("Some", payload, heap, budget)
            } else {
                option_value("None", None, heap, budget)
            }
        }
        _ => type_error("method first"),
    }
}

fn last_value(
    receiver: &Value,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    match receiver {
        Value::HeapRef(reference) => {
            let Some(HeapValue::Array(values)) =
                heap.as_deref().and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error("method last");
            };
            let payload = values.last().map(stored_runtime_value);
            if payload.is_some() {
                option_value("Some", payload, heap, budget)
            } else {
                option_value("None", None, heap, budget)
            }
        }
        _ => type_error("method last"),
    }
}

fn array_contains(
    receiver: &Value,
    needle: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    match receiver {
        Value::HeapRef(reference) => {
            let Some(HeapValue::Array(values)) = heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error("method contains");
            };
            for value in values {
                if values_equal(&stored_runtime_value(value), needle, heap)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        _ => type_error("method contains"),
    }
}

fn array_index_of(
    receiver: &Value,
    needle: &Value,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    match receiver {
        Value::HeapRef(reference) => {
            let Some(HeapValue::Array(values)) =
                heap.as_deref().and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error("method index_of");
            };
            for (index, value) in values.iter().enumerate() {
                if values_equal(&stored_runtime_value(value), needle, heap.as_deref())? {
                    return index_option(index, heap, budget);
                }
            }
            option_value("None", None, heap, budget)
        }
        _ => type_error("method index_of"),
    }
}

fn index_option(
    index: usize,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    let index = i64::try_from(index).map_err(|_| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "method index_of",
        })
    })?;
    option_value("Some", Some(Value::I64(index)), heap, budget)
}
