use crate::option_result::option_value;
use crate::runtime_view::MapView;
use crate::{HeapExecution, Value, VmResult, string_methods};

use super::expect_arity;

pub(crate) fn has(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    expect_arity("has", args, 1)?;
    let key = lookup_key(&args[0], heap)?;
    MapView::from_value(receiver, heap, "method has").map(|values| values.contains_key(key))
}

pub(crate) fn get(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("get", args, 1)?;
    let key = lookup_key(&args[0], heap)?;
    MapView::from_value(receiver, heap, "method get")
        .map(|values| option_value(values.get_owned(key)))
}

pub(crate) fn get_or(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("get_or", args, 2)?;
    let key = lookup_key(&args[0], heap)?;
    MapView::from_value(receiver, heap, "method get_or")
        .map(|values| values.get_owned(key).unwrap_or_else(|| args[1].clone()))
}

fn lookup_key<'a>(value: &'a Value, heap: Option<&'a HeapExecution<'_>>) -> VmResult<&'a str> {
    string_methods::string_value(value, heap, "map key")
}
