use crate::collection_mutation;
use crate::heap::HeapValue;
use crate::value_key::ValueKey;
use crate::{
    ExecutionBudget, HeapExecution, StandardMethodInlineCacheTarget, Value, VmError, VmErrorKind,
    VmResult, store_runtime_value,
};

pub(in crate::standard_method_cache) fn call_cached_set_mutation(
    receiver: &Value,
    target: StandardMethodInlineCacheTarget,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> Option<VmResult<Value>> {
    match target {
        StandardMethodInlineCacheTarget::Add => Some(call_cached_set_insert(
            receiver,
            args,
            heap,
            budget,
            "add",
            "method add",
        )),
        StandardMethodInlineCacheTarget::Insert => Some(call_cached_set_insert(
            receiver,
            args,
            heap,
            budget,
            "insert",
            "method insert",
        )),
        StandardMethodInlineCacheTarget::Remove => {
            Some(call_cached_set_remove(receiver, args, heap))
        }
        StandardMethodInlineCacheTarget::Clear => Some(call_cached_set_clear(receiver, args, heap)),
        StandardMethodInlineCacheTarget::Extend => {
            Some(call_cached_set_extend(receiver, args, heap, budget))
        }
        _ => None,
    }
}

fn call_cached_set_insert(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    method: &'static str,
    operation: &'static str,
) -> VmResult<Value> {
    crate::runtime_checks::expect_arity(method, args, 1)?;
    let reference = set_reference(receiver, operation)?;
    let Some(heap) = heap.as_deref_mut() else {
        return type_error(operation);
    };
    let key = ValueKey::from_value(&args[0], Some(&*heap), operation)?;
    if set_slots(heap, reference, operation)?.contains_key(&key) {
        return Ok(Value::Bool(false));
    }
    let slot = store_runtime_value(&args[0], heap, budget.as_deref_mut())?;
    collection_mutation::push_set_slot(heap, reference, slot, budget.as_deref_mut(), operation)?;
    Ok(Value::Bool(true))
}

fn call_cached_set_remove(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
) -> VmResult<Value> {
    crate::runtime_checks::expect_arity("remove", args, 1)?;
    let reference = set_reference(receiver, "method remove")?;
    let Some(heap) = heap.as_deref_mut() else {
        return type_error("method remove");
    };
    let key = ValueKey::from_value(&args[0], Some(&*heap), "method remove")?;
    let changed =
        collection_mutation::remove_set_slot(heap, reference, &key, None, "method remove")?;
    Ok(Value::Bool(changed))
}

fn call_cached_set_clear(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
) -> VmResult<Value> {
    crate::runtime_checks::expect_arity("clear", args, 0)?;
    let reference = set_reference(receiver, "method clear")?;
    let Some(heap) = heap.as_deref_mut() else {
        return type_error("method clear");
    };
    collection_mutation::clear_set(heap, reference, None, "method clear")?;
    Ok(Value::Unit)
}

fn call_cached_set_extend(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    crate::runtime_checks::expect_arity("extend", args, 1)?;
    let reference = set_reference(receiver, "method extend")?;
    let Some(heap) = heap.as_deref_mut() else {
        return type_error("method extend");
    };
    let extension_reference = set_reference(&args[0], "method extend")?;
    if reference == extension_reference {
        set_slots(heap, reference, "method extend")?;
        return Ok(Value::Unit);
    }
    match set_slot_entry(heap, extension_reference, "method extend")? {
        SetSlotEntry::Empty => {
            set_slots(heap, reference, "method extend")?;
            return Ok(Value::Unit);
        }
        SetSlotEntry::Single(slot) => {
            extend_set_slots(
                heap,
                reference,
                &[slot],
                budget.as_deref_mut(),
                "method extend",
            )?;
            return Ok(Value::Unit);
        }
        SetSlotEntry::Pair(first, second) => {
            extend_set_slots(
                heap,
                reference,
                &[first, second],
                budget.as_deref_mut(),
                "method extend",
            )?;
            return Ok(Value::Unit);
        }
        SetSlotEntry::Many => {}
    }
    let extension = set_slot_values(heap, extension_reference, "method extend")?;
    extend_set_slots(
        heap,
        reference,
        &extension,
        budget.as_deref_mut(),
        "method extend",
    )?;
    Ok(Value::Unit)
}

enum SetSlotEntry {
    Empty,
    Single(Value),
    Pair(Value, Value),
    Many,
}

fn set_slot_entry(
    heap: &HeapExecution<'_>,
    reference: crate::heap::GcRef,
    operation: &'static str,
) -> VmResult<SetSlotEntry> {
    let values = set_slots(heap, reference, operation)?.values_vec();
    match values.as_slice() {
        [] => Ok(SetSlotEntry::Empty),
        [Value::Missing] => type_error("missing value"),
        [value] => Ok(SetSlotEntry::Single(*value)),
        [Value::Missing, _] | [_, Value::Missing] => type_error("missing value"),
        [first, second] => Ok(SetSlotEntry::Pair(*first, *second)),
        _ => Ok(SetSlotEntry::Many),
    }
}

fn extend_set_slots(
    heap: &mut HeapExecution<'_>,
    reference: crate::heap::GcRef,
    extension: &[Value],
    budget: Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<()> {
    collection_mutation::extend_set_slots(
        heap,
        reference,
        extension.iter().copied(),
        budget,
        operation,
    )
}

fn set_slot_values(
    heap: &HeapExecution<'_>,
    reference: crate::heap::GcRef,
    operation: &'static str,
) -> VmResult<Vec<Value>> {
    let Some(HeapValue::Set(values)) = heap.heap.get(reference) else {
        return type_error(operation);
    };
    if values.values().any(|value| matches!(value, Value::Missing)) {
        return type_error("missing value");
    }
    Ok(values.values_vec())
}

fn set_slots<'a>(
    heap: &'a HeapExecution<'_>,
    reference: crate::heap::GcRef,
    operation: &'static str,
) -> VmResult<&'a crate::script_set::ScriptSet> {
    let Some(HeapValue::Set(values)) = heap.heap.get(reference) else {
        return type_error(operation);
    };
    Ok(values)
}

fn set_reference(receiver: &Value, operation: &'static str) -> VmResult<crate::heap::GcRef> {
    match receiver {
        Value::HeapRef(reference) => Ok(*reference),
        _ => type_error(operation),
    }
}

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}
