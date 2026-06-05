use crate::option_result::option_value;
use crate::{ExecutionBudget, HeapExecution, Value, VmResult};

use super::{expect_arity, make_string, string_value};

pub(crate) fn strip_prefix(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("strip_prefix", args, 1)?;
    let payload = {
        let value = string_value(receiver, heap.as_deref(), "method strip_prefix")?;
        let affix = string_value(&args[0], heap.as_deref(), "method strip_prefix")?;
        value.strip_prefix(affix).map(str::to_owned)
    };
    let payload = payload
        .map(|value| make_string(value, heap, budget, "method strip_prefix"))
        .transpose()?;
    let Some(heap) = heap.as_deref_mut() else {
        return super::type_error("method strip_prefix");
    };
    option_value(payload, heap, budget.as_deref_mut())
}

pub(crate) fn strip_suffix(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("strip_suffix", args, 1)?;
    let payload = {
        let value = string_value(receiver, heap.as_deref(), "method strip_suffix")?;
        let affix = string_value(&args[0], heap.as_deref(), "method strip_suffix")?;
        value.strip_suffix(affix).map(str::to_owned)
    };
    let payload = payload
        .map(|value| make_string(value, heap, budget, "method strip_suffix"))
        .transpose()?;
    let Some(heap) = heap.as_deref_mut() else {
        return super::type_error("method strip_suffix");
    };
    option_value(payload, heap, budget.as_deref_mut())
}
