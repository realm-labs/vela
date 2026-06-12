use std::collections::BTreeMap;

use crate::heap::HeapValue;
use crate::option_result::option_value;
use crate::{
    ExecutionBudget, HeapExecution, StandardMethodInlineCacheTarget, Value, VmError, VmErrorKind,
    VmResult, store_runtime_value, stored_runtime_value,
};

pub(in crate::standard_method_cache) fn call_cached_map_mutation(
    receiver: &Value,
    target: StandardMethodInlineCacheTarget,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> Option<VmResult<Value>> {
    match target {
        StandardMethodInlineCacheTarget::Set => {
            Some(call_cached_map_set(receiver, args, heap, budget))
        }
        StandardMethodInlineCacheTarget::Remove => {
            Some(call_cached_map_remove(receiver, args, heap, budget))
        }
        StandardMethodInlineCacheTarget::Clear => Some(call_cached_map_clear(receiver, args, heap)),
        StandardMethodInlineCacheTarget::Extend => {
            Some(call_cached_map_extend(receiver, args, heap, budget))
        }
        _ => None,
    }
}

fn call_cached_map_set(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    crate::runtime_checks::expect_arity("set", args, 2)?;
    let key = map_key(&args[0], heap.as_deref(), "map key")?;
    let reference = map_reference(receiver, "method set")?;
    let Some(heap) = heap.as_deref_mut() else {
        return type_error("method set");
    };
    let slot = store_runtime_value(&args[1], heap, budget.as_deref_mut())?;
    let values = map_slots_mut(heap, reference, "method set")?;
    values.insert(key, slot);
    Ok(args[1])
}

fn call_cached_map_remove(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    crate::runtime_checks::expect_arity("remove", args, 1)?;
    let key = map_key(&args[0], heap.as_deref(), "map key")?;
    let reference = map_reference(receiver, "method remove")?;
    let Some(heap) = heap.as_deref_mut() else {
        return type_error("method remove");
    };
    let payload = map_slots_mut(heap, reference, "method remove")?
        .remove(&key)
        .map(|slot| stored_runtime_value(&slot));
    option_value(payload, heap, budget.as_deref_mut())
}

fn call_cached_map_clear(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
) -> VmResult<Value> {
    crate::runtime_checks::expect_arity("clear", args, 0)?;
    let reference = map_reference(receiver, "method clear")?;
    let Some(heap) = heap.as_deref_mut() else {
        return type_error("method clear");
    };
    map_slots_mut(heap, reference, "method clear")?.clear();
    Ok(Value::Null)
}

fn call_cached_map_extend(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    _budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    crate::runtime_checks::expect_arity("extend", args, 1)?;
    let reference = map_reference(receiver, "method extend")?;
    let Some(heap) = heap.as_deref_mut() else {
        return type_error("method extend");
    };
    let slots = map_slot_entries(heap, &args[0], "method extend")?;
    map_slots_mut(heap, reference, "method extend")?.extend(slots);
    Ok(Value::Null)
}

fn map_slot_entries(
    heap: &HeapExecution<'_>,
    receiver: &Value,
    operation: &'static str,
) -> VmResult<BTreeMap<String, Value>> {
    let values = map_slots(receiver, Some(heap), operation)?;
    if values.values().any(|value| matches!(value, Value::Missing)) {
        return type_error("missing value");
    }
    Ok(values.clone())
}

fn map_slots<'a>(
    receiver: &Value,
    heap: Option<&'a HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<&'a BTreeMap<String, Value>> {
    let reference = map_reference(receiver, operation)?;
    let Some(HeapValue::Map(values)) = heap.and_then(|heap| heap.heap.get(reference)) else {
        return type_error(operation);
    };
    Ok(values)
}

fn map_slots_mut<'a>(
    heap: &'a mut HeapExecution<'_>,
    reference: crate::heap::GcRef,
    operation: &'static str,
) -> VmResult<&'a mut BTreeMap<String, Value>> {
    let Some(HeapValue::Map(values)) = heap.heap.get_mut(reference).ok() else {
        return type_error(operation);
    };
    Ok(values)
}

fn map_key(
    value: &Value,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<String> {
    crate::string_methods::string_value(value, heap, operation).map(str::to_owned)
}

fn map_reference(receiver: &Value, operation: &'static str) -> VmResult<crate::heap::GcRef> {
    match receiver {
        Value::HeapRef(reference) => Ok(*reference),
        _ => type_error(operation),
    }
}

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}
