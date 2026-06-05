use crate::option_result::option_value;
use crate::{ExecutionBudget, HeapExecution, Value, VmResult};

use super::{expect_arity, expect_no_args, make_array, make_string, string_value};

pub(crate) fn split(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("split", args, 1)?;
    let value = string_value(receiver, heap.as_deref(), "method split")?;
    let separator = string_value(&args[0], heap.as_deref(), "method split")?;
    let parts = value
        .split(separator)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    string_array(parts, heap, budget, "method split")
}

pub(crate) fn split_once(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("split_once", args, 1)?;
    let value = string_value(receiver, heap.as_deref(), "method split_once")?;
    let separator = string_value(&args[0], heap.as_deref(), "method split_once")?;
    let payload = value
        .split_once(separator)
        .map(|(before, after)| [before.to_owned(), after.to_owned()]);
    let payload = payload
        .map(|parts| string_array(parts.into(), heap, budget, "method split_once"))
        .transpose()?;
    let Some(heap) = heap.as_deref_mut() else {
        return super::type_error("method split_once");
    };
    option_value(payload, heap, budget.as_deref_mut())
}

pub(crate) fn split_lines(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_no_args("split_lines", args)?;
    let value = string_value(receiver, heap.as_deref(), "method split_lines")?;
    let parts = value.lines().map(str::to_owned).collect::<Vec<_>>();
    string_array(parts, heap, budget, "method split_lines")
}

pub(crate) fn split_whitespace(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_no_args("split_whitespace", args)?;
    let value = string_value(receiver, heap.as_deref(), "method split_whitespace")?;
    let parts = value
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    string_array(parts, heap, budget, "method split_whitespace")
}

fn string_array(
    parts: Vec<String>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<Value> {
    let values = parts
        .into_iter()
        .map(|part| make_string(part, heap, budget, operation))
        .collect::<VmResult<Vec<_>>>()?;
    make_array(values, heap, budget, operation)
}
