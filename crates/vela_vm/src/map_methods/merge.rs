use std::collections::BTreeMap;

use crate::{HeapExecution, Value, VmResult};

use super::{expect_arity, materialize_map_entries};

pub(crate) fn merge(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("merge", args, 1)?;
    let mut merged = BTreeMap::new();
    for (key, value) in materialize_map_entries(receiver, heap, "method merge")? {
        merged.insert(key, value);
    }
    for (key, value) in materialize_map_entries(&args[0], heap, "method merge")? {
        merged.insert(key, value);
    }
    Ok(Value::Map(merged))
}
