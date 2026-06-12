use crate::heap::HeapValue;
use crate::option_result::option_value;
use crate::{
    ExecutionBudget, HeapExecution, StandardMethodInlineCacheTarget, Value, VmError, VmErrorKind,
    VmResult, store_runtime_value,
};

pub(in crate::standard_method_cache) fn call_cached_array_mutation(
    receiver: &Value,
    target: StandardMethodInlineCacheTarget,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> Option<VmResult<Value>> {
    match target {
        StandardMethodInlineCacheTarget::Push => {
            Some(call_cached_array_push(receiver, args, heap, budget))
        }
        StandardMethodInlineCacheTarget::Pop => {
            Some(call_cached_array_pop(receiver, args, heap, budget))
        }
        StandardMethodInlineCacheTarget::Insert => {
            Some(call_cached_array_insert(receiver, args, heap, budget))
        }
        StandardMethodInlineCacheTarget::RemoveAt => {
            Some(call_cached_array_remove_at(receiver, args, heap, budget))
        }
        StandardMethodInlineCacheTarget::Clear => {
            Some(call_cached_array_clear(receiver, args, heap))
        }
        StandardMethodInlineCacheTarget::Extend => {
            Some(call_cached_array_extend(receiver, args, heap, budget))
        }
        _ => None,
    }
}

fn call_cached_array_push(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    crate::runtime_checks::expect_arity("push", args, 1)?;
    let reference = array_reference(receiver, "method push")?;
    let Some(heap) = heap.as_deref_mut() else {
        return type_error("method push");
    };
    let slot = store_runtime_value(&args[0], heap, budget.as_deref_mut())?;
    array_slots_mut(heap, reference, "method push")?.push(slot);
    Ok(Value::Null)
}

fn call_cached_array_pop(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    crate::runtime_checks::expect_arity("pop", args, 0)?;
    let reference = array_reference(receiver, "method pop")?;
    let Some(heap) = heap.as_deref_mut() else {
        return type_error("method pop");
    };
    let payload = array_slots_mut(heap, reference, "method pop")?.pop();
    option_value(payload, heap, budget.as_deref_mut())
}

fn call_cached_array_insert(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    crate::runtime_checks::expect_arity("insert", args, 2)?;
    let index = array_index_value(&args[0], "method insert")?;
    let reference = array_reference(receiver, "method insert")?;
    let Some(heap) = heap.as_deref_mut() else {
        return type_error("method insert");
    };
    let len = array_slots(heap, reference, "method insert")?.len();
    if index > len {
        return Err(index_out_of_bounds(index, len));
    }
    let slot = store_runtime_value(&args[1], heap, budget.as_deref_mut())?;
    array_slots_mut(heap, reference, "method insert")?.insert(index, slot);
    Ok(Value::Null)
}

fn call_cached_array_remove_at(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    crate::runtime_checks::expect_arity("remove_at", args, 1)?;
    let index = array_index_value(&args[0], "method remove_at")?;
    let reference = array_reference(receiver, "method remove_at")?;
    let Some(heap) = heap.as_deref_mut() else {
        return type_error("method remove_at");
    };
    if index >= array_slots(heap, reference, "method remove_at")?.len() {
        return option_value(None, heap, budget.as_deref_mut());
    }
    let payload = array_slots_mut(heap, reference, "method remove_at")?.remove(index);
    option_value(Some(payload), heap, budget.as_deref_mut())
}

fn call_cached_array_clear(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
) -> VmResult<Value> {
    crate::runtime_checks::expect_arity("clear", args, 0)?;
    let reference = array_reference(receiver, "method clear")?;
    let Some(heap) = heap.as_deref_mut() else {
        return type_error("method clear");
    };
    array_slots_mut(heap, reference, "method clear")?.clear();
    Ok(Value::Null)
}

fn call_cached_array_extend(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    _budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    crate::runtime_checks::expect_arity("extend", args, 1)?;
    let reference = array_reference(receiver, "method extend")?;
    let Some(heap) = heap.as_deref_mut() else {
        return type_error("method extend");
    };
    let extension_reference = array_reference(&args[0], "method extend")?;
    match array_slot_entry(heap, extension_reference, "method extend")? {
        ArraySlotEntry::Empty => {
            array_slots(heap, reference, "method extend")?;
            return Ok(Value::Null);
        }
        ArraySlotEntry::Single(slot) => {
            array_slots_mut(heap, reference, "method extend")?.push(slot);
            return Ok(Value::Null);
        }
        ArraySlotEntry::Pair(first, second) => {
            let values = array_slots_mut(heap, reference, "method extend")?;
            values.push(first);
            values.push(second);
            return Ok(Value::Null);
        }
        ArraySlotEntry::Many => {}
    }
    let slots = array_slot_values(heap, extension_reference, "method extend")?;
    array_slots_mut(heap, reference, "method extend")?.extend(slots);
    Ok(Value::Null)
}

enum ArraySlotEntry {
    Empty,
    Single(Value),
    Pair(Value, Value),
    Many,
}

fn array_slot_entry(
    heap: &HeapExecution<'_>,
    reference: crate::heap::GcRef,
    operation: &'static str,
) -> VmResult<ArraySlotEntry> {
    let values = array_slots(heap, reference, operation)?;
    match values {
        [] => Ok(ArraySlotEntry::Empty),
        [Value::Missing] => type_error("missing value"),
        [value] => Ok(ArraySlotEntry::Single(*value)),
        [Value::Missing, _] | [_, Value::Missing] => type_error("missing value"),
        [first, second] => Ok(ArraySlotEntry::Pair(*first, *second)),
        _ => Ok(ArraySlotEntry::Many),
    }
}

fn array_slot_values(
    heap: &HeapExecution<'_>,
    reference: crate::heap::GcRef,
    operation: &'static str,
) -> VmResult<Vec<Value>> {
    let Some(HeapValue::Array(values)) = heap.heap.get(reference) else {
        return type_error(operation);
    };
    if values.iter().any(|value| matches!(value, Value::Missing)) {
        return type_error("missing value");
    }
    Ok(values.clone())
}

fn array_slots<'a>(
    heap: &'a HeapExecution<'_>,
    reference: crate::heap::GcRef,
    operation: &'static str,
) -> VmResult<&'a [Value]> {
    let Some(HeapValue::Array(values)) = heap.heap.get(reference) else {
        return type_error(operation);
    };
    Ok(values)
}

fn array_slots_mut<'a>(
    heap: &'a mut HeapExecution<'_>,
    reference: crate::heap::GcRef,
    operation: &'static str,
) -> VmResult<&'a mut Vec<Value>> {
    let Some(HeapValue::Array(values)) = heap.heap.get_mut(reference).ok() else {
        return type_error(operation);
    };
    Ok(values)
}

fn array_reference(receiver: &Value, operation: &'static str) -> VmResult<crate::heap::GcRef> {
    match receiver {
        Value::HeapRef(reference) => Ok(*reference),
        _ => type_error(operation),
    }
}

fn array_index_value(value: &Value, operation: &'static str) -> VmResult<usize> {
    match value {
        Value::Scalar(vela_common::ScalarValue::I64(value)) if *value >= 0 => Ok(*value as usize),
        _ => type_error(operation),
    }
}

fn index_out_of_bounds(index: usize, len: usize) -> VmError {
    VmError::new(VmErrorKind::IndexOutOfBounds {
        index: i64::try_from(index).unwrap_or(i64::MAX),
        len,
    })
}

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}
