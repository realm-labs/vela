use std::collections::BTreeMap;
use std::sync::Arc;

use vela_bytecode::{CodeObject, Constant};
use vela_host::path::HostRef;
use vela_host::proxy::PathProxy;

use crate::error::{VmError, VmErrorKind, VmResult};
use crate::iteration::IteratorState;
use crate::ranges::RangeValue;
use crate::script_object::ScriptFields;
use crate::value::{ClosureValue, Value};

#[derive(Clone, Debug, PartialEq)]
pub enum OwnedValue {
    Missing,
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(Vec<OwnedValue>),
    Map(BTreeMap<String, OwnedValue>),
    Set(Vec<OwnedValue>),
    Record {
        type_name: String,
        fields: ScriptFields<OwnedValue>,
    },
    Enum {
        enum_name: String,
        variant: String,
        fields: ScriptFields<OwnedValue>,
    },
    Closure(OwnedClosureValue),
    Range(RangeValue),
    HostRef(HostRef),
    PathProxy(PathProxy),
    Iterator(OwnedIteratorState),
}

#[derive(Clone, Debug, PartialEq)]
pub struct OwnedClosureValue {
    pub(crate) code: Arc<CodeObject>,
    pub(crate) captures: Vec<OwnedValue>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OwnedIteratorState {
    pub(crate) values: Vec<OwnedValue>,
    pub(crate) next: usize,
}

impl OwnedIteratorState {
    #[allow(dead_code)]
    pub(crate) fn from_runtime(iterator: &IteratorState, values: Vec<OwnedValue>) -> Self {
        Self {
            values,
            next: iterator.next_index(),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn values(&self) -> &[OwnedValue] {
        &self.values
    }

    #[allow(dead_code)]
    pub(crate) fn next_index(&self) -> usize {
        self.next
    }
}

impl From<&Constant> for OwnedValue {
    fn from(value: &Constant) -> Self {
        match value {
            Constant::Null => Self::Null,
            Constant::Bool(value) => Self::Bool(*value),
            Constant::Int(value) => Self::Int(*value),
            Constant::Float(value) => Self::Float(*value),
            Constant::String(value) => Self::String(value.clone()),
            Constant::Array(values) => Self::Array(values.iter().map(Self::from).collect()),
            Constant::Map(entries) => Self::Map(
                entries
                    .iter()
                    .map(|(key, value)| (key.clone(), Self::from(value)))
                    .collect::<BTreeMap<_, _>>(),
            ),
        }
    }
}

pub fn owned_to_value_detached(value: OwnedValue) -> Value {
    match value {
        OwnedValue::Missing => Value::Missing,
        OwnedValue::Null => Value::Null,
        OwnedValue::Bool(value) => Value::Bool(value),
        OwnedValue::Int(value) => Value::Int(value),
        OwnedValue::Float(value) => Value::Float(value),
        OwnedValue::String(value) => Value::String(value),
        OwnedValue::Array(values) => Value::Array(
            values
                .into_iter()
                .map(owned_to_value_detached)
                .collect::<Vec<_>>(),
        ),
        OwnedValue::Map(values) => Value::Map(
            values
                .into_iter()
                .map(|(key, value)| (key, owned_to_value_detached(value)))
                .collect::<BTreeMap<_, _>>(),
        ),
        OwnedValue::Set(values) => Value::Set(
            values
                .into_iter()
                .map(owned_to_value_detached)
                .collect::<Vec<_>>(),
        ),
        OwnedValue::Record { type_name, fields } => {
            let fields = fields
                .into_pairs()
                .map(|(key, value)| (key, owned_to_value_detached(value)))
                .collect::<Vec<_>>();
            Value::Record {
                fields: ScriptFields::from_pairs(&type_name, fields),
                type_name,
            }
        }
        OwnedValue::Enum {
            enum_name,
            variant,
            fields,
        } => {
            let owner = format!("{enum_name}::{variant}");
            let fields = fields
                .into_pairs()
                .map(|(key, value)| (key, owned_to_value_detached(value)))
                .collect::<Vec<_>>();
            Value::Enum {
                enum_name,
                variant,
                fields: ScriptFields::from_pairs(&owner, fields),
            }
        }
        OwnedValue::Closure(closure) => Value::Closure(ClosureValue {
            code: Arc::clone(&closure.code),
            captures: closure
                .captures
                .into_iter()
                .map(owned_to_value_detached)
                .collect(),
        }),
        OwnedValue::Range(value) => Value::Range(value),
        OwnedValue::HostRef(value) => Value::HostRef(value),
        OwnedValue::PathProxy(value) => Value::PathProxy(value),
        OwnedValue::Iterator(iterator) => Value::Iterator(IteratorState::from_values_at(
            iterator
                .values
                .into_iter()
                .map(owned_to_value_detached)
                .collect(),
            iterator.next,
        )),
    }
}

pub fn value_to_owned_detached(value: &Value) -> VmResult<OwnedValue> {
    match value {
        Value::Missing => Ok(OwnedValue::Missing),
        Value::Null => Ok(OwnedValue::Null),
        Value::Bool(value) => Ok(OwnedValue::Bool(*value)),
        Value::Int(value) => Ok(OwnedValue::Int(*value)),
        Value::Float(value) => Ok(OwnedValue::Float(*value)),
        Value::String(value) => Ok(OwnedValue::String(value.clone())),
        Value::Array(values) => values
            .iter()
            .map(value_to_owned_detached)
            .collect::<VmResult<Vec<_>>>()
            .map(OwnedValue::Array),
        Value::Map(values) => values
            .iter()
            .map(|(key, value)| Ok((key.clone(), value_to_owned_detached(value)?)))
            .collect::<VmResult<BTreeMap<_, _>>>()
            .map(OwnedValue::Map),
        Value::Set(values) => values
            .iter()
            .map(value_to_owned_detached)
            .collect::<VmResult<Vec<_>>>()
            .map(OwnedValue::Set),
        Value::Record { type_name, fields } => fields
            .iter()
            .map(|(key, value)| Ok((key.to_owned(), value_to_owned_detached(value)?)))
            .collect::<VmResult<Vec<_>>>()
            .map(|fields| OwnedValue::Record {
                type_name: type_name.clone(),
                fields: ScriptFields::from_pairs(type_name, fields),
            }),
        Value::Enum {
            enum_name,
            variant,
            fields,
        } => fields
            .iter()
            .map(|(key, value)| Ok((key.to_owned(), value_to_owned_detached(value)?)))
            .collect::<VmResult<Vec<_>>>()
            .map(|fields| OwnedValue::Enum {
                enum_name: enum_name.clone(),
                variant: variant.clone(),
                fields: ScriptFields::from_pairs(&format!("{enum_name}::{variant}"), fields),
            }),
        Value::Closure(closure) => closure
            .captures
            .iter()
            .map(value_to_owned_detached)
            .collect::<VmResult<Vec<_>>>()
            .map(|captures| {
                OwnedValue::Closure(OwnedClosureValue {
                    code: Arc::clone(&closure.code),
                    captures,
                })
            }),
        Value::Range(value) => Ok(OwnedValue::Range(*value)),
        Value::HostRef(value) => Ok(OwnedValue::HostRef(*value)),
        Value::PathProxy(value) => Ok(OwnedValue::PathProxy(value.clone())),
        Value::Iterator(iterator) => iterator
            .values()
            .iter()
            .map(value_to_owned_detached)
            .collect::<VmResult<Vec<_>>>()
            .map(|values| OwnedValue::Iterator(OwnedIteratorState::from_runtime(iterator, values))),
        Value::HeapRef(_) => Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "detached owned value conversion",
        })),
    }
}

