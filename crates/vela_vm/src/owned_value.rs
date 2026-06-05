use std::collections::BTreeMap;
use std::sync::Arc;

use vela_bytecode::{CodeObject, Constant};
use vela_host::path::HostRef;
use vela_host::proxy::PathProxy;

use crate::iteration::IteratorState;
use crate::ranges::RangeValue;
use crate::script_object::ScriptFields;

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
