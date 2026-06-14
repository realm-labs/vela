use crate::collection_mutation;
use crate::heap::HeapValue;
use crate::{ExecutionBudget, HeapExecution, Value, VmResult, store_runtime_value};

use super::{SetKey, expect_arity, set_values, type_error};

pub(crate) fn add(
    receiver: &mut Value,
    args: &[Value],
    heap: Option<&mut HeapExecution<'_>>,
    mut budget: Option<&mut ExecutionBudget>,
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
            if values.contains_key(&key) {
                return Ok(Value::Bool(false));
            }
            let slot = store_runtime_value(&args[0], heap, budget.as_deref_mut())?;
            collection_mutation::push_set_slot(heap, *reference, slot, budget, "method add")?;
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
            if !values.contains_key(&key) {
                return Ok(Value::Bool(false));
            }
            let changed = collection_mutation::remove_set_slot(
                heap,
                *reference,
                &key,
                None,
                "method remove",
            )?;
            Ok(Value::Bool(changed))
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
            collection_mutation::clear_set(heap, *reference, None, "method clear")?;
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
                .values()
                .map(|slot| SetKey::from_value(slot, Some(&*heap), "method extend"))
                .collect::<VmResult<Vec<_>>>()?;
            let mut slots = Vec::new();
            for value in extension {
                let key = SetKey::from_value(&value, Some(&*heap), "method extend")?;
                if keys.contains(&key) {
                    continue;
                }
                keys.push(key);
                slots.push(store_runtime_value(&value, heap, budget.as_deref_mut())?);
            }
            collection_mutation::extend_set_slots(
                heap,
                *reference,
                slots,
                budget,
                "method extend",
            )?;
            Ok(Value::Null)
        }
        _ => type_error("method extend"),
    }
}
