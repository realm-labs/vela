use crate::method_runtime::MethodRuntime;
use crate::{Value, VmError, VmErrorKind, VmResult};

use super::{array_values, call_unary_callback, expect_arity, is_truthy, option_value};

pub(crate) fn map(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("map", args, 1)?;
    let values = array_values(receiver, runtime.heap.as_deref(), "method map")?;
    let mut mapped = Vec::with_capacity(values.len());
    for value in values {
        mapped.push(call_unary_callback(
            &mut runtime,
            "method map",
            &args[0],
            value,
            &mapped,
        )?);
    }
    Ok(Value::Array(mapped))
}

pub(crate) fn filter(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("filter", args, 1)?;
    let values = array_values(receiver, runtime.heap.as_deref(), "method filter")?;
    let mut filtered = Vec::new();
    for value in values {
        let predicate = call_unary_callback(
            &mut runtime,
            "method filter",
            &args[0],
            value.clone(),
            &filtered,
        )?;
        if is_truthy(&predicate) {
            filtered.push(value);
        }
    }
    Ok(Value::Array(filtered))
}

pub(crate) fn find(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("find", args, 1)?;
    let values = array_values(receiver, runtime.heap.as_deref(), "method find")?;
    for value in values {
        let predicate =
            call_unary_callback(&mut runtime, "method find", &args[0], value.clone(), &[])?;
        if is_truthy(&predicate) {
            return Ok(option_value("Some", Some(value)));
        }
    }
    Ok(option_value("None", None))
}

pub(crate) fn any(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<bool> {
    expect_arity("any", args, 1)?;
    let values = array_values(receiver, runtime.heap.as_deref(), "method any")?;
    for value in values {
        let predicate = call_unary_callback(&mut runtime, "method any", &args[0], value, &[])?;
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
    let values = array_values(receiver, runtime.heap.as_deref(), "method all")?;
    for value in values {
        let predicate = call_unary_callback(&mut runtime, "method all", &args[0], value, &[])?;
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
    let values = array_values(receiver, runtime.heap.as_deref(), "method count")?;
    let mut count = 0_i64;
    for value in values {
        let predicate = call_unary_callback(&mut runtime, "method count", &args[0], value, &[])?;
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
