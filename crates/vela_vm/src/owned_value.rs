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

impl OwnedValue {
    #[must_use]
    pub fn array<T>(values: impl IntoIterator<Item = T>) -> Self
    where
        T: Into<Self>,
    {
        Self::Array(values.into_iter().map(Into::into).collect())
    }

    #[must_use]
    pub fn map<K, V>(entries: impl IntoIterator<Item = (K, V)>) -> Self
    where
        K: Into<String>,
        V: Into<Self>,
    {
        Self::Map(
            entries
                .into_iter()
                .map(|(key, value)| (key.into(), value.into()))
                .collect(),
        )
    }

    #[must_use]
    pub fn set<T>(values: impl IntoIterator<Item = T>) -> Self
    where
        T: Into<Self>,
    {
        Self::Set(values.into_iter().map(Into::into).collect())
    }

    #[must_use]
    pub fn record<K, V>(
        type_name: impl Into<String>,
        fields: impl IntoIterator<Item = (K, V)>,
    ) -> Self
    where
        K: Into<String>,
        V: Into<Self>,
    {
        let type_name = type_name.into();
        let fields = ScriptFields::from_pairs(
            &type_name,
            fields
                .into_iter()
                .map(|(field, value)| (field.into(), value.into())),
        );
        Self::Record { type_name, fields }
    }

    #[must_use]
    pub fn enum_variant<K, V>(
        enum_name: impl Into<String>,
        variant: impl Into<String>,
        fields: impl IntoIterator<Item = (K, V)>,
    ) -> Self
    where
        K: Into<String>,
        V: Into<Self>,
    {
        let enum_name = enum_name.into();
        let variant = variant.into();
        let owner = format!("{enum_name}::{variant}");
        let fields = ScriptFields::from_pairs(
            &owner,
            fields
                .into_iter()
                .map(|(field, value)| (field.into(), value.into())),
        );
        Self::Enum {
            enum_name,
            variant,
            fields,
        }
    }

    #[must_use]
    pub fn field(&self, field: &str) -> Option<&Self> {
        match self {
            Self::Record { fields, .. } | Self::Enum { fields, .. } => fields.get(field),
            _ => None,
        }
    }

    pub fn set_existing_field(&mut self, field: &str, value: impl Into<Self>) -> Result<(), Self> {
        let value = value.into();
        match self {
            Self::Record { fields, .. } | Self::Enum { fields, .. } => {
                fields.set_existing(field, value)
            }
            _ => Err(value),
        }
    }
}

#[macro_export]
macro_rules! owned_array {
    [$($value:expr),* $(,)?] => {
        $crate::owned_value::OwnedValue::Array(vec![
            $($crate::owned_value::OwnedValue::from($value)),*
        ])
    };
}

#[macro_export]
macro_rules! owned_map {
    {} => {
        $crate::owned_value::OwnedValue::map(
            Vec::<(String, $crate::owned_value::OwnedValue)>::new(),
        )
    };
    {$($key:expr => $value:expr),* $(,)?} => {
        $crate::owned_value::OwnedValue::map(vec![
            $(($key, $crate::owned_value::OwnedValue::from($value))),*
        ])
    };
}

#[macro_export]
macro_rules! owned_set {
    [$($value:expr),* $(,)?] => {
        $crate::owned_value::OwnedValue::Set(vec![
            $($crate::owned_value::OwnedValue::from($value)),*
        ])
    };
}

#[macro_export]
macro_rules! owned_record {
    ($type_name:expr, {}) => {
        $crate::owned_value::OwnedValue::record(
            $type_name,
            Vec::<(String, $crate::owned_value::OwnedValue)>::new(),
        )
    };
    ($type_name:expr, {$($field:expr => $value:expr),* $(,)?}) => {
        $crate::owned_value::OwnedValue::record(
            $type_name,
            vec![$(($field, $crate::owned_value::OwnedValue::from($value))),*],
        )
    };
}

#[macro_export]
macro_rules! owned_enum {
    ($enum_name:expr, $variant:expr, {}) => {
        $crate::owned_value::OwnedValue::enum_variant(
            $enum_name,
            $variant,
            Vec::<(String, $crate::owned_value::OwnedValue)>::new(),
        )
    };
    ($enum_name:expr, $variant:expr, {$($field:expr => $value:expr),* $(,)?}) => {
        $crate::owned_value::OwnedValue::enum_variant(
            $enum_name,
            $variant,
            vec![$(($field, $crate::owned_value::OwnedValue::from($value))),*],
        )
    };
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

impl From<i32> for OwnedValue {
    fn from(value: i32) -> Self {
        Self::Int(i64::from(value))
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

#[cfg(test)]
mod tests {
    use super::OwnedValue;

    #[test]
    fn owned_value_constructors_build_complex_values() {
        let state = OwnedValue::record(
            "ServerState",
            vec![
                ("level", 1.into()),
                ("name", "boot".into()),
                (
                    "stats",
                    OwnedValue::record("ServerStats", [("handled_ticks", 0)]),
                ),
            ],
        );

        assert_eq!(state.field("level"), Some(&OwnedValue::Int(1)));
        assert_eq!(
            state
                .field("stats")
                .and_then(|stats| stats.field("handled_ticks")),
            Some(&OwnedValue::Int(0))
        );
    }

    #[test]
    fn owned_value_macros_build_heterogeneous_values() {
        let state = crate::owned_record!("ServerState", {
            "level" => 10,
            "name" => "rust-updated",
            "stats" => crate::owned_record!("ServerStats", {
                "handled_ticks" => 7,
            }),
            "rewards" => crate::owned_array![
                crate::owned_map! {"kind" => "gold", "amount" => 5},
                crate::owned_map! {"kind" => "gem", "amount" => 1},
            ],
        });

        assert_eq!(state.field("level"), Some(&OwnedValue::Int(10)));
        assert_eq!(
            state
                .field("stats")
                .and_then(|stats| stats.field("handled_ticks")),
            Some(&OwnedValue::Int(7))
        );
    }

    #[test]
    fn owned_value_set_existing_field_updates_records_and_enums() {
        let mut state = crate::owned_record!("ServerState", {
            "level" => 1,
        });

        assert_eq!(state.set_existing_field("level", 2), Ok(()));
        assert_eq!(state.field("level"), Some(&OwnedValue::Int(2)));
        assert_eq!(
            state.set_existing_field("missing", 3),
            Err(OwnedValue::Int(3))
        );
        assert_eq!(
            OwnedValue::Int(1).set_existing_field("level", 2),
            Err(OwnedValue::Int(2))
        );
        assert_eq!(crate::owned_record!("Empty", {}).field("missing"), None);
    }
}
