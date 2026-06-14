use crate::collection_mutation::check_collection_len;
use crate::heap::HeapValue;
use crate::heap_values::allocate_heap_value;
use crate::iteration::{self, IteratorState};
use crate::method_runtime::MethodRuntime;
use crate::option_result::option_value;
use crate::script_set::ScriptSet;
use crate::{Value, VmResult};

use super::{expect_arity, set_values, type_error};

pub(crate) fn map(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("map", args, 1)?;
    let values = set_values(receiver, runtime.heap.as_deref(), "method map")?;
    let mut iterator = IteratorState::map(IteratorState::from_values(values), args[0]);
    let mapped = collect_unique_values(&mut iterator, &mut runtime, "method map")?;
    make_result_set(mapped, &mut runtime, "method map")
}

pub(crate) fn filter(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("filter", args, 1)?;
    let values = set_values(receiver, runtime.heap.as_deref(), "method filter")?;
    let mut iterator = IteratorState::filter(IteratorState::from_values(values), args[0]);
    let filtered = collect_unique_values(&mut iterator, &mut runtime, "method filter")?;
    make_result_set(filtered, &mut runtime, "method filter")
}

pub(crate) fn find(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("find", args, 1)?;
    let values = set_values(receiver, runtime.heap.as_deref(), "method find")?;
    let found = iteration::callback_find(
        &mut IteratorState::from_values(values),
        &mut runtime,
        "method find",
        args[0],
    )?;
    option_result(found, &mut runtime, "method find")
}

pub(crate) fn any(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<bool> {
    expect_arity("any", args, 1)?;
    let values = set_values(receiver, runtime.heap.as_deref(), "method any")?;
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
    let values = set_values(receiver, runtime.heap.as_deref(), "method all")?;
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
    let values = set_values(receiver, runtime.heap.as_deref(), "method count")?;
    iteration::callback_count(
        &mut IteratorState::from_values(values),
        &mut runtime,
        "method count",
        args[0],
    )
}

fn make_result_set(
    values: ScriptSet,
    runtime: &mut MethodRuntime<'_, '_, '_>,
    operation: &'static str,
) -> VmResult<Value> {
    let Some(heap) = runtime.heap.as_deref_mut() else {
        return type_error(operation);
    };
    check_result_set_len(values.len(), runtime.budget.as_deref())?;
    allocate_heap_value(HeapValue::Set(values), heap, runtime.budget.as_deref_mut())
}

fn option_result(
    payload: Option<Value>,
    runtime: &mut MethodRuntime<'_, '_, '_>,
    operation: &'static str,
) -> VmResult<Value> {
    let Some(heap) = runtime.heap.as_deref_mut() else {
        return type_error(operation);
    };
    option_value(payload, heap, runtime.budget.as_deref_mut())
}

fn collect_unique_values(
    iterator: &mut IteratorState,
    runtime: &mut MethodRuntime<'_, '_, '_>,
    operation: &'static str,
) -> VmResult<ScriptSet> {
    let mut values = ScriptSet::new();
    let mut protected = Vec::new();
    while let Some(value) = iterator.next_with_runtime(runtime, operation, &protected)? {
        if values.insert(value, runtime.heap.as_deref(), operation)? {
            protected.push(value);
        }
    }
    Ok(values)
}

fn check_result_set_len(len: usize, budget: Option<&crate::ExecutionBudget>) -> VmResult<()> {
    check_collection_len("set", 0, len, budget, |budget| {
        budget.collection_limits().max_set_len
    })
}
