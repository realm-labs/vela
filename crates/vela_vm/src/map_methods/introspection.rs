use crate::heap::HeapValue;
use crate::{HeapExecution, Value, VmResult, value_from_heap_slot};

use super::{expect_no_args, map_entry, type_error};

pub(crate) fn keys(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_no_args("keys", args)?;
    match receiver {
        Value::Map(values) => Ok(Value::Array(
            values
                .keys()
                .map(|key| Value::String(key.clone()))
                .collect(),
        )),
        Value::HeapRef(reference) => {
            let Some(HeapValue::Map(values)) = heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error("method keys");
            };
            Ok(Value::Array(
                values
                    .keys()
                    .map(|key| Value::String(key.clone()))
                    .collect(),
            ))
        }
        _ => type_error("method keys"),
    }
}

pub(crate) fn values(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_no_args("values", args)?;
    match receiver {
        Value::Map(values) => Ok(Value::Array(values.values().cloned().collect())),
        Value::HeapRef(reference) => {
            let Some(HeapValue::Map(values)) = heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error("method values");
            };
            Ok(Value::Array(
                values.values().map(value_from_heap_slot).collect(),
            ))
        }
        _ => type_error("method values"),
    }
}

pub(crate) fn entries(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_no_args("entries", args)?;
    match receiver {
        Value::Map(values) => Ok(Value::Array(
            values
                .iter()
                .map(|(key, value)| map_entry(key, value.clone()))
                .collect(),
        )),
        Value::HeapRef(reference) => {
            let Some(HeapValue::Map(values)) = heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error("method entries");
            };
            Ok(Value::Array(
                values
                    .iter()
                    .map(|(key, value)| map_entry(key, value_from_heap_slot(value)))
                    .collect(),
            ))
        }
        _ => type_error("method entries"),
    }
}
