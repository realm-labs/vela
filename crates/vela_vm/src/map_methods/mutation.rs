use crate::heap::HeapValue;
use crate::option_result::option_value;
use crate::{
    ExecutionBudget, HeapExecution, Value, VmResult, value_from_heap_slot, value_to_heap_slot,
};

use super::{expect_arity, map_key, materialize_map_entries, type_error};

pub(crate) fn set(
    receiver: &mut Value,
    args: &[Value],
    heap: Option<&mut HeapExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("set", args, 2)?;
    let key = map_key(&args[0], heap.as_deref())?;
    match receiver {
        Value::HeapRef(reference) => {
            let Some(heap) = heap else {
                return type_error("method set");
            };
            let slot = value_to_heap_slot(&args[1], heap, budget)?;
            let Some(HeapValue::Map(values)) = heap.heap.get_mut(*reference).ok() else {
                return type_error("method set");
            };
            values.insert(key, slot);
            Ok(args[1])
        }
        _ => type_error("method set"),
    }
}

pub(crate) fn remove(
    receiver: &mut Value,
    args: &[Value],
    heap: Option<&mut HeapExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("remove", args, 1)?;
    let key = map_key(&args[0], heap.as_deref())?;
    match receiver {
        Value::HeapRef(reference) => {
            let Some(heap) = heap else {
                return type_error("method remove");
            };
            let Some(HeapValue::Map(values)) = heap.heap.get_mut(*reference).ok() else {
                return type_error("method remove");
            };
            let payload = values.remove(&key).map(|slot| value_from_heap_slot(&slot));
            option_value(payload, heap, budget)
        }
        _ => type_error("method remove"),
    }
}

pub(crate) fn clear(
    receiver: &mut Value,
    args: &[Value],
    heap: Option<&mut HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("clear", args, 0)?;
    match receiver {
        Value::HeapRef(reference) => {
            let Some(heap) = heap else {
                return type_error("method clear");
            };
            let Some(HeapValue::Map(values)) = heap.heap.get_mut(*reference).ok() else {
                return type_error("method clear");
            };
            values.clear();
            Ok(Value::Null)
        }
        _ => type_error("method clear"),
    }
}

pub(crate) fn extend(
    receiver: &mut Value,
    args: &[Value],
    heap: Option<&mut HeapExecution<'_>>,
    mut budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("extend", args, 1)?;
    let entries = materialize_map_entries(&args[0], heap.as_deref(), "method extend")?;
    match receiver {
        Value::HeapRef(reference) => {
            let Some(heap) = heap else {
                return type_error("method extend");
            };
            let mut slots = Vec::with_capacity(entries.len());
            for (key, value) in entries {
                slots.push((
                    key,
                    value_to_heap_slot(&value, heap, budget.as_deref_mut())?,
                ));
            }
            let Some(HeapValue::Map(values)) = heap.heap.get_mut(*reference).ok() else {
                return type_error("method extend");
            };
            values.extend(slots);
            Ok(Value::Null)
        }
        _ => type_error("method extend"),
    }
}
