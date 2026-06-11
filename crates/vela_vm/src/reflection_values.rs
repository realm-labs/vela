use std::collections::BTreeMap;

use vela_host::value::HostValue;
use vela_reflect as reflect;

use crate::heap::HeapValue;
use crate::owned_value::OwnedValue;
use crate::script_object::ScriptFields;
use crate::{HeapExecution, Value, VmError, VmErrorKind, VmResult};

pub(crate) fn value_to_reflect(
    value: &OwnedValue,
    operation: &'static str,
) -> VmResult<reflect::value::ReflectValue> {
    match value {
        OwnedValue::HostRef(host_ref) => Ok(reflect::value::ReflectValue::HostRef(*host_ref)),
        OwnedValue::Array(values) => values
            .iter()
            .map(|value| value_to_reflect(value, operation))
            .collect::<VmResult<Vec<_>>>()
            .map(reflect::value::ReflectValue::Array),
        OwnedValue::Map(values) => {
            let values = values
                .iter()
                .map(|(key, value)| Ok((key.clone(), value_to_reflect(value, operation)?)))
                .collect::<VmResult<BTreeMap<_, _>>>()?;
            Ok(reflect::value::ReflectValue::Record(values))
        }
        OwnedValue::Set(values) => values
            .iter()
            .map(|value| value_to_reflect(value, operation))
            .collect::<VmResult<Vec<_>>>()
            .map(reflect::value::ReflectValue::Set),
        OwnedValue::Record {
            type_name,
            fields: values,
        } => {
            let values = values
                .iter()
                .map(|(key, value)| Ok((key.to_owned(), value_to_reflect(value, operation)?)))
                .collect::<VmResult<BTreeMap<_, _>>>()?;
            Ok(reflect::value::ReflectValue::ScriptRecord {
                type_name: type_name.clone(),
                fields: values,
            })
        }
        OwnedValue::Enum {
            enum_name,
            variant,
            fields: values,
        } => {
            let values = values
                .iter()
                .map(|(key, value)| Ok((key.to_owned(), value_to_reflect(value, operation)?)))
                .collect::<VmResult<BTreeMap<_, _>>>()?;
            Ok(reflect::value::ReflectValue::ScriptEnum {
                enum_name: enum_name.clone(),
                variant: variant.clone(),
                fields: values,
            })
        }
        OwnedValue::Closure(_) => Ok(reflect::value::ReflectValue::Closure),
        OwnedValue::Range(_) => Ok(reflect::value::ReflectValue::Range),
        OwnedValue::Missing
        | OwnedValue::Bytes(_)
        | OwnedValue::PathProxy(_)
        | OwnedValue::Iterator(_) => Err(type_error(operation)),
        OwnedValue::Null | OwnedValue::Bool(_) | OwnedValue::Scalar(_) | OwnedValue::String(_) => {
            Ok(reflect::value::ReflectValue::Host(owned_to_host(
                value, operation,
            )?))
        }
    }
}

pub(crate) fn runtime_value_to_reflect(
    value: &Value,
    heap: &HeapExecution<'_>,
    operation: &'static str,
) -> VmResult<reflect::value::ReflectValue> {
    match value {
        Value::Missing => Err(type_error(operation)),
        Value::Null => Ok(reflect::value::ReflectValue::Host(HostValue::Null)),
        Value::Bool(value) => Ok(reflect::value::ReflectValue::Host(HostValue::Bool(*value))),
        Value::Scalar(value) => Ok(reflect::value::ReflectValue::Host(HostValue::Scalar(
            *value,
        ))),
        Value::Range(_) => Ok(reflect::value::ReflectValue::Range),
        Value::HostRef(host_ref) => Ok(reflect::value::ReflectValue::HostRef(*host_ref)),
        Value::HeapRef(reference) => match heap.heap.get(*reference) {
            Some(HeapValue::String(value)) => Ok(reflect::value::ReflectValue::Host(
                HostValue::String(value.clone()),
            )),
            Some(HeapValue::Bytes(_)) => Err(type_error(operation)),
            Some(HeapValue::Array(values)) => values
                .iter()
                .map(|value| runtime_value_to_reflect(value, heap, operation))
                .collect::<VmResult<Vec<_>>>()
                .map(reflect::value::ReflectValue::Array),
            Some(HeapValue::Map(values)) => {
                let values = values
                    .iter()
                    .map(|(key, value)| {
                        Ok((
                            key.clone(),
                            runtime_value_to_reflect(value, heap, operation)?,
                        ))
                    })
                    .collect::<VmResult<BTreeMap<_, _>>>()?;
                Ok(reflect::value::ReflectValue::Record(values))
            }
            Some(HeapValue::Set(values)) => values
                .iter()
                .map(|value| runtime_value_to_reflect(value, heap, operation))
                .collect::<VmResult<Vec<_>>>()
                .map(reflect::value::ReflectValue::Set),
            Some(HeapValue::Record {
                type_name, fields, ..
            }) => {
                let fields = fields
                    .iter()
                    .map(|(key, value)| {
                        Ok((
                            key.to_owned(),
                            runtime_value_to_reflect(value, heap, operation)?,
                        ))
                    })
                    .collect::<VmResult<BTreeMap<_, _>>>()?;
                Ok(reflect::value::ReflectValue::ScriptRecord {
                    type_name: type_name.clone(),
                    fields,
                })
            }
            Some(HeapValue::Enum {
                enum_name,
                variant,
                fields,
                ..
            }) => {
                let fields = fields
                    .iter()
                    .map(|(key, value)| {
                        Ok((
                            key.to_owned(),
                            runtime_value_to_reflect(value, heap, operation)?,
                        ))
                    })
                    .collect::<VmResult<BTreeMap<_, _>>>()?;
                Ok(reflect::value::ReflectValue::ScriptEnum {
                    enum_name: enum_name.clone(),
                    variant: variant.clone(),
                    fields,
                })
            }
            Some(HeapValue::Closure(_)) => Ok(reflect::value::ReflectValue::Closure),
            Some(HeapValue::Iterator(_) | HeapValue::PathProxy(_)) | None => {
                Err(type_error(operation))
            }
        },
    }
}

