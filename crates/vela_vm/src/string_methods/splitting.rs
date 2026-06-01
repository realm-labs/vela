use crate::option_result::option_value;
use crate::{HeapExecution, Value, VmResult};

use super::{expect_arity, expect_no_args, string_value};

pub(crate) fn split(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("split", args, 1)?;
    let value = string_value(receiver, heap, "method split")?;
    let separator = string_value(&args[0], heap, "method split")?;
    Ok(Value::Array(
        value
            .split(separator)
            .map(|part| Value::String(part.to_owned()))
            .collect(),
    ))
}

pub(crate) fn split_once(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("split_once", args, 1)?;
    let value = string_value(receiver, heap, "method split_once")?;
    let separator = string_value(&args[0], heap, "method split_once")?;
    Ok(option_value(value.split_once(separator).map(
        |(before, after)| {
            Value::Array(vec![
                Value::String(before.to_owned()),
                Value::String(after.to_owned()),
            ])
        },
    )))
}

pub(crate) fn split_lines(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_no_args("split_lines", args)?;
    let value = string_value(receiver, heap, "method split_lines")?;
    Ok(Value::Array(
        value
            .lines()
            .map(|line| Value::String(line.to_owned()))
            .collect(),
    ))
}

pub(crate) fn split_whitespace(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_no_args("split_whitespace", args)?;
    let value = string_value(receiver, heap, "method split_whitespace")?;
    Ok(Value::Array(
        value
            .split_whitespace()
            .map(|word| Value::String(word.to_owned()))
            .collect(),
    ))
}
