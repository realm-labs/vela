use crate::runtime_view::MapView;
use crate::{HeapExecution, Value, VmResult, value_from_heap_slot};

use super::{expect_no_args, map_entry};

pub(crate) fn keys(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_no_args("keys", args)?;
    match MapView::from_value(receiver, heap, "method keys")? {
        MapView::Values(values) => Ok(Value::Array(
            values
                .keys()
                .map(|key| Value::String(key.clone()))
                .collect(),
        )),
        MapView::Slots(values, _) => Ok(Value::Array(
            values
                .keys()
                .map(|key| Value::String(key.clone()))
                .collect(),
        )),
    }
}

pub(crate) fn values(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_no_args("values", args)?;
    match MapView::from_value(receiver, heap, "method values")? {
        MapView::Values(values) => Ok(Value::Array(values.values().cloned().collect())),
        MapView::Slots(values, _) => Ok(Value::Array(
            values.values().map(value_from_heap_slot).collect(),
        )),
    }
}

pub(crate) fn entries(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_no_args("entries", args)?;
    match MapView::from_value(receiver, heap, "method entries")? {
        MapView::Values(values) => Ok(Value::Array(
            values
                .iter()
                .map(|(key, value)| map_entry(key, value.clone()))
                .collect(),
        )),
        MapView::Slots(values, _) => Ok(Value::Array(
            values
                .iter()
                .map(|(key, value)| map_entry(key, value_from_heap_slot(value)))
                .collect(),
        )),
    }
}
