use vela_host::value::HostValue;

use crate::heap::HeapValue;
use crate::{HeapExecution, Value, VmError, VmErrorKind, VmResult};

pub(crate) fn value_from_host(value: HostValue) -> Value {
    match value {
        HostValue::Null => Value::Null,
        HostValue::Bool(value) => Value::Bool(value),
        HostValue::Int(value) => Value::Int(value),
        HostValue::Float(value) => Value::Float(value),
        HostValue::HostRef(value) => Value::HostRef(value),
        HostValue::String(_)
        | HostValue::Array(_)
        | HostValue::Map(_)
        | HostValue::Record { .. }
        | HostValue::Enum { .. } => Value::Missing,
    }
}

pub(crate) fn value_to_host(
    value: &Value,
    operation: &'static str,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<HostValue> {
    match value {
        Value::Null => Ok(HostValue::Null),
        Value::Bool(value) => Ok(HostValue::Bool(*value)),
        Value::Int(value) => Ok(HostValue::Int(*value)),
        Value::Float(value) => Ok(HostValue::Float(*value)),
        Value::HostRef(value) => Ok(HostValue::HostRef(*value)),
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::String(value)) => Ok(HostValue::String(value.clone())),
            Some(
                HeapValue::Array(_)
                | HeapValue::Map(_)
                | HeapValue::Set(_)
                | HeapValue::Record { .. }
                | HeapValue::Enum { .. },
            ) => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
            _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
        },
        Value::Range(_) | Value::Missing => {
            Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
        }
    }
}
