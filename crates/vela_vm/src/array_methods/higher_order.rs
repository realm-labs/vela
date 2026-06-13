use crate::iteration::{self, IteratorState};
use crate::method_runtime::MethodRuntime;
use crate::{Value, VmResult};

use super::{array_values, expect_arity, make_array_value, option_value};

pub(crate) fn map(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("map", args, 1)?;
    let values = array_values(receiver, runtime.heap.as_deref(), "method map")?;
    let mut iterator = IteratorState::map(IteratorState::from_values(values), args[0]);
    let mapped = iteration::collect_values(&mut iterator, &mut runtime, "method map")?;
    make_array_value(mapped, &mut runtime.heap, &mut runtime.budget, "method map")
}

pub(crate) fn filter(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("filter", args, 1)?;
    let values = array_values(receiver, runtime.heap.as_deref(), "method filter")?;
    let mut iterator = IteratorState::filter(IteratorState::from_values(values), args[0]);
    let filtered = iteration::collect_values(&mut iterator, &mut runtime, "method filter")?;
    make_array_value(
        filtered,
        &mut runtime.heap,
        &mut runtime.budget,
        "method filter",
    )
}

pub(crate) fn find(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("find", args, 1)?;
    let values = array_values(receiver, runtime.heap.as_deref(), "method find")?;
    let found = iteration::callback_find(
        &mut IteratorState::from_values(values),
        &mut runtime,
        "method find",
        args[0],
    )?;
    option_value(
        if found.is_some() { "Some" } else { "None" },
        found,
        &mut runtime.heap,
        &mut runtime.budget,
    )
}

pub(crate) fn any(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<bool> {
    expect_arity("any", args, 1)?;
    let values = array_values(receiver, runtime.heap.as_deref(), "method any")?;
    iteration::callback_any(
        &mut IteratorState::from_values(values),
        &mut runtime,
        "method any",
        args[0],
    )
}

pub(crate) fn all(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<bool> {
    expect_arity("all", args, 1)?;
    let values = array_values(receiver, runtime.heap.as_deref(), "method all")?;
    iteration::callback_all(
        &mut IteratorState::from_values(values),
        &mut runtime,
        "method all",
        args[0],
    )
}

pub(crate) fn count(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<i64> {
    expect_arity("count", args, 1)?;
    let values = array_values(receiver, runtime.heap.as_deref(), "method count")?;
    iteration::callback_count(
        &mut IteratorState::from_values(values),
        &mut runtime,
        "method count",
        args[0],
    )
}
