use std::collections::BTreeMap;

use vela_host::HostValue;

use crate::heap::HeapValue;
use crate::script_object::ScriptFields;
use crate::{HeapExecution, Value, VmError, VmErrorKind, VmResult, value_from_heap_slot};

pub(crate) fn value_from_host(value: HostValue) -> Value {
    match value {
        HostValue::Null => Value::Null,
        HostValue::Bool(value) => Value::Bool(value),
        HostValue::Int(value) => Value::Int(value),
        HostValue::Float(value) => Value::Float(value),
        HostValue::String(value) => Value::String(value),
        HostValue::Array(values) => Value::Array(values.into_iter().map(value_from_host).collect()),
        HostValue::Map(values) => Value::Map(
            values
                .into_iter()
                .map(|(key, value)| (key, value_from_host(value)))
                .collect(),
        ),
        HostValue::Record { type_name, fields } => {
            let fields = fields
                .into_iter()
                .map(|(key, value)| (key, value_from_host(value)));
            Value::Record {
                fields: ScriptFields::from_pairs(&type_name, fields),
                type_name,
            }
        }
        HostValue::Enum {
            enum_name,
            variant,
            fields,
        } => {
            let owner = enum_variant_owner(&enum_name, &variant);
            let fields = fields
                .into_iter()
                .map(|(key, value)| (key, value_from_host(value)));
            Value::Enum {
                fields: ScriptFields::from_pairs(&owner, fields),
                enum_name,
                variant,
            }
        }
        HostValue::HostRef(value) => Value::HostRef(value),
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
        Value::String(value) => Ok(HostValue::String(value.clone())),
        Value::Array(values) => values
            .iter()
            .map(|value| value_to_host(value, operation, heap))
            .collect::<VmResult<Vec<_>>>()
            .map(HostValue::Array),
        Value::Map(values) => values
            .iter()
            .map(|(key, value)| Ok((key.clone(), value_to_host(value, operation, heap)?)))
            .collect::<VmResult<BTreeMap<_, _>>>()
            .map(HostValue::Map),
        Value::Record { type_name, fields } => fields
            .iter()
            .map(|(key, value)| Ok((key.to_owned(), value_to_host(value, operation, heap)?)))
            .collect::<VmResult<BTreeMap<_, _>>>()
            .map(|fields| HostValue::Record {
                type_name: type_name.clone(),
                fields,
            }),
        Value::Enum {
            enum_name,
            variant,
            fields,
        } => fields
            .iter()
            .map(|(key, value)| Ok((key.to_owned(), value_to_host(value, operation, heap)?)))
            .collect::<VmResult<BTreeMap<_, _>>>()
            .map(|fields| HostValue::Enum {
                enum_name: enum_name.clone(),
                variant: variant.clone(),
                fields,
            }),
        Value::HostRef(value) => Ok(HostValue::HostRef(*value)),
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::String(value)) => Ok(HostValue::String(value.clone())),
            Some(HeapValue::Array(values)) => values
                .iter()
                .map(value_from_heap_slot)
                .map(|value| value_to_host(&value, operation, heap))
                .collect::<VmResult<Vec<_>>>()
                .map(HostValue::Array),
            Some(HeapValue::Map(values)) => values
                .iter()
                .map(|(key, value)| {
                    let value = value_from_heap_slot(value);
                    Ok((key.clone(), value_to_host(&value, operation, heap)?))
                })
                .collect::<VmResult<BTreeMap<_, _>>>()
                .map(HostValue::Map),
            Some(HeapValue::Record { type_name, fields }) => fields
                .iter()
                .map(|(key, value)| {
                    let value = value_from_heap_slot(value);
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
                    let value = value_from_heap_slot(value);
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
        Value::Set(_)
        | Value::Range(_)
        | Value::Closure(_)
        | Value::Iterator(_)
        | Value::Missing => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
    }
}

fn enum_variant_owner(enum_name: &str, variant: &str) -> String {
    format!("{enum_name}.{variant}")
}
