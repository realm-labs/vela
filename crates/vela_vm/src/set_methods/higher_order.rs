use crate::method_runtime::{MethodRuntime, call_callback};
use crate::option_result::option_value;
use crate::{Value, VmError, VmErrorKind, VmResult};

use super::{expect_arity, push_unique, set_values};

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
    Ok(Value::Set(mapped))
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
    Ok(Value::Set(filtered))
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
            return Ok(option_value(Some(value)));
        }
    }
    Ok(option_value(None))
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
