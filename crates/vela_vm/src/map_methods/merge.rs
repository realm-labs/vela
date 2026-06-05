use std::collections::BTreeMap;

use crate::array_methods::make_map_value;
use crate::{ExecutionBudget, HeapExecution, Value, VmResult};

use super::{expect_arity, map_entries};

pub(crate) fn merge(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("merge", args, 1)?;
    let mut merged = BTreeMap::new();
    for (key, value) in map_entries(receiver, heap.as_deref(), "method merge")? {
        merged.insert(key, value);
    }
    for (key, value) in map_entries(&args[0], heap.as_deref(), "method merge")? {
        merged.insert(key, value);
    }
    make_map_value(merged, heap, budget, "method merge")
}
