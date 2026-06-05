use crate::{ExecutionBudget, HeapExecution, Value, VmError, VmErrorKind, VmResult};

use super::{expect_arity, expect_no_args, index_value, make_string, string_value};

pub(crate) fn to_upper(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_no_args("to_upper", args)?;
    let value = string_value(receiver, heap.as_deref(), "method to_upper")?.to_uppercase();
    make_string(value, heap, budget, "method to_upper")
}

pub(crate) fn to_lower(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_no_args("to_lower", args)?;
    let value = string_value(receiver, heap.as_deref(), "method to_lower")?.to_lowercase();
    make_string(value, heap, budget, "method to_lower")
}

pub(crate) fn trim(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_no_args("trim", args)?;
    let value = string_value(receiver, heap.as_deref(), "method trim")?
        .trim()
        .to_owned();
    make_string(value, heap, budget, "method trim")
}

pub(crate) fn trim_start(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_no_args("trim_start", args)?;
    let value = string_value(receiver, heap.as_deref(), "method trim_start")?
        .trim_start()
        .to_owned();
    make_string(value, heap, budget, "method trim_start")
}

pub(crate) fn trim_end(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_no_args("trim_end", args)?;
    let value = string_value(receiver, heap.as_deref(), "method trim_end")?
        .trim_end()
        .to_owned();
    make_string(value, heap, budget, "method trim_end")
}

pub(crate) fn replace(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("replace", args, 2)?;
    let value = string_value(receiver, heap.as_deref(), "method replace")?;
    let from = string_value(&args[0], heap.as_deref(), "method replace")?;
    let to = string_value(&args[1], heap.as_deref(), "method replace")?;
    let value = value.replace(from, to);
    make_string(value, heap, budget, "method replace")
}

pub(crate) fn repeat(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("repeat", args, 1)?;
    let value = string_value(receiver, heap.as_deref(), "method repeat")?;
    let count = index_value(&args[0], "method repeat")?;
    value.len().checked_mul(count).ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "method repeat",
        })
    })?;
    let value = value.repeat(count);
    make_string(value, heap, budget, "method repeat")
}
