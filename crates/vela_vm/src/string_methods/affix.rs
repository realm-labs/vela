use crate::option_result::option_value;
use crate::{HeapExecution, Value, VmResult};

use super::{expect_arity, string_value};

pub(crate) fn strip_prefix(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    strip_affix(
        receiver,
        args,
        heap,
        "strip_prefix",
        "method strip_prefix",
        str::strip_prefix,
    )
}

pub(crate) fn strip_suffix(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    strip_affix(
        receiver,
        args,
        heap,
        "strip_suffix",
        "method strip_suffix",
        str::strip_suffix,
    )
}

fn strip_affix<'a>(
    receiver: &'a Value,
    args: &'a [Value],
    heap: Option<&'a HeapExecution<'_>>,
    method: &str,
    operation: &'static str,
    strip: impl FnOnce(&'a str, &'a str) -> Option<&'a str>,
) -> VmResult<Value> {
    expect_arity(method, args, 1)?;
    let value = string_value(receiver, heap, operation)?;
    let affix = string_value(&args[0], heap, operation)?;
    Ok(option_value(
        strip(value, affix).map(|stripped| Value::String(stripped.to_owned())),
    ))
}