pub(crate) fn value_from_reflect(value: reflect::value::ReflectValue) -> VmResult<OwnedValue> {
    match value {
        reflect::value::ReflectValue::Host(value) => Ok(host_to_owned(value)),
        reflect::value::ReflectValue::HostRef(host_ref) => Ok(OwnedValue::HostRef(host_ref)),
        reflect::value::ReflectValue::Closure => Err(type_error("reflect closure conversion")),
        reflect::value::ReflectValue::Range => Err(type_error("reflect range conversion")),
        reflect::value::ReflectValue::Array(values) => values
            .into_iter()
            .map(value_from_reflect)
            .collect::<VmResult<Vec<_>>>()
            .map(OwnedValue::Array),
        reflect::value::ReflectValue::Record(values) => {
            let values = values
                .into_iter()
                .map(|(key, value)| Ok((key, value_from_reflect(value)?)))
                .collect::<VmResult<BTreeMap<_, _>>>()?;
            Ok(OwnedValue::Map(values))
        }
        reflect::value::ReflectValue::Set(values) => values
            .into_iter()
            .map(value_from_reflect)
            .collect::<VmResult<Vec<_>>>()
            .map(OwnedValue::Set),
        reflect::value::ReflectValue::ScriptRecord { type_name, fields } => {
            let fields = fields
                .into_iter()
                .map(|(key, value)| Ok((key, value_from_reflect(value)?)))
                .collect::<VmResult<BTreeMap<_, _>>>()?;
            Ok(OwnedValue::Record {
                fields: ScriptFields::from_pairs(&type_name, fields),
                type_name,
            })
        }
        reflect::value::ReflectValue::ScriptEnum {
            enum_name,
            variant,
            fields,
        } => {
            let fields = fields
                .into_iter()
                .map(|(key, value)| Ok((key, value_from_reflect(value)?)))
                .collect::<VmResult<BTreeMap<_, _>>>()?;
            Ok(OwnedValue::Enum {
                fields: ScriptFields::from_pairs(&format!("{enum_name}::{variant}"), fields),
                enum_name,
                variant,
            })
        }
    }
}

fn owned_to_host(value: &OwnedValue, operation: &'static str) -> VmResult<HostValue> {
    match value {
        OwnedValue::Null => Ok(HostValue::Null),
        OwnedValue::Bool(value) => Ok(HostValue::Bool(*value)),
        OwnedValue::Scalar(value) => Ok(HostValue::Scalar(*value)),
        OwnedValue::String(value) => Ok(HostValue::String(value.clone())),
        OwnedValue::HostRef(value) => Ok(HostValue::HostRef(*value)),
        OwnedValue::Missing
        | OwnedValue::Bytes(_)
        | OwnedValue::Array(_)
        | OwnedValue::Map(_)
        | OwnedValue::Set(_)
        | OwnedValue::Record { .. }
        | OwnedValue::Enum { .. }
        | OwnedValue::Closure(_)
        | OwnedValue::Range(_)
        | OwnedValue::PathProxy(_)
        | OwnedValue::Iterator(_) => Err(type_error(operation)),
    }
}

fn host_to_owned(value: HostValue) -> OwnedValue {
    match value {
        HostValue::Null => OwnedValue::Null,
        HostValue::Bool(value) => OwnedValue::Bool(value),
        HostValue::Scalar(value) => OwnedValue::Scalar(value),
        HostValue::String(value) => OwnedValue::String(value),
        HostValue::HostRef(value) => OwnedValue::HostRef(value),
    }
}

fn type_error(operation: &'static str) -> VmError {
    VmError::new(VmErrorKind::TypeMismatch { operation })
}
