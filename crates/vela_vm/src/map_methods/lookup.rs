use crate::heap::HeapValue;
use crate::option_result::option_value;
use crate::{ExecutionBudget, HeapExecution, Value, VmResult};

use super::{expect_arity, type_error};

pub(crate) fn has(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    contains_key_with_name(receiver, args, heap, "has", "method has")
}

pub(crate) fn contains_key(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    contains_key_with_name(receiver, args, heap, "contains_key", "method contains_key")
}

fn contains_key_with_name(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
    method: &str,
    operation: &'static str,
) -> VmResult<bool> {
    expect_arity(method, args, 1)?;
    match receiver {
        Value::HeapRef(reference) => {
            let Some(HeapValue::Map(values)) = heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error(operation);
            };
            values.contains_key_value(&args[0], heap, operation)
        }
        _ => type_error(operation),
    }
}

pub(crate) fn get(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("get", args, 1)?;
    match receiver {
        Value::HeapRef(reference) => {
            let payload = {
                let Some(HeapValue::Map(values)) =
                    heap.as_deref().and_then(|heap| heap.heap.get(*reference))
                else {
                    return type_error("method get");
                };
                values.get(&args[0], heap.as_deref(), "method get")?
            };
            let Some(heap) = heap.as_deref_mut() else {
                return type_error("method get");
            };
            option_value(payload, heap, budget.as_deref_mut())
        }
        _ => type_error("method get"),
    }
}

pub(crate) fn get_or(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("get_or", args, 2)?;
    match receiver {
        Value::HeapRef(reference) => {
            let Some(HeapValue::Map(values)) = heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error("method get_or");
            };
            Ok(values
                .get(&args[0], heap, "method get_or")?
                .unwrap_or(args[1]))
        }
        _ => type_error("method get_or"),
    }
}
