use crate::heap::HeapValue;
use crate::{
    ExecutionBudget, HeapExecution, Value, VmError, VmErrorKind, VmResult, stored_runtime_value,
    value_key::ValueKey,
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

pub(crate) fn contains_by_key(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    expect_arity("contains", args, 1)?;
    let needle = ValueKey::from_value(&args[0], heap, "method contains")?;
    let values = super::array_values(receiver, heap, "method contains")?;
    for value in values {
        if ValueKey::from_value(&value, heap, "method contains")? == needle {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn index_of_by_key(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("index_of", args, 1)?;
    let index = {
        let heap_ref = heap.as_deref();
        let needle = ValueKey::from_value(&args[0], heap_ref, "method index_of")?;
        let values = super::array_values(receiver, heap_ref, "method index_of")?;
        let mut found = None;
        for (index, value) in values.iter().enumerate() {
            if ValueKey::from_value(value, heap_ref, "method index_of")? == needle {
                found = Some(index);
                break;
            }
        }
        found
    };
    if let Some(index) = index {
        return index_option(index, heap, budget);
    }
    option_value("None", None, heap, budget)
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
