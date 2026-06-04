use crate::heap::HeapValue;
use crate::{
    HeapExecution, Value, VmError, VmErrorKind, VmResult, value_from_heap_slot, values_equal,
};

use super::{expect_arity, option_value, type_error};

pub(crate) fn first(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("first", args, 0)?;
    first_value(receiver, heap)
}

pub(crate) fn last(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("last", args, 0)?;
    last_value(receiver, heap)
}

pub(crate) fn contains(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    expect_arity("contains", args, 1)?;
    array_contains(receiver, &args[0], heap)
}

pub(crate) fn index_of(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("index_of", args, 1)?;
    array_index_of(receiver, &args[0], heap)
}

fn first_value(receiver: &Value, heap: Option<&HeapExecution<'_>>) -> VmResult<Value> {
    match receiver {
        Value::Array(values) => Ok(values.first().cloned().map_or_else(
            || option_value("None", None),
            |value| option_value("Some", Some(value)),
        )),
        Value::HeapRef(reference) => {
            let Some(HeapValue::Array(values)) = heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error("method first");
            };
            Ok(values.first().map(value_from_heap_slot).map_or_else(
                || option_value("None", None),
                |value| option_value("Some", Some(value)),
            ))
        }
        _ => type_error("method first"),
    }
}

fn last_value(receiver: &Value, heap: Option<&HeapExecution<'_>>) -> VmResult<Value> {
    match receiver {
        Value::Array(values) => Ok(values.last().cloned().map_or_else(
            || option_value("None", None),
            |value| option_value("Some", Some(value)),
        )),
        Value::HeapRef(reference) => {
            let Some(HeapValue::Array(values)) = heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error("method last");
            };
            Ok(values.last().map(value_from_heap_slot).map_or_else(
                || option_value("None", None),
                |value| option_value("Some", Some(value)),
            ))
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
        Value::Array(values) => {
            for value in values {
                if values_equal(value, needle, heap)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Value::HeapRef(reference) => {
            let Some(HeapValue::Array(values)) = heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error("method contains");
            };
            for value in values {
                if values_equal(&value_from_heap_slot(value), needle, heap)? {
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
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    match receiver {
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                if values_equal(value, needle, heap)? {
                    return index_option(index);
                }
            }
            Ok(option_value("None", None))
        }
        Value::HeapRef(reference) => {
            let Some(HeapValue::Array(values)) = heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error("method index_of");
            };
            for (index, value) in values.iter().enumerate() {
                if values_equal(&value_from_heap_slot(value), needle, heap)? {
                    return index_option(index);
                }
            }
            Ok(option_value("None", None))
        }
        _ => type_error("method index_of"),
    }
}

fn index_option(index: usize) -> VmResult<Value> {
    let index = i64::try_from(index).map_err(|_| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "method index_of",
        })
    })?;
    Ok(option_value("Some", Some(Value::Int(index))))
}
