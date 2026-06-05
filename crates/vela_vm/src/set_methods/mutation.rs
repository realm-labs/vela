use crate::heap::HeapValue;
use crate::{ExecutionBudget, HeapExecution, Value, VmResult, value_to_heap_slot};

use super::{SetKey, expect_arity, set_values, slot_key, type_error};

pub(crate) fn add(
    receiver: &mut Value,
    args: &[Value],
    heap: Option<&mut HeapExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("add", args, 1)?;
    match receiver {
        Value::HeapRef(reference) => {
            let Some(heap) = heap else {
                return type_error("method add");
            };
            let key = SetKey::from_value(&args[0], Some(&*heap), "method add")?;
            let Some(HeapValue::Set(values)) = heap.heap.get(*reference) else {
                return type_error("method add");
            };
            if values
                .iter()
                .any(|value| slot_key(value, &*heap).as_ref() == Ok(&key))
            {
                return Ok(Value::Bool(false));
            }
            let slot = value_to_heap_slot(&args[0], heap, budget)?;
            let Some(HeapValue::Set(values)) = heap.heap.get_mut(*reference).ok() else {
                return type_error("method add");
            };
            values.push(slot);
            Ok(Value::Bool(true))
        }
        _ => type_error("method add"),
    }
}

pub(crate) fn remove(
    receiver: &mut Value,
    args: &[Value],
    heap: Option<&mut HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("remove", args, 1)?;
    match receiver {
        Value::HeapRef(reference) => {
            let Some(heap) = heap else {
                return type_error("method remove");
            };
            let key = SetKey::from_value(&args[0], Some(&*heap), "method remove")?;
            let Some(HeapValue::Set(values)) = heap.heap.get(*reference) else {
                return type_error("method remove");
            };
            let indexes = values
                .iter()
                .enumerate()
                .filter_map(|(index, value)| {
                    (slot_key(value, &*heap).as_ref() == Ok(&key)).then_some(index)
                })
                .collect::<Vec<_>>();
            let Some(HeapValue::Set(values)) = heap.heap.get_mut(*reference).ok() else {
                return type_error("method remove");
            };
            let before = values.len();
            for index in indexes.into_iter().rev() {
                values.remove(index);
            }
            Ok(Value::Bool(values.len() != before))
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
            let Some(HeapValue::Set(values)) = heap.heap.get_mut(*reference).ok() else {
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
    let extension = set_values(&args[0], heap.as_deref(), "method extend")?;
    match receiver {
        Value::HeapRef(reference) => {
            let Some(heap) = heap else {
                return type_error("method extend");
            };
            let Some(HeapValue::Set(values)) = heap.heap.get(*reference) else {
                return type_error("method extend");
            };
            let mut keys = values
                .iter()
                .map(|slot| slot_key(slot, &*heap))
                .collect::<VmResult<Vec<_>>>()?;
            let mut slots = Vec::new();
            for value in extension {
                let key = SetKey::from_value(&value, Some(&*heap), "method extend")?;
                if keys.contains(&key) {
                    continue;
                }
                keys.push(key);
                slots.push(value_to_heap_slot(&value, heap, budget.as_deref_mut())?);
            }
            let Some(HeapValue::Set(values)) = heap.heap.get_mut(*reference).ok() else {
                return type_error("method extend");
            };
            values.extend(slots);
            Ok(Value::Null)
        }
        _ => type_error("method extend"),
    }
}
