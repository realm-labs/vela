use crate::collection_mutation;
use crate::option_result::option_value;
use crate::{ExecutionBudget, HeapExecution, Value, VmResult, store_runtime_value};

use super::{expect_arity, map_entries, map_key, type_error};

pub(crate) fn set(
    receiver: &mut Value,
    args: &[Value],
    heap: Option<&mut HeapExecution<'_>>,
    mut budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("set", args, 2)?;
    let key = map_key(&args[0], heap.as_deref())?;
    match receiver {
        Value::HeapRef(reference) => {
            let Some(heap) = heap else {
                return type_error("method set");
            };
            let slot = store_runtime_value(&args[1], heap, budget.as_deref_mut())?;
            collection_mutation::insert_map_slot(
                heap,
                *reference,
                key,
                slot,
                budget,
                "method set",
            )?;
            Ok(args[1])
        }
        _ => type_error("method set"),
    }
}

pub(crate) fn remove(
    receiver: &mut Value,
    args: &[Value],
    heap: Option<&mut HeapExecution<'_>>,
    mut budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("remove", args, 1)?;
    let key = map_key(&args[0], heap.as_deref())?;
    match receiver {
        Value::HeapRef(reference) => {
            let Some(heap) = heap else {
                return type_error("method remove");
            };
            let payload = collection_mutation::remove_map_slot(
                heap,
                *reference,
                &key,
                budget.as_deref_mut(),
                "method remove",
            )?;
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
            collection_mutation::clear_map(heap, *reference, None, "method clear")?;
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
    let entries = map_entries(&args[0], heap.as_deref(), "method extend")?;
    match receiver {
        Value::HeapRef(reference) => {
            let Some(heap) = heap else {
                return type_error("method extend");
            };
            let mut slots = Vec::with_capacity(entries.len());
            for (key, value) in entries {
                slots.push((
                    key,
                    store_runtime_value(&value, heap, budget.as_deref_mut())?,
                ));
            }
            collection_mutation::extend_map_slots(
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
