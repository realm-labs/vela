use crate::heap::HeapValue;
use crate::heap_values::allocate_heap_value;
use crate::iteration::IteratorState;
use crate::owned_value::OwnedValue;
use crate::{ExecutionBudget, HeapExecution, Value, VmError, VmErrorKind, VmResult};

use super::{contains_value, expect_arity, type_error};

pub(crate) fn from_array(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    expect_arity("set::from_array", args, 1)?;
    let OwnedValue::Array(values) = &args[0] else {
        return owned_type_error("set::from_array");
    };
    Ok(OwnedValue::Set(values.clone()))
}

pub(crate) fn has(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    expect_arity("has", args, 1)?;
    match receiver {
        Value::HeapRef(reference) => {
            let Some(heap) = heap else {
                return type_error("method has");
            };
            let Some(HeapValue::Set(values)) = heap.heap.get(*reference) else {
                return type_error("method has");
            };
            contains_value(values, &args[0], heap, "method has")
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
    let Value::HeapRef(reference) = receiver else {
        return type_error("method values");
    };
    let len = match heap.as_deref().and_then(|heap| heap.heap.get(*reference)) {
        Some(HeapValue::Set(values)) => values.len(),
        _ => return type_error("method values"),
    };
    let Some(heap) = heap.as_deref_mut() else {
        return type_error("method values");
    };
    allocate_heap_value(
        HeapValue::Iterator(IteratorState::from_set_source(*reference, len)),
        heap,
        budget.as_deref_mut(),
    )
}

fn owned_type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}
