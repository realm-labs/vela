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
            (Self::Null, HeapSlot::Null) => Ok(true),
            (Self::Bool(lhs), HeapSlot::Bool(rhs)) => Ok(*lhs == *rhs),
            (Self::Int(lhs), HeapSlot::Int(rhs)) => Ok(*lhs == *rhs),
            (Self::Float(lhs), HeapSlot::Float(rhs)) if rhs.is_finite() => {
                Ok(*lhs == rhs.to_bits())
            }
            (Self::String(lhs), HeapSlot::Ref(reference)) => match heap.heap.get(*reference) {
                Some(HeapValue::String(rhs)) => Ok(lhs == rhs),
                _ => type_error(operation),
            },
            (_, HeapSlot::Null | HeapSlot::Bool(_) | HeapSlot::Int(_)) => Ok(false),
            (_, HeapSlot::Float(value)) if value.is_finite() => Ok(false),
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
        HeapSlot::Null => Ok(SetKey::Null),
        HeapSlot::Bool(value) => Ok(SetKey::Bool(*value)),
        HeapSlot::Int(value) => Ok(SetKey::Int(*value)),
        HeapSlot::Float(value) if value.is_finite() => Ok(SetKey::Float(value.to_bits())),
        HeapSlot::Ref(reference) => match heap.heap.get(*reference) {
            Some(HeapValue::String(value)) => Ok(SetKey::String(value.clone())),
            _ => type_error("method set"),
        },
        HeapSlot::HostRef(_) | HeapSlot::PathProxy(_) => type_error("method set"),
        HeapSlot::Float(_) => type_error("method set"),
    }
}
