//! Conversions between VM-owned values and detached Host boundary values.

use vela_host::call_value::{HostCallField, HostCallMapEntry, HostCallValue};

use crate::error::{VmError, VmErrorKind, VmResult};
use crate::owned_value::{OwnedMapEntry, OwnedValue};

pub fn owned_to_host_call_value(value: &OwnedValue) -> VmResult<HostCallValue> {
    match value {
        OwnedValue::Unit => Ok(HostCallValue::Unit),
        OwnedValue::Bool(value) => Ok(HostCallValue::Bool(*value)),
        OwnedValue::Char(value) => Ok(HostCallValue::Char(*value)),
        OwnedValue::Scalar(value) => Ok(HostCallValue::Scalar(*value)),
        OwnedValue::String(value) => Ok(HostCallValue::String(value.clone())),
        OwnedValue::Bytes(value) => Ok(HostCallValue::Bytes(value.clone())),
        OwnedValue::Tuple(values) => values
            .iter()
            .map(owned_to_host_call_value)
            .collect::<VmResult<Vec<_>>>()
            .map(HostCallValue::Tuple),
        OwnedValue::Array(values) => values
            .iter()
            .map(owned_to_host_call_value)
            .collect::<VmResult<Vec<_>>>()
            .map(HostCallValue::Array),
        OwnedValue::Map(entries) => entries
            .iter()
            .map(|entry| {
                Ok(HostCallMapEntry::new(
                    owned_to_host_call_value(&entry.key)?,
                    owned_to_host_call_value(&entry.value)?,
                ))
            })
            .collect::<VmResult<Vec<_>>>()
            .map(HostCallValue::Map),
        OwnedValue::Set(values) => values
            .iter()
            .map(owned_to_host_call_value)
            .collect::<VmResult<Vec<_>>>()
            .map(HostCallValue::Set),
        OwnedValue::Record { type_name, fields } => Ok(HostCallValue::Record {
            type_name: type_name.clone(),
            fields: fields
                .iter()
                .map(|(name, value)| Ok(HostCallField::new(name, owned_to_host_call_value(value)?)))
                .collect::<VmResult<Vec<_>>>()?,
        }),
        OwnedValue::Enum {
            enum_name,
            variant,
            fields,
        } => Ok(HostCallValue::Enum {
            enum_name: enum_name.clone(),
            variant: variant.clone(),
            fields: fields
                .iter()
                .map(|(name, value)| Ok(HostCallField::new(name, owned_to_host_call_value(value)?)))
                .collect::<VmResult<Vec<_>>>()?,
        }),
        OwnedValue::HostRef(value) => Ok(HostCallValue::HostRef(*value)),
        OwnedValue::Closure(_)
        | OwnedValue::Range(_)
        | OwnedValue::PathProxy(_)
        | OwnedValue::Iterator(_) => Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "non-detached Host boundary value",
        })),
    }
}

#[must_use]
pub fn host_call_to_owned_value(value: HostCallValue) -> OwnedValue {
    match value {
        HostCallValue::Unit => OwnedValue::Unit,
        HostCallValue::Bool(value) => OwnedValue::Bool(value),
        HostCallValue::Char(value) => OwnedValue::Char(value),
        HostCallValue::Scalar(value) => OwnedValue::Scalar(value),
        HostCallValue::String(value) => OwnedValue::String(value),
        HostCallValue::Bytes(value) => OwnedValue::Bytes(value),
        HostCallValue::Tuple(values) => {
            OwnedValue::Tuple(values.into_iter().map(host_call_to_owned_value).collect())
        }
        HostCallValue::Array(values) => {
            OwnedValue::Array(values.into_iter().map(host_call_to_owned_value).collect())
        }
        HostCallValue::Map(entries) => OwnedValue::Map(
            entries
                .into_iter()
                .map(|entry| {
                    OwnedMapEntry::new(
                        host_call_to_owned_value(entry.key),
                        host_call_to_owned_value(entry.value),
                    )
                })
                .collect(),
        ),
        HostCallValue::Set(values) => {
            OwnedValue::Set(values.into_iter().map(host_call_to_owned_value).collect())
        }
        HostCallValue::Record { type_name, fields } => OwnedValue::record(
            type_name,
            fields
                .into_iter()
                .map(|field| (field.name, host_call_to_owned_value(field.value))),
        ),
        HostCallValue::Enum {
            enum_name,
            variant,
            fields,
        } => OwnedValue::enum_variant(
            enum_name,
            variant,
            fields
                .into_iter()
                .map(|field| (field.name, host_call_to_owned_value(field.value))),
        ),
        HostCallValue::HostRef(value) => OwnedValue::HostRef(value),
    }
}
