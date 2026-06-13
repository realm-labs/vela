use crate::option_result::option_value;
use crate::{ExecutionBudget, HeapExecution, Value, VmResult};

use super::{expect_arity, string_value};

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
    let payload = value
        .find(needle)
        .map(|byte_index| Value::i64(i64::try_from(byte_index).unwrap_or(i64::MAX)));
    let Some(heap) = heap.as_deref_mut() else {
        return super::type_error("method find");
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