impl PartialEq<Value> for OwnedValue {
    fn eq(&self, other: &Value) -> bool {
        owned_value_eq_runtime(self, other)
    }
}

impl PartialEq<OwnedValue> for Value {
    fn eq(&self, other: &OwnedValue) -> bool {
        owned_value_eq_runtime(other, self)
    }
}

fn owned_value_eq_runtime(lhs: &OwnedValue, rhs: &Value) -> bool {
    match (lhs, rhs) {
        (OwnedValue::Missing, Value::Missing) | (OwnedValue::Null, Value::Null) => true,
        (OwnedValue::Bool(lhs), Value::Bool(rhs)) => lhs == rhs,
        (OwnedValue::Int(lhs), Value::Int(rhs)) => lhs == rhs,
        (OwnedValue::Float(lhs), Value::Float(rhs)) => lhs == rhs,
        (OwnedValue::String(lhs), Value::String(rhs)) => lhs == rhs,
        (OwnedValue::Range(lhs), Value::Range(rhs)) => lhs == rhs,
        (OwnedValue::HostRef(lhs), Value::HostRef(rhs)) => lhs == rhs,
        (OwnedValue::PathProxy(lhs), Value::PathProxy(rhs)) => lhs == rhs,
        (OwnedValue::Array(lhs), Value::Array(rhs)) | (OwnedValue::Set(lhs), Value::Set(rhs)) => {
            lhs.len() == rhs.len()
                && lhs
                    .iter()
                    .zip(rhs.iter())
                    .all(|(lhs, rhs)| owned_value_eq_runtime(lhs, rhs))
        }
        (OwnedValue::Map(lhs), Value::Map(rhs)) => {
            lhs.len() == rhs.len()
                && lhs.iter().all(|(key, lhs)| {
                    rhs.get(key)
                        .is_some_and(|rhs| owned_value_eq_runtime(lhs, rhs))
                })
        }
        (
            OwnedValue::Record {
                type_name: lhs_type,
                fields: lhs_fields,
            },
            Value::Record {
                type_name: rhs_type,
                fields: rhs_fields,
            },
        ) => {
            lhs_type == rhs_type
                && lhs_fields.len() == rhs_fields.len()
                && lhs_fields.iter().all(|(key, lhs)| {
                    rhs_fields
                        .get(key)
                        .is_some_and(|rhs| owned_value_eq_runtime(lhs, rhs))
                })
        }
        (
            OwnedValue::Enum {
                enum_name: lhs_enum,
                variant: lhs_variant,
                fields: lhs_fields,
            },
            Value::Enum {
                enum_name: rhs_enum,
                variant: rhs_variant,
                fields: rhs_fields,
            },
        ) => {
            lhs_enum == rhs_enum
                && lhs_variant == rhs_variant
                && lhs_fields.len() == rhs_fields.len()
                && lhs_fields.iter().all(|(key, lhs)| {
                    rhs_fields
                        .get(key)
                        .is_some_and(|rhs| owned_value_eq_runtime(lhs, rhs))
                })
        }
        (OwnedValue::Closure(lhs), Value::Closure(rhs)) => {
            Arc::ptr_eq(&lhs.code, &rhs.code)
                && lhs.captures.len() == rhs.captures.len()
                && lhs
                    .captures
                    .iter()
                    .zip(rhs.captures.iter())
                    .all(|(lhs, rhs)| owned_value_eq_runtime(lhs, rhs))
        }
        (OwnedValue::Iterator(lhs), Value::Iterator(rhs)) => {
            lhs.next == rhs.next_index()
                && lhs.values.len() == rhs.values().len()
                && lhs
                    .values
                    .iter()
                    .zip(rhs.values().iter())
                    .all(|(lhs, rhs)| owned_value_eq_runtime(lhs, rhs))
        }
        _ => false,
    }
}
