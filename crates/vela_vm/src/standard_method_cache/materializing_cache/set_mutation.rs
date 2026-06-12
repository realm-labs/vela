use crate::heap::HeapValue;
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
        StandardMethodInlineCacheTarget::Add => {
            Some(call_cached_set_add(receiver, args, heap, budget))
        }
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

fn call_cached_set_add(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    crate::runtime_checks::expect_arity("add", args, 1)?;
    let reference = set_reference(receiver, "method add")?;
    let Some(heap) = heap.as_deref_mut() else {
        return type_error("method add");
    };
    let key = SetKey::from_value(&args[0], Some(&*heap), "method add")?;
    if set_slots(heap, reference, "method add")?
        .iter()
        .any(|slot| slot_key(slot, heap).as_ref() == Ok(&key))
    {
        return Ok(Value::Bool(false));
    }
    let slot = store_runtime_value(&args[0], heap, budget.as_deref_mut())?;
    set_slots_mut(heap, reference, "method add")?.push(slot);
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
    let key = SetKey::from_value(&args[0], Some(&*heap), "method remove")?;
    let indexes = set_slots(heap, reference, "method remove")?
        .iter()
        .enumerate()
        .filter_map(|(index, slot)| (slot_key(slot, heap).as_ref() == Ok(&key)).then_some(index))
        .collect::<Vec<_>>();
    let values = set_slots_mut(heap, reference, "method remove")?;
    let before = values.len();
    for index in indexes.into_iter().rev() {
        values.remove(index);
    }
    Ok(Value::Bool(values.len() != before))
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
    set_slots_mut(heap, reference, "method clear")?.clear();
    Ok(Value::Null)
}

fn call_cached_set_extend(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    _budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    crate::runtime_checks::expect_arity("extend", args, 1)?;
    let reference = set_reference(receiver, "method extend")?;
    let Some(heap) = heap.as_deref_mut() else {
        return type_error("method extend");
    };
    let extension_reference = set_reference(&args[0], "method extend")?;
    if reference == extension_reference {
        set_slots(heap, reference, "method extend")?;
        return Ok(Value::Null);
    }
    match set_slot_entry(heap, extension_reference, "method extend")? {
        SetSlotEntry::Empty => {
            set_slots(heap, reference, "method extend")?;
            return Ok(Value::Null);
        }
        SetSlotEntry::Single(slot) => {
            extend_set_slots(heap, reference, &[slot], "method extend")?;
            return Ok(Value::Null);
        }
        SetSlotEntry::Pair(first, second) => {
            extend_set_slots(heap, reference, &[first, second], "method extend")?;
            return Ok(Value::Null);
        }
        SetSlotEntry::Many => {}
    }
    let extension = set_slot_values(heap, extension_reference, "method extend")?;
    extend_set_slots(heap, reference, &extension, "method extend")?;
    Ok(Value::Null)
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
    let values = set_slots(heap, reference, operation)?;
    match values {
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
    operation: &'static str,
) -> VmResult<()> {
    let mut keys = set_slots(heap, reference, operation)?
        .iter()
        .map(|slot| slot_key(slot, heap))
        .collect::<VmResult<Vec<_>>>()?;
    let mut slots = Vec::new();
    for slot in extension {
        let key = SetKey::from_value(slot, Some(&*heap), operation)?;
        if keys.contains(&key) {
            continue;
        }
        keys.push(key);
        slots.push(*slot);
    }
    set_slots_mut(heap, reference, operation)?.extend(slots);
    Ok(())
}

fn set_slot_values(
    heap: &HeapExecution<'_>,
    reference: crate::heap::GcRef,
    operation: &'static str,
) -> VmResult<Vec<Value>> {
    let Some(HeapValue::Set(values)) = heap.heap.get(reference) else {
        return type_error(operation);
    };
    if values.iter().any(|value| matches!(value, Value::Missing)) {
        return type_error("missing value");
    }
    Ok(values.clone())
}

fn set_slots<'a>(
    heap: &'a HeapExecution<'_>,
    reference: crate::heap::GcRef,
    operation: &'static str,
) -> VmResult<&'a [Value]> {
    let Some(HeapValue::Set(values)) = heap.heap.get(reference) else {
        return type_error(operation);
    };
    Ok(values)
}

fn set_slots_mut<'a>(
    heap: &'a mut HeapExecution<'_>,
    reference: crate::heap::GcRef,
    operation: &'static str,
) -> VmResult<&'a mut Vec<Value>> {
    let Some(HeapValue::Set(values)) = heap.heap.get_mut(reference).ok() else {
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

fn slot_key(slot: &Value, heap: &HeapExecution<'_>) -> VmResult<SetKey> {
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

#[derive(Clone, Debug, PartialEq)]
enum SetKey {
    Null,
    Bool(bool),
    Int(i64),
    Float(u64),
    String(String),
}

impl SetKey {
    fn from_value(
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
}

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}
