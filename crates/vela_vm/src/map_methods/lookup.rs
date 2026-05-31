use crate::heap::HeapValue;
use crate::option_result::option_value;
use crate::{HeapExecution, Value, VmResult, value_from_heap_slot};

use super::{expect_arity, map_key, type_error};

pub(crate) fn has(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    expect_arity("has", args, 1)?;
    let key = map_key(&args[0], heap)?;
    match receiver {
        Value::Map(values) => Ok(values.contains_key(&key)),
        Value::HeapRef(reference) => {
            let Some(HeapValue::Map(values)) = heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error("method has");
            };
            Ok(values.contains_key(&key))
        }
        _ => type_error("method has"),
    }
}

pub(crate) fn get(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("get", args, 1)?;
    let key = map_key(&args[0], heap)?;
    match receiver {
        Value::Map(values) => Ok(option_value(values.get(&key).cloned())),
        Value::HeapRef(reference) => {
            let Some(HeapValue::Map(values)) = heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error("method get");
            };
            Ok(option_value(values.get(&key).map(value_from_heap_slot)))
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
    let key = map_key(&args[0], heap)?;
    match receiver {
        Value::Map(values) => Ok(values.get(&key).cloned().unwrap_or_else(|| args[1].clone())),
        Value::HeapRef(reference) => {
            let Some(HeapValue::Map(values)) = heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error("method get_or");
            };
            Ok(values
                .get(&key)
                .map_or_else(|| args[1].clone(), value_from_heap_slot))
        }
        _ => type_error("method get_or"),
    }
}
