use crate::heap::HeapValue;
use crate::heap_values::allocate_heap_value;
use crate::iteration::IteratorState;
use crate::{ExecutionBudget, HeapExecution, Value, VmResult};

use super::{expect_no_args, type_error};

pub(crate) fn keys(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_no_args("keys", args)?;
    match receiver {
        Value::HeapRef(reference) => {
            let keys = {
                let Some(HeapValue::Map(values)) =
                    heap.as_deref().and_then(|heap| heap.heap.get(*reference))
                else {
                    return type_error("method keys");
                };
                values.keys().cloned().collect::<Vec<_>>()
            };
            allocate_iterator(
                IteratorState::from_map_keys_source(*reference, keys),
                heap,
                budget,
                "method keys",
            )
        }
        _ => type_error("method keys"),
    }
}

pub(crate) fn values(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_no_args("values", args)?;
    match receiver {
        Value::HeapRef(reference) => {
            let keys = {
                let Some(HeapValue::Map(values)) =
                    heap.as_deref().and_then(|heap| heap.heap.get(*reference))
                else {
                    return type_error("method values");
                };
                values.keys().cloned().collect::<Vec<_>>()
            };
            allocate_iterator(
                IteratorState::from_map_values_source(*reference, keys),
                heap,
                budget,
                "method values",
            )
        }
        _ => type_error("method values"),
    }
}

pub(crate) fn entries(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_no_args("entries", args)?;
    match receiver {
        Value::HeapRef(reference) => {
            let keys = {
                let Some(HeapValue::Map(values)) =
                    heap.as_deref().and_then(|heap| heap.heap.get(*reference))
                else {
                    return type_error("method entries");
                };
                values.keys().cloned().collect::<Vec<_>>()
            };
            allocate_iterator(
                IteratorState::from_map_entries_source(*reference, keys),
                heap,
                budget,
                "method entries",
            )
        }
        _ => type_error("method entries"),
    }
}

fn allocate_iterator(
    iterator: IteratorState,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<Value> {
    let Some(heap) = heap.as_deref_mut() else {
        return type_error(operation);
    };
    allocate_heap_value(HeapValue::Iterator(iterator), heap, budget.as_deref_mut())
}
