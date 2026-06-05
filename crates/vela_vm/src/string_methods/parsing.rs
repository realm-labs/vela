use crate::option_result::option_value;
use crate::{ExecutionBudget, HeapExecution, Value, VmResult};

use super::{expect_no_args, string_value};

pub(crate) fn parse_int(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_no_args("parse_int", args)?;
    let value = string_value(receiver, heap.as_deref(), "method parse_int")?;
    let payload = value.parse::<i64>().ok().map(Value::Int);
    let Some(heap) = heap.as_deref_mut() else {
        return super::type_error("method parse_int");
    };
    option_value(payload, heap, budget.as_deref_mut())
}

pub(crate) fn parse_float(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_no_args("parse_float", args)?;
    let value = string_value(receiver, heap.as_deref(), "method parse_float")?;
    let payload = value
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .map(Value::Float);
    let Some(heap) = heap.as_deref_mut() else {
        return super::type_error("method parse_float");
    };
    option_value(payload, heap, budget.as_deref_mut())
}

pub(crate) fn parse_bool(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_no_args("parse_bool", args)?;
    let value = string_value(receiver, heap.as_deref(), "method parse_bool")?;
    let payload = match value {
        "true" => Some(Value::Bool(true)),
        "false" => Some(Value::Bool(false)),
        _ => None,
    };
    let Some(heap) = heap.as_deref_mut() else {
        return super::type_error("method parse_bool");
    };
    option_value(payload, heap, budget.as_deref_mut())
}
