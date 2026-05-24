use crate::heap::HeapValue;
use crate::{HeapExecution, Value, VmError, VmErrorKind, VmResult, value_from_heap_slot};

pub(crate) fn call_method(
    receiver: &Value,
    method: &str,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    match method {
        "len" => {
            expect_no_args(method, args)?;
            len(receiver, heap).map(Value::Int)
        }
        "is_empty" => {
            expect_no_args(method, args)?;
            is_empty(receiver, heap).map(Value::Bool)
        }
        "has" => map_has(receiver, args, heap).map(Value::Bool),
        "get" => map_get(receiver, args, heap),
        "get_or" => map_get_or(receiver, args, heap),
        _ => Err(VmError::new(VmErrorKind::UnknownMethod {
            method: method.to_owned(),
        })),
    }
}

fn len(receiver: &Value, heap: Option<&HeapExecution<'_>>) -> VmResult<i64> {
    match receiver {
        Value::String(value) => usize_to_i64(value.chars().count(), "method len"),
        Value::Array(values) => usize_to_i64(values.len(), "method len"),
        Value::Map(values) => usize_to_i64(values.len(), "method len"),
        Value::Range(range) => range.len().ok_or_else(|| {
            VmError::new(VmErrorKind::TypeMismatch {
                operation: "method len",
            })
        }),
        Value::HeapRef(reference) => {
            let Some(value) = heap.and_then(|heap| heap.heap.get(*reference)) else {
                return type_error("method len");
            };
            match value {
                HeapValue::String(value) => usize_to_i64(value.chars().count(), "method len"),
                HeapValue::Array(values) | HeapValue::Set(values) => {
                    usize_to_i64(values.len(), "method len")
                }
                HeapValue::Map(values)
                | HeapValue::Record { fields: values, .. }
                | HeapValue::Enum { fields: values, .. } => {
                    usize_to_i64(values.len(), "method len")
                }
            }
        }
        Value::Record { fields, .. } | Value::Enum { fields, .. } => {
            usize_to_i64(fields.len(), "method len")
        }
        _ => type_error("method len"),
    }
}

fn is_empty(receiver: &Value, heap: Option<&HeapExecution<'_>>) -> VmResult<bool> {
    match receiver {
        Value::String(value) => Ok(value.is_empty()),
        Value::Array(values) => Ok(values.is_empty()),
        Value::Map(values) => Ok(values.is_empty()),
        Value::Range(range) => Ok(range.is_empty()),
        Value::HeapRef(reference) => {
            let Some(value) = heap.and_then(|heap| heap.heap.get(*reference)) else {
                return type_error("method is_empty");
            };
            match value {
                HeapValue::String(value) => Ok(value.is_empty()),
                HeapValue::Array(values) | HeapValue::Set(values) => Ok(values.is_empty()),
                HeapValue::Map(values)
                | HeapValue::Record { fields: values, .. }
                | HeapValue::Enum { fields: values, .. } => Ok(values.is_empty()),
            }
        }
        Value::Record { fields, .. } | Value::Enum { fields, .. } => Ok(fields.is_empty()),
        _ => type_error("method is_empty"),
    }
}

fn map_has(receiver: &Value, args: &[Value], heap: Option<&HeapExecution<'_>>) -> VmResult<bool> {
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

fn map_get(receiver: &Value, args: &[Value], heap: Option<&HeapExecution<'_>>) -> VmResult<Value> {
    expect_arity("get", args, 1)?;
    let key = map_key(&args[0], heap)?;
    match receiver {
        Value::Map(values) => Ok(values.get(&key).cloned().unwrap_or(Value::Null)),
        Value::HeapRef(reference) => {
            let Some(HeapValue::Map(values)) = heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error("method get");
            };
            Ok(values.get(&key).map_or(Value::Null, value_from_heap_slot))
        }
        _ => type_error("method get"),
    }
}

fn map_get_or(
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

fn expect_no_args(method: &str, args: &[Value]) -> VmResult<()> {
    expect_arity(method, args, 0)
}

fn expect_arity(method: &str, args: &[Value], expected: usize) -> VmResult<()> {
    if args.len() == expected {
        return Ok(());
    }
    Err(VmError::new(VmErrorKind::ArityMismatch {
        name: method.to_owned(),
        expected,
        actual: args.len(),
    }))
}

fn map_key(value: &Value, heap: Option<&HeapExecution<'_>>) -> VmResult<String> {
    match value {
        Value::String(key) => Ok(key.clone()),
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::String(key)) => Ok(key.clone()),
            _ => type_error("map key"),
        },
        _ => type_error("map key"),
    }
}

fn usize_to_i64(value: usize, operation: &'static str) -> VmResult<i64> {
    i64::try_from(value).map_err(|_| VmError::new(VmErrorKind::TypeMismatch { operation }))
}

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}
