use crate::heap::HeapValue;
use crate::heap_values::make_array_value;
use crate::owned_value::OwnedValue;
use crate::{ExecutionBudget, HeapExecution, Value, VmError, VmErrorKind, VmResult};

use super::{SetKey, expect_arity, set_values, type_error};

pub(crate) fn from_array(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    expect_arity("set::from_array", args, 1)?;
    let OwnedValue::Array(values) = &args[0] else {
        return owned_type_error("set::from_array");
    };
    let mut set = Vec::new();
    for value in values {
        let key = OwnedSetKey::from_value(value, "set::from_array")?;
        if set.iter().any(|existing| {
            OwnedSetKey::from_value(existing, "set::from_array").as_ref() == Ok(&key)
        }) {
            continue;
        }
        set.push(value.clone());
    }
    Ok(OwnedValue::Set(set))
}

pub(crate) fn has(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    expect_arity("has", args, 1)?;
    let key = SetKey::from_value(&args[0], heap, "method has")?;
    match receiver {
        Value::HeapRef(reference) => {
            let Some(heap) = heap else {
                return type_error("method has");
            };
            let Some(HeapValue::Set(values)) = heap.heap.get(*reference) else {
                return type_error("method has");
            };
            for value in values {
                if key.matches_slot(value, heap, "method has")? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        _ => type_error("method has"),
    }
}

pub(crate) fn values(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("values", args, 0)?;
    let values = set_values(receiver, heap.as_deref(), "method values")?;
    let Some(heap) = heap.as_deref_mut() else {
        return type_error("method values");
    };
    make_array_value(values, heap, budget.as_deref_mut())
}

#[derive(Clone, Debug, PartialEq)]
enum OwnedSetKey {
    Null,
    Bool(bool),
    Int(i64),
    Float(u64),
    String(String),
}

impl OwnedSetKey {
    fn from_value(value: &OwnedValue, operation: &'static str) -> VmResult<Self> {
        match value {
            OwnedValue::Null => Ok(Self::Null),
            OwnedValue::Bool(value) => Ok(Self::Bool(*value)),
            OwnedValue::Int(value) => Ok(Self::Int(*value)),
            OwnedValue::Float(value) if value.is_finite() => Ok(Self::Float(value.to_bits())),
            OwnedValue::String(value) => Ok(Self::String(value.clone())),
            _ => owned_type_error(operation),
        }
    }
}

fn owned_type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}
