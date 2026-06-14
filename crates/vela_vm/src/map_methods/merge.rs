use crate::{ExecutionBudget, HeapExecution, Value, VmResult};

use super::{expect_arity, make_map_from_entries, map_slots};

pub(crate) fn merge(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("merge", args, 1)?;
    let mut merged = map_slots(receiver, heap.as_deref(), "method merge")?.entries_vec();
    merged.extend(map_slots(&args[0], heap.as_deref(), "method merge")?.entries_vec());
    make_map_from_entries(merged, heap, budget, "method merge")
}
