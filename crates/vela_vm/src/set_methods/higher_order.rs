use crate::heap_values::make_set_value;
use crate::method_runtime::{MethodRuntime, call_callback};
use crate::option_result::option_value;
use crate::{Value, VmError, VmErrorKind, VmResult};

use super::{expect_arity, push_unique, set_values, type_error};

pub(crate) fn map(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("map", args, 1)?;
    let values = set_values(receiver, runtime.heap.as_deref(), "method map")?;
    let mut mapped = Vec::new();
    for value in values {
        let mapped_value = call_callback(
            &mut runtime,
            "method map",
            &args[0],
            std::slice::from_ref(&value),
            &mapped,
        )?;
        push_unique(
            &mut mapped,
            mapped_value,
            runtime.heap.as_deref(),
            "method map",
        )?;
    }
    make_result_set(mapped, &mut runtime, "method map")
}

pub(crate) fn filter(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("filter", args, 1)?;
    let values = set_values(receiver, runtime.heap.as_deref(), "method filter")?;
    let mut filtered = Vec::new();
    for value in values {
        let predicate = call_callback(
            &mut runtime,
            "method filter",
            &args[0],
            std::slice::from_ref(&value),
            &filtered,
        )?;
        if is_truthy(&predicate) {
            push_unique(
                &mut filtered,
                value,
                runtime.heap.as_deref(),
                "method filter",
            )?;
        }
    }
    make_result_set(filtered, &mut runtime, "method filter")
}

pub(crate) fn find(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("find", args, 1)?;
    for value in set_values(receiver, runtime.heap.as_deref(), "method find")? {
        let predicate = call_callback(
            &mut runtime,
            "method find",
            &args[0],
            std::slice::from_ref(&value),
            &[],
        )?;
        if is_truthy(&predicate) {
            return option_result(Some(value), &mut runtime, "method find");
        }
    }
    option_result(None, &mut runtime, "method find")
}

pub(crate) fn any(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<bool> {
    expect_arity("any", args, 1)?;
    for value in set_values(receiver, runtime.heap.as_deref(), "method any")? {
        let predicate = call_callback(
            &mut runtime,
            "method any",
            &args[0],
            std::slice::from_ref(&value),
            &[],
        )?;
        if is_truthy(&predicate) {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn all(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<bool> {
    expect_arity("all", args, 1)?;
    for value in set_values(receiver, runtime.heap.as_deref(), "method all")? {
        let predicate = call_callback(
            &mut runtime,
            "method all",
            &args[0],
            std::slice::from_ref(&value),
            &[],
        )?;
        if !is_truthy(&predicate) {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(crate) fn count(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<i64> {
    expect_arity("count", args, 1)?;
    let mut count = 0_i64;
    for value in set_values(receiver, runtime.heap.as_deref(), "method count")? {
        let predicate = call_callback(
            &mut runtime,
            "method count",
            &args[0],
            std::slice::from_ref(&value),
            &[],
        )?;
        if is_truthy(&predicate) {
            count = count.checked_add(1).ok_or_else(|| {
                VmError::new(VmErrorKind::TypeMismatch {
                    operation: "method count",
                })
            })?;
        }
    }
    Ok(count)
}

fn is_truthy(value: &Value) -> bool {
    !matches!(value, Value::Bool(false) | Value::Null)
}

fn make_result_set(
    values: Vec<Value>,
    runtime: &mut MethodRuntime<'_, '_, '_>,
    operation: &'static str,
) -> VmResult<Value> {
    let Some(heap) = runtime.heap.as_deref_mut() else {
        return type_error(operation);
    };
    make_set_value(values, heap, runtime.budget.as_deref_mut())
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
