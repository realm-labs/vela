use crate::heap::HeapValue;
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
            Value::Scalar(vela_common::ScalarValue::I64(value)) => Ok(Self::Int(*value)),
            Value::Scalar(vela_common::ScalarValue::F64(value)) if value.is_finite() => {
                Ok(Self::Float(value.to_bits()))
            }
            Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
                Some(HeapValue::String(value)) => Ok(Self::String(value.clone())),
                _ => type_error(operation),
            },
            _ => type_error(operation),
        }
    }

    #[allow(dead_code)]
    pub(super) fn matches_value(
        &self,
        value: &Value,
        heap: Option<&HeapExecution<'_>>,
        operation: &'static str,
    ) -> VmResult<bool> {
        match (self, value) {
            (Self::Null, Value::Null) => Ok(true),
            (Self::Bool(lhs), Value::Bool(rhs)) => Ok(*lhs == *rhs),
            (Self::Int(lhs), Value::Scalar(vela_common::ScalarValue::I64(rhs))) => Ok(*lhs == *rhs),
            (Self::Float(lhs), Value::Scalar(vela_common::ScalarValue::F64(rhs)))
                if rhs.is_finite() =>
            {
                Ok(*lhs == rhs.to_bits())
            }
            (Self::String(lhs), Value::HeapRef(reference)) => {
                match heap.and_then(|heap| heap.heap.get(*reference)) {
                    Some(HeapValue::String(rhs)) => Ok(lhs == rhs),
                    _ => type_error(operation),
                }
            }
            (_, Value::Null | Value::Bool(_) | Value::Scalar(vela_common::ScalarValue::I64(_))) => {
                Ok(false)
            }
            (_, Value::Scalar(vela_common::ScalarValue::F64(value))) if value.is_finite() => {
                Ok(false)
            }
            _ => type_error(operation),
        }
    }

    pub(super) fn matches_slot(
        &self,
        slot: &Value,
        heap: &HeapExecution<'_>,
        operation: &'static str,
    ) -> VmResult<bool> {
        match (self, slot) {
            (Self::Null, Value::Null) => Ok(true),
            (Self::Bool(lhs), Value::Bool(rhs)) => Ok(*lhs == *rhs),
            (Self::Int(lhs), Value::Scalar(vela_common::ScalarValue::I64(rhs))) => Ok(*lhs == *rhs),
            (Self::Float(lhs), Value::Scalar(vela_common::ScalarValue::F64(rhs)))
                if rhs.is_finite() =>
            {
                Ok(*lhs == rhs.to_bits())
            }
            (Self::String(lhs), Value::HeapRef(reference)) => match heap.heap.get(*reference) {
                Some(HeapValue::String(rhs)) => Ok(lhs == rhs),
                _ => type_error(operation),
            },
            (_, Value::Null | Value::Bool(_) | Value::Scalar(vela_common::ScalarValue::I64(_))) => {
                Ok(false)
            }
            (_, Value::Scalar(vela_common::ScalarValue::F64(value))) if value.is_finite() => {
                Ok(false)
            }
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

pub(super) fn slot_key(slot: &Value, heap: &HeapExecution<'_>) -> VmResult<SetKey> {
    match slot {
        Value::Null => Ok(SetKey::Null),
        Value::Bool(value) => Ok(SetKey::Bool(*value)),
        Value::Scalar(vela_common::ScalarValue::I64(value)) => Ok(SetKey::Int(*value)),
        Value::Scalar(vela_common::ScalarValue::F64(value)) if value.is_finite() => {
            Ok(SetKey::Float(value.to_bits()))
        }
        Value::HeapRef(reference) => match heap.heap.get(*reference) {
            Some(HeapValue::String(value)) => Ok(SetKey::String(value.clone())),
            _ => type_error("method set"),
        },
        Value::Missing | Value::Scalar(_) | Value::Range(_) | Value::HostRef(_) => {
            type_error("method set")
        }
    }
}
