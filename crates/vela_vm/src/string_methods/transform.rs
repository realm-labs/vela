use crate::{HeapExecution, Value, VmError, VmErrorKind, VmResult};

use super::{expect_arity, expect_no_args, index_value, string_value};

pub(crate) fn to_upper(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_no_args("to_upper", args)?;
    string_value(receiver, heap, "method to_upper")
        .map(str::to_uppercase)
        .map(Value::String)
}

pub(crate) fn to_lower(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_no_args("to_lower", args)?;
    string_value(receiver, heap, "method to_lower")
        .map(str::to_lowercase)
        .map(Value::String)
}

pub(crate) fn trim(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    trim_with(receiver, args, heap, "trim", "method trim", str::trim)
}

pub(crate) fn trim_start(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    trim_with(
        receiver,
        args,
        heap,
        "trim_start",
        "method trim_start",
        str::trim_start,
    )
}

pub(crate) fn trim_end(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    trim_with(
        receiver,
        args,
        heap,
        "trim_end",
        "method trim_end",
        str::trim_end,
    )
}

pub(crate) fn replace(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("replace", args, 2)?;
    let value = string_value(receiver, heap, "method replace")?;
    let from = string_value(&args[0], heap, "method replace")?;
    let to = string_value(&args[1], heap, "method replace")?;
    Ok(Value::String(value.replace(from, to)))
}

pub(crate) fn repeat(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("repeat", args, 1)?;
    let value = string_value(receiver, heap, "method repeat")?;
    let count = index_value(&args[0], "method repeat")?;
    value.len().checked_mul(count).ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "method repeat",
        })
    })?;
    Ok(Value::String(value.repeat(count)))
}

fn trim_with<'a>(
    receiver: &'a Value,
    args: &[Value],
    heap: Option<&'a HeapExecution<'_>>,
    method: &str,
    operation: &'static str,
    trim: impl FnOnce(&'a str) -> &'a str,
) -> VmResult<Value> {
    expect_no_args(method, args)?;
    string_value(receiver, heap, operation)
        .map(trim)
        .map(str::to_owned)
        .map(Value::String)
}
