use crate::option_result::option_value;
use crate::{ExecutionBudget, HeapExecution, Value, VmResult};

use super::{expect_arity, index_value, make_string, string_value};

pub(crate) fn contains(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    predicate(
        receiver,
        "contains",
        "method contains",
        args,
        heap,
        |value, needle| value.contains(needle),
    )
}

pub(crate) fn starts_with(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    predicate(
        receiver,
        "starts_with",
        "method starts_with",
        args,
        heap,
        |value, prefix| value.starts_with(prefix),
    )
}

pub(crate) fn ends_with(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    predicate(
        receiver,
        "ends_with",
        "method ends_with",
        args,
        heap,
        |value, suffix| value.ends_with(suffix),
    )
}

pub(crate) fn find(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("find", args, 1)?;
    let value = string_value(receiver, heap.as_deref(), "method find")?;
    let needle = string_value(&args[0], heap.as_deref(), "method find")?;
    let payload = value.find(needle).map(|byte_index| {
        let char_index = value[..byte_index].chars().count();
        Value::Int(i64::try_from(char_index).unwrap_or(i64::MAX))
    });
    let Some(heap) = heap.as_deref_mut() else {
        return super::type_error("method find");
    };
    option_value(payload, heap, budget.as_deref_mut())
}

pub(crate) fn char_at(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("char_at", args, 1)?;
    let value = string_value(receiver, heap.as_deref(), "method char_at")?;
    let index = index_value(&args[0], "method char_at")?;
    let payload = value
        .chars()
        .nth(index)
        .map(|ch| make_string(ch.to_string(), heap, budget, "method char_at"))
        .transpose()?;
    let Some(heap) = heap.as_deref_mut() else {
        return super::type_error("method char_at");
    };
    option_value(payload, heap, budget.as_deref_mut())
}

fn predicate(
    receiver: &Value,
    method: &str,
    operation: &'static str,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
    predicate: impl FnOnce(&str, &str) -> bool,
) -> VmResult<bool> {
    expect_arity(method, args, 1)?;
    let receiver = string_value(receiver, heap, operation)?;
    let needle = string_value(&args[0], heap, operation)?;
    Ok(predicate(receiver, needle))
}
