use crate::option_result::option_value;
use crate::{HeapExecution, Value, VmResult};

use super::{expect_arity, index_value, string_value};

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
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("find", args, 1)?;
    let value = string_value(receiver, heap, "method find")?;
    let needle = string_value(&args[0], heap, "method find")?;
    let Some(byte_index) = value.find(needle) else {
        return Ok(option_value(None));
    };
    let char_index = value[..byte_index].chars().count();
    Ok(option_value(Some(Value::Int(
        i64::try_from(char_index).unwrap_or(i64::MAX),
    ))))
}

pub(crate) fn char_at(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("char_at", args, 1)?;
    let value = string_value(receiver, heap, "method char_at")?;
    let index = index_value(&args[0], "method char_at")?;
    Ok(option_value(
        value
            .chars()
            .nth(index)
            .map(|ch| Value::String(ch.to_string())),
    ))
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
