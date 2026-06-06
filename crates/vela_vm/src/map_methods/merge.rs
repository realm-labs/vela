use std::collections::BTreeMap;

use crate::array_methods::make_map_value;
use crate::{ExecutionBudget, HeapExecution, Value, VmResult, stored_runtime_value};

use super::{expect_arity, map_slots};

pub(crate) fn merge(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("merge", args, 1)?;
    let mut merged = BTreeMap::new();
    for (key, value) in map_slots(receiver, heap.as_deref(), "method merge")? {
        merged.insert(key.clone(), stored_runtime_value(value));
    }
    for (key, value) in map_slots(&args[0], heap.as_deref(), "method merge")? {
        merged.insert(key.clone(), stored_runtime_value(value));
    }
    make_map_value(merged, heap, budget, "method merge")
}
