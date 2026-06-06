use std::collections::BTreeMap;
use std::sync::Arc;

use vela_bytecode::{CodeObject, Constant};
use vela_host::path::HostRef;
use vela_host::proxy::PathProxy;

use crate::error::VmResult;
use crate::iteration::IteratorState;
use crate::ranges::RangeValue;
use crate::script_object::ScriptFields;
use crate::value::Value;

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

impl From<bool> for OwnedValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<i64> for OwnedValue {
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}

impl From<f64> for OwnedValue {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl From<String> for OwnedValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&str> for OwnedValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

impl From<HostRef> for OwnedValue {
    fn from(value: HostRef) -> Self {
        Self::HostRef(value)
    }
}

pub fn owned_to_value_detached(value: OwnedValue) -> Value {
    match value {
        OwnedValue::Missing => Value::Missing,
        OwnedValue::Null => Value::Null,
        OwnedValue::Bool(value) => Value::Bool(value),
        OwnedValue::Int(value) => Value::Int(value),
        OwnedValue::Float(value) => Value::Float(value),
        OwnedValue::Range(value) => Value::Range(value),
        OwnedValue::HostRef(value) => Value::HostRef(value),
        OwnedValue::String(_)
        | OwnedValue::Array(_)
        | OwnedValue::Map(_)
        | OwnedValue::Set(_)
        | OwnedValue::Record { .. }
        | OwnedValue::Enum { .. }
        | OwnedValue::Closure(_)
        | OwnedValue::PathProxy(_)
        | OwnedValue::Iterator(_) => Value::Missing,
    }
}

pub fn value_to_owned_detached(value: &Value) -> VmResult<OwnedValue> {
    match value {
        Value::Missing => Ok(OwnedValue::Missing),
        Value::Null => Ok(OwnedValue::Null),
        Value::Bool(value) => Ok(OwnedValue::Bool(*value)),
        Value::Int(value) => Ok(OwnedValue::Int(*value)),
        Value::Float(value) => Ok(OwnedValue::Float(*value)),
        Value::Range(value) => Ok(OwnedValue::Range(*value)),
        Value::HostRef(value) => Ok(OwnedValue::HostRef(*value)),
        Value::HeapRef(_) => Ok(OwnedValue::Missing),
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
        (OwnedValue::Range(lhs), Value::Range(rhs)) => lhs == rhs,
        (OwnedValue::HostRef(lhs), Value::HostRef(rhs)) => lhs == rhs,
        _ => false,
    }
}
