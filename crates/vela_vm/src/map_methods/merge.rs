use crate::script_map::ScriptMap;
use crate::{ExecutionBudget, HeapExecution, Value, VmResult};

use super::{expect_arity, make_map_from_entries, map_slots};

pub(crate) fn merge(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("merge", args, 1)?;
    let left = map_slots(receiver, heap.as_deref(), "method merge")?;
    let right = map_slots(&args[0], heap.as_deref(), "method merge")?;
    let merged = merge_payload(left.entries_vec(), right);
    make_map_from_entries(merged, heap, budget, "method merge")
}

pub(crate) fn merge_payload(
    mut left: Vec<(Value, Value)>,
    right: &ScriptMap,
) -> Vec<(Value, Value)> {
    left.extend(right.entries_vec());
    left
}
