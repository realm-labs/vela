use crate::heap::{HeapSlot, HeapValue};
use crate::{HeapExecution, Value, VmResult};

use super::type_error;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum SetKey {
    Null,
    Bool(bool),
    Int(i64),
    Float(u64),
    String(String),
}

impl SetKey {
    pub(super) fn from_value(
        value: &Value,
        heap: Option<&HeapExecution<'_>>,
        operation: &'static str,
    ) -> VmResult<Self> {
        match value {
            Value::Null => Ok(Self::Null),
            Value::Bool(value) => Ok(Self::Bool(*value)),
            Value::Int(value) => Ok(Self::Int(*value)),
            Value::Float(value) if value.is_finite() => Ok(Self::Float(value.to_bits())),
            Value::String(value) => Ok(Self::String(value.clone())),
            Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
                Some(HeapValue::String(value)) => Ok(Self::String(value.clone())),
                _ => type_error(operation),
            },
            _ => type_error(operation),
        }
    }

    pub(super) fn matches_value(
        &self,
        value: &Value,
        heap: Option<&HeapExecution<'_>>,
        operation: &'static str,
    ) -> VmResult<bool> {
        match (self, value) {
            (Self::Null, Value::Null) => Ok(true),
            (Self::Bool(lhs), Value::Bool(rhs)) => Ok(*lhs == *rhs),
            (Self::Int(lhs), Value::Int(rhs)) => Ok(*lhs == *rhs),
            (Self::Float(lhs), Value::Float(rhs)) if rhs.is_finite() => Ok(*lhs == rhs.to_bits()),
            (Self::String(lhs), Value::String(rhs)) => Ok(lhs == rhs),
            (Self::String(lhs), Value::HeapRef(reference)) => {
                match heap.and_then(|heap| heap.heap.get(*reference)) {
                    Some(HeapValue::String(rhs)) => Ok(lhs == rhs),
                    _ => type_error(operation),
                }
            }
            (_, Value::Null | Value::Bool(_) | Value::Int(_) | Value::String(_)) => Ok(false),
            (_, Value::Float(value)) if value.is_finite() => Ok(false),
            _ => type_error(operation),
        }
    }

    pub(super) fn matches_slot(
        &self,
        slot: &HeapSlot,
        heap: &HeapExecution<'_>,
        operation: &'static str,
    ) -> VmResult<bool> {
        match (self, slot) {
            (Self::Null, Value::Null) => Ok(true),
            (Self::Bool(lhs), Value::Bool(rhs)) => Ok(*lhs == *rhs),
            (Self::Int(lhs), Value::Int(rhs)) => Ok(*lhs == *rhs),
            (Self::Float(lhs), Value::Float(rhs)) if rhs.is_finite() => Ok(*lhs == rhs.to_bits()),
            (Self::String(lhs), Value::HeapRef(reference)) => match heap.heap.get(*reference) {
                Some(HeapValue::String(rhs)) => Ok(lhs == rhs),
                _ => type_error(operation),
            },
            (_, Value::Null | Value::Bool(_) | Value::Int(_)) => Ok(false),
            (_, Value::Float(value)) if value.is_finite() => Ok(false),
            _ => type_error(operation),
        }
    }
}

pub(super) fn set_keys(
    values: &[Value],
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<Vec<SetKey>> {
    values
        .iter()
        .map(|value| SetKey::from_value(value, heap, operation))
        .collect()
}

pub(super) fn slot_key(slot: &HeapSlot, heap: &HeapExecution<'_>) -> VmResult<SetKey> {
    match slot {
        Value::Null => Ok(SetKey::Null),
        Value::Bool(value) => Ok(SetKey::Bool(*value)),
        Value::Int(value) => Ok(SetKey::Int(*value)),
        Value::Float(value) if value.is_finite() => Ok(SetKey::Float(value.to_bits())),
        Value::HeapRef(reference) => match heap.heap.get(*reference) {
            Some(HeapValue::String(value)) => Ok(SetKey::String(value.clone())),
            _ => type_error("method set"),
        },
        Value::Missing
        | Value::Float(_)
        | Value::String(_)
        | Value::Array(_)
        | Value::Map(_)
        | Value::Set(_)
        | Value::Record { .. }
        | Value::Enum { .. }
        | Value::Closure(_)
        | Value::Range(_)
        | Value::Iterator(_)
        | Value::HostRef(_)
        | Value::PathProxy(_) => type_error("method set"),
    }
}
