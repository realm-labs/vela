use crate::heap::HeapValue;
use crate::{HeapExecution, Value, VmError, VmErrorKind, VmResult};

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

fn expect_no_args(method: &str, args: &[Value]) -> VmResult<()> {
    if args.is_empty() {
        return Ok(());
    }
    Err(VmError::new(VmErrorKind::ArityMismatch {
        name: method.to_owned(),
        expected: 0,
        actual: args.len(),
    }))
}

fn usize_to_i64(value: usize, operation: &'static str) -> VmResult<i64> {
    i64::try_from(value).map_err(|_| VmError::new(VmErrorKind::TypeMismatch { operation }))
}

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}
