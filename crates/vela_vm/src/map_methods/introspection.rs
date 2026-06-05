use crate::heap::HeapValue;
use crate::{ExecutionBudget, HeapExecution, Value, VmResult, stored_runtime_value};

use super::{expect_no_args, map_entry, type_error};
use crate::array_methods::{make_array_value, make_string_value};

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
            let values = keys
                .into_iter()
                .map(|key| make_string_value(key, heap, budget, "method keys"))
                .collect::<VmResult<Vec<_>>>()?;
            make_array_value(values, heap, budget, "method keys")
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
            let values = {
                let Some(HeapValue::Map(values)) =
                    heap.as_deref().and_then(|heap| heap.heap.get(*reference))
                else {
                    return type_error("method values");
                };
                values
                    .values()
                    .map(stored_runtime_value)
                    .collect::<Vec<_>>()
            };
            make_array_value(values, heap, budget, "method values")
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
            let entries = {
                let Some(HeapValue::Map(values)) =
                    heap.as_deref().and_then(|heap| heap.heap.get(*reference))
                else {
                    return type_error("method entries");
                };
                values
                    .iter()
                    .map(|(key, value)| (key.clone(), stored_runtime_value(value)))
                    .collect::<Vec<_>>()
            };
            let entries = entries
                .into_iter()
                .map(|(key, value)| map_entry(&key, value, heap, budget))
                .collect::<VmResult<Vec<_>>>()?;
            make_array_value(entries, heap, budget, "method entries")
        }
        _ => type_error("method entries"),
    }
}
