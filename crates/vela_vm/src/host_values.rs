use vela_host::value::HostValue;

use crate::heap::HeapValue;
use crate::{HeapExecution, Value, VmError, VmErrorKind, VmResult};

pub(crate) fn value_from_host(value: HostValue) -> Value {
    match value {
        HostValue::Null => Value::Null,
        HostValue::Bool(value) => Value::Bool(value),
        HostValue::Scalar(value) => Value::from_scalar(value),
        HostValue::HostRef(value) => Value::HostRef(value),
        HostValue::String(_) | HostValue::Bytes(_) => Value::Missing,
    }
}

pub(crate) fn value_to_host(
    value: &Value,
    operation: &'static str,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<HostValue> {
    if let Some(value) = value.as_scalar() {
        return Ok(HostValue::Scalar(value));
    }
    match value {
        Value::Null => Ok(HostValue::Null),
        Value::Bool(value) => Ok(HostValue::Bool(*value)),
        Value::HostRef(value) => Ok(HostValue::HostRef(*value)),
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::String(value)) => Ok(HostValue::String(value.clone())),
            Some(HeapValue::Bytes(value)) => Ok(HostValue::Bytes(value.clone())),
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
        _ => unreachable!("scalar values return before host conversion match"),
    }
}
