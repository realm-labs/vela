use crate::option_result::option_value;
use crate::{HeapExecution, Value, VmResult};

use super::{expect_no_args, string_value};

pub(crate) fn parse_int(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_no_args("parse_int", args)?;
    let value = string_value(receiver, heap, "method parse_int")?;
    Ok(option_value(value.parse::<i64>().ok().map(Value::Int)))
}

pub(crate) fn parse_float(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_no_args("parse_float", args)?;
    let value = string_value(receiver, heap, "method parse_float")?;
    Ok(option_value(
        value
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
            .map(Value::Float),
    ))
}

pub(crate) fn parse_bool(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_no_args("parse_bool", args)?;
    let value = string_value(receiver, heap, "method parse_bool")?;
    Ok(option_value(match value {
        "true" => Some(Value::Bool(true)),
        "false" => Some(Value::Bool(false)),
        _ => None,
    }))
}
