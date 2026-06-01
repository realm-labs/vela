use std::collections::BTreeMap;
use std::sync::Arc;

use vela_bytecode::{CodeObject, Constant};
use vela_host::{HostRef, PathProxy};

use crate::heap::GcRef;
use crate::iteration::IteratorState;
use crate::ranges::RangeValue;
use crate::script_object::ScriptFields;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Missing,
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(Vec<Value>),
    Map(BTreeMap<String, Value>),
    Set(Vec<Value>),
    Record {
        type_name: String,
        fields: ScriptFields<Value>,
    },
    Enum {
        enum_name: String,
        variant: String,
        fields: ScriptFields<Value>,
    },
    Closure(ClosureValue),
    Range(RangeValue),
    HeapRef(GcRef),
    HostRef(HostRef),
    PathProxy(PathProxy),
    Iterator(IteratorState),
}

impl Value {
    pub fn trace_heap_refs(&self, refs: &mut Vec<GcRef>) {
        match self {
            Self::HeapRef(reference) => refs.push(*reference),
            Self::Array(values) => values.iter().for_each(|value| value.trace_heap_refs(refs)),
            Self::Set(values) => values.iter().for_each(|value| value.trace_heap_refs(refs)),
            Self::Map(values) => values
                .values()
                .for_each(|value| value.trace_heap_refs(refs)),
            Self::Record { fields, .. } | Self::Enum { fields, .. } => {
                fields
                    .values()
                    .for_each(|value| value.trace_heap_refs(refs));
            }
            Self::Closure(closure) => closure
                .captures
                .iter()
                .for_each(|value| value.trace_heap_refs(refs)),
            Self::Iterator(iterator) => iterator.trace_heap_refs(refs),
            Self::Null
            | Self::Missing
            | Self::Bool(_)
            | Self::Int(_)
            | Self::Float(_)
            | Self::String(_)
            | Self::Range(_)
            | Self::HostRef(_)
            | Self::PathProxy(_) => {}
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClosureValue {
    pub(crate) code: Arc<CodeObject>,
    pub(crate) captures: Vec<Value>,
}

impl From<&Constant> for Value {
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
