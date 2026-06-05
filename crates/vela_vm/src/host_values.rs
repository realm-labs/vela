use std::collections::BTreeMap;

use vela_host::value::HostValue;

use crate::heap::HeapValue;
use crate::{HeapExecution, Value, VmError, VmErrorKind, VmResult, stored_runtime_value};

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
            Some(HeapValue::Array(values)) => values
                .iter()
                .map(stored_runtime_value)
                .map(|value| value_to_host(&value, operation, heap))
                .collect::<VmResult<Vec<_>>>()
                .map(HostValue::Array),
            Some(HeapValue::Map(values)) => values
                .iter()
                .map(|(key, value)| {
                    let value = stored_runtime_value(value);
                    Ok((key.clone(), value_to_host(&value, operation, heap)?))
                })
                .collect::<VmResult<BTreeMap<_, _>>>()
                .map(HostValue::Map),
            Some(HeapValue::Record { type_name, fields }) => fields
                .iter()
                .map(|(key, value)| {
                    let value = stored_runtime_value(value);
                    Ok((key.to_owned(), value_to_host(&value, operation, heap)?))
                })
                .collect::<VmResult<BTreeMap<_, _>>>()
                .map(|fields| HostValue::Record {
                    type_name: type_name.clone(),
                    fields,
                }),
            Some(HeapValue::Enum {
                enum_name,
                variant,
                fields,
            }) => fields
                .iter()
                .map(|(key, value)| {
                    let value = stored_runtime_value(value);
                    Ok((key.to_owned(), value_to_host(&value, operation, heap)?))
                })
                .collect::<VmResult<BTreeMap<_, _>>>()
                .map(|fields| HostValue::Enum {
                    enum_name: enum_name.clone(),
                    variant: variant.clone(),
                    fields,
                }),
            _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
        },
        Value::Range(_) | Value::Missing => {
            Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
        }
    }
}
