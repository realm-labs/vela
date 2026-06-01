use std::collections::BTreeMap;

use vela_reflect as reflect;

use crate::host_values::{value_from_host, value_to_host};
use crate::script_object::ScriptFields;
use crate::{Value, VmError, VmErrorKind, VmResult};

pub(crate) fn value_to_reflect(
    value: &Value,
    operation: &'static str,
) -> VmResult<reflect::ReflectValue> {
    match value {
        Value::HostRef(host_ref) => Ok(reflect::ReflectValue::HostRef(*host_ref)),
        Value::Map(values) => {
            let values = values
                .iter()
                .map(|(key, value)| Ok((key.clone(), value_to_reflect(value, operation)?)))
                .collect::<VmResult<BTreeMap<_, _>>>()?;
            Ok(reflect::ReflectValue::Record(values))
        }
        Value::Set(values) => values
            .iter()
            .map(|value| value_to_reflect(value, operation))
            .collect::<VmResult<Vec<_>>>()
            .map(reflect::ReflectValue::Set),
        Value::Record {
            type_name,
            fields: values,
        } => {
            let values = values
                .iter()
                .map(|(key, value)| Ok((key.to_owned(), value_to_reflect(value, operation)?)))
                .collect::<VmResult<BTreeMap<_, _>>>()?;
            Ok(reflect::ReflectValue::ScriptRecord {
                type_name: type_name.clone(),
                fields: values,
            })
        }
        Value::Enum {
            enum_name,
            variant,
            fields: values,
        } => {
            let values = values
                .iter()
                .map(|(key, value)| Ok((key.to_owned(), value_to_reflect(value, operation)?)))
                .collect::<VmResult<BTreeMap<_, _>>>()?;
            Ok(reflect::ReflectValue::ScriptEnum {
                enum_name: enum_name.clone(),
                variant: variant.clone(),
                fields: values,
            })
        }
        Value::Array(_) => Ok(reflect::ReflectValue::Host(value_to_host(
            value, operation, None,
        )?)),
        Value::Closure(_) => Ok(reflect::ReflectValue::Closure),
        Value::Range(_) => Ok(reflect::ReflectValue::Range),
        Value::PathProxy(_) | Value::Missing | Value::HeapRef(_) | Value::Iterator(_) => {
            Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
        }
        Value::Null | Value::Bool(_) | Value::Int(_) | Value::Float(_) | Value::String(_) => Ok(
            reflect::ReflectValue::Host(value_to_host(value, operation, None)?),
        ),
    }
}

pub(crate) fn value_from_reflect(value: reflect::ReflectValue) -> VmResult<Value> {
    match value {
        reflect::ReflectValue::Host(value) => Ok(value_from_host(value)),
        reflect::ReflectValue::HostRef(host_ref) => Ok(Value::HostRef(host_ref)),
        reflect::ReflectValue::Closure => Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "reflect closure conversion",
        })),
        reflect::ReflectValue::Range => Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "reflect range conversion",
        })),
        reflect::ReflectValue::Record(values) => {
            let values = values
                .into_iter()
                .map(|(key, value)| Ok((key, value_from_reflect(value)?)))
                .collect::<VmResult<BTreeMap<_, _>>>()?;
            Ok(Value::Map(values))
        }
        reflect::ReflectValue::Set(values) => values
            .into_iter()
            .map(value_from_reflect)
            .collect::<VmResult<Vec<_>>>()
            .map(Value::Set),
        reflect::ReflectValue::ScriptRecord { type_name, fields } => {
            let fields = fields
                .into_iter()
                .map(|(key, value)| Ok((key, value_from_reflect(value)?)))
                .collect::<VmResult<BTreeMap<_, _>>>()?;
            Ok(Value::Record {
                fields: ScriptFields::from_pairs(&type_name, fields),
                type_name,
            })
        }
        reflect::ReflectValue::ScriptEnum {
            enum_name,
            variant,
            fields,
        } => {
            let fields = fields
                .into_iter()
                .map(|(key, value)| Ok((key, value_from_reflect(value)?)))
                .collect::<VmResult<BTreeMap<_, _>>>()?;
            Ok(Value::Enum {
                fields: ScriptFields::from_pairs(&format!("{enum_name}.{variant}"), fields),
                enum_name,
                variant,
            })
        }
    }
}
