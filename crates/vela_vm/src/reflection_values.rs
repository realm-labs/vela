use std::collections::BTreeMap;

use vela_host::value::HostValue;
use vela_reflect as reflect;

use crate::owned_value::OwnedValue;
use crate::script_object::ScriptFields;
use crate::{VmError, VmErrorKind, VmResult};

pub(crate) fn value_to_reflect(
    value: &OwnedValue,
    operation: &'static str,
) -> VmResult<reflect::value::ReflectValue> {
    match value {
        OwnedValue::HostRef(host_ref) => Ok(reflect::value::ReflectValue::HostRef(*host_ref)),
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
        OwnedValue::Missing | OwnedValue::PathProxy(_) | OwnedValue::Iterator(_) => {
            Err(type_error(operation))
        }
        OwnedValue::Null
        | OwnedValue::Bool(_)
        | OwnedValue::Int(_)
        | OwnedValue::Float(_)
        | OwnedValue::String(_)
        | OwnedValue::Array(_) => Ok(reflect::value::ReflectValue::Host(owned_to_host(
            value, operation,
        )?)),
    }
}

pub(crate) fn value_from_reflect(value: reflect::value::ReflectValue) -> VmResult<OwnedValue> {
    match value {
        reflect::value::ReflectValue::Host(value) => Ok(host_to_owned(value)),
        reflect::value::ReflectValue::HostRef(host_ref) => Ok(OwnedValue::HostRef(host_ref)),
        reflect::value::ReflectValue::Closure => Err(type_error("reflect closure conversion")),
        reflect::value::ReflectValue::Range => Err(type_error("reflect range conversion")),
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
        OwnedValue::Int(value) => Ok(HostValue::Int(*value)),
        OwnedValue::Float(value) => Ok(HostValue::Float(*value)),
        OwnedValue::String(value) => Ok(HostValue::String(value.clone())),
        OwnedValue::Array(values) => values
            .iter()
            .map(|value| owned_to_host(value, operation))
            .collect::<VmResult<Vec<_>>>()
            .map(HostValue::Array),
        OwnedValue::Map(values) => values
            .iter()
            .map(|(key, value)| Ok((key.clone(), owned_to_host(value, operation)?)))
            .collect::<VmResult<BTreeMap<_, _>>>()
            .map(HostValue::Map),
        OwnedValue::Record { type_name, fields } if is_reflect_metadata_record(type_name) => {
            owned_record_to_host(type_name, fields, operation)
        }
        OwnedValue::Enum {
            enum_name,
            variant,
            fields,
        } => {
            let fields = fields
                .iter()
                .map(|(key, value)| Ok((key.to_owned(), owned_to_host(value, operation)?)))
                .collect::<VmResult<BTreeMap<_, _>>>()?;
            Ok(HostValue::Enum {
                enum_name: enum_name.clone(),
                variant: variant.clone(),
                fields,
            })
        }
        OwnedValue::HostRef(value) => Ok(HostValue::HostRef(*value)),
        OwnedValue::Missing
        | OwnedValue::Set(_)
        | OwnedValue::Record { .. }
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
        HostValue::Int(value) => OwnedValue::Int(value),
        HostValue::Float(value) => OwnedValue::Float(value),
        HostValue::String(value) => OwnedValue::String(value),
        HostValue::Array(values) => {
            OwnedValue::Array(values.into_iter().map(host_to_owned).collect())
        }
        HostValue::Map(values) => OwnedValue::Map(
            values
                .into_iter()
                .map(|(key, value)| (key, host_to_owned(value)))
                .collect(),
        ),
        HostValue::Record { type_name, fields } => {
            let fields = fields
                .into_iter()
                .map(|(key, value)| (key, host_to_owned(value)))
                .collect::<BTreeMap<_, _>>();
            OwnedValue::Record {
                fields: ScriptFields::from_pairs(&type_name, fields),
                type_name,
            }
        }
        HostValue::Enum {
            enum_name,
            variant,
            fields,
        } => {
            let owner = format!("{enum_name}::{variant}");
            let fields = fields
                .into_iter()
                .map(|(key, value)| (key, host_to_owned(value)))
                .collect::<BTreeMap<_, _>>();
            OwnedValue::Enum {
                fields: ScriptFields::from_pairs(&owner, fields),
                enum_name,
                variant,
            }
        }
        HostValue::HostRef(value) => OwnedValue::HostRef(value),
    }
}

fn owned_record_to_host(
    type_name: &str,
    fields: &ScriptFields<OwnedValue>,
    operation: &'static str,
) -> VmResult<HostValue> {
    let fields = fields
        .iter()
        .map(|(key, value)| Ok((key.to_owned(), owned_to_host(value, operation)?)))
        .collect::<VmResult<BTreeMap<_, _>>>()?;
    Ok(HostValue::Record {
        type_name: type_name.to_owned(),
        fields,
    })
}

fn is_reflect_metadata_record(type_name: &str) -> bool {
    type_name.starts_with("Reflect")
}

fn type_error(operation: &'static str) -> VmError {
    VmError::new(VmErrorKind::TypeMismatch { operation })
}
