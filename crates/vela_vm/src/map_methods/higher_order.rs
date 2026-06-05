use std::collections::BTreeMap;

use crate::method_runtime::{MethodRuntime, call_callback, call_callback_with_protected_values};
use crate::option_result::option_value;
use crate::runtime_checks::expect_closure;
use crate::{Value, VmError, VmErrorKind, VmResult};

use super::{expect_arity, map_entry, materialize_map_entries, type_error};

pub(crate) fn map_values(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("map_values", args, 1)?;
    if runtime.heap.is_none()
        && let Value::Map(entries) = receiver
    {
        let mut mapped = BTreeMap::new();
        for (key, value) in entries {
            let value = call_map_callback(
                &mut runtime,
                "method map_values",
                &args[0],
                key.clone(),
                value.clone(),
                &[],
            )?;
            mapped.insert(key.clone(), value);
        }
        return Ok(Value::Map(mapped));
    }
    let entries = materialize_map_entries(receiver, runtime.heap.as_deref(), "method map_values")?;
    let mut mapped = BTreeMap::new();
    for (key, value) in entries {
        let value = if runtime.heap.is_some() {
            call_map_callback_with_protected_values(
                &mut runtime,
                "method map_values",
                &args[0],
                key.clone(),
                value,
                mapped.values(),
            )?
        } else {
            call_map_callback(
                &mut runtime,
                "method map_values",
                &args[0],
                key.clone(),
                value,
                &[],
            )?
        };
        mapped.insert(key, value);
    }
    Ok(Value::Map(mapped))
}

pub(crate) fn filter(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("filter", args, 1)?;
    if runtime.heap.is_none()
        && let Value::Map(entries) = receiver
    {
        let mut filtered = BTreeMap::new();
        for (key, value) in entries {
            let predicate = call_map_callback(
                &mut runtime,
                "method filter",
                &args[0],
                key.clone(),
                value.clone(),
                &[],
            )?;
            if is_truthy(&predicate) {
                filtered.insert(key.clone(), value.clone());
            }
        }
        return Ok(Value::Map(filtered));
    }
    let entries = materialize_map_entries(receiver, runtime.heap.as_deref(), "method filter")?;
    let mut filtered = BTreeMap::new();
    for (key, value) in entries {
        let predicate = if runtime.heap.is_some() {
            call_map_callback_with_protected_values(
                &mut runtime,
                "method filter",
                &args[0],
                key.clone(),
                value.clone(),
                filtered.values(),
            )?
        } else {
            call_map_callback(
                &mut runtime,
                "method filter",
                &args[0],
                key.clone(),
                value.clone(),
                &[],
            )?
        };
        if is_truthy(&predicate) {
            filtered.insert(key, value);
        }
    }
    Ok(Value::Map(filtered))
}

pub(crate) fn find(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("find", args, 1)?;
    if runtime.heap.is_none()
        && let Value::Map(entries) = receiver
    {
        for (key, value) in entries {
            let predicate = call_map_callback(
                &mut runtime,
                "method find",
                &args[0],
                key.clone(),
                value.clone(),
                &[],
            )?;
            if is_truthy(&predicate) {
                return Ok(option_value(Some(map_entry(key, value.clone()))));
            }
        }
        return Ok(option_value(None));
    }
    let entries = materialize_map_entries(receiver, runtime.heap.as_deref(), "method find")?;
    for (key, value) in entries {
        let predicate = call_map_callback(
            &mut runtime,
            "method find",
            &args[0],
            key.clone(),
            value.clone(),
            &[],
        )?;
        if is_truthy(&predicate) {
            return Ok(option_value(Some(map_entry(&key, value))));
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
    if runtime.heap.is_none()
        && let Value::Map(entries) = receiver
    {
        for (key, value) in entries {
            let predicate = call_map_callback(
                &mut runtime,
                "method any",
                &args[0],
                key.clone(),
                value.clone(),
                &[],
            )?;
            if is_truthy(&predicate) {
                return Ok(true);
            }
        }
        return Ok(false);
    }
    let entries = materialize_map_entries(receiver, runtime.heap.as_deref(), "method any")?;
    for (key, value) in entries {
        let predicate = call_map_callback(&mut runtime, "method any", &args[0], key, value, &[])?;
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
    if runtime.heap.is_none()
        && let Value::Map(entries) = receiver
    {
        for (key, value) in entries {
            let predicate = call_map_callback(
                &mut runtime,
                "method all",
                &args[0],
                key.clone(),
                value.clone(),
                &[],
            )?;
            if !is_truthy(&predicate) {
                return Ok(false);
            }
        }
        return Ok(true);
    }
    let entries = materialize_map_entries(receiver, runtime.heap.as_deref(), "method all")?;
    for (key, value) in entries {
        let predicate = call_map_callback(&mut runtime, "method all", &args[0], key, value, &[])?;
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
    if runtime.heap.is_none()
        && let Value::Map(entries) = receiver
    {
        let mut count = 0_i64;
        for (key, value) in entries {
            let predicate = call_map_callback(
                &mut runtime,
                "method count",
                &args[0],
                key.clone(),
                value.clone(),
                &[],
            )?;
            if is_truthy(&predicate) {
                count = checked_count_increment(count)?;
            }
        }
        return Ok(count);
    }
    let entries = materialize_map_entries(receiver, runtime.heap.as_deref(), "method count")?;
    let mut count = 0_i64;
    for (key, value) in entries {
        let predicate = call_map_callback(&mut runtime, "method count", &args[0], key, value, &[])?;
        if is_truthy(&predicate) {
            count = checked_count_increment(count)?;
        }
    }
    Ok(count)
}

fn checked_count_increment(count: i64) -> VmResult<i64> {
    count.checked_add(1).ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "method count",
        })
    })
}

fn call_map_callback(
    runtime: &mut MethodRuntime<'_, '_, '_>,
    operation: &'static str,
    callback: &Value,
    key: String,
    value: Value,
    protected_values: &[Value],
) -> VmResult<Value> {
    let param_len = expect_closure(callback, runtime.heap.as_deref(), operation)?
        .code
        .params
        .len();
    match param_len {
        0 => call_callback(runtime, operation, callback, &[], protected_values),
        1 => call_callback(
            runtime,
            operation,
            callback,
            std::slice::from_ref(&value),
            protected_values,
        ),
        _ => {
            let callback_args = [Value::String(key), value];
            call_callback(
                runtime,
                operation,
                callback,
                &callback_args,
                protected_values,
            )
        }
    }
}

fn call_map_callback_with_protected_values<'value>(
    runtime: &mut MethodRuntime<'_, '_, '_>,
    operation: &'static str,
    callback: &Value,
    key: String,
    value: Value,
    protected_values: impl IntoIterator<Item = &'value Value>,
) -> VmResult<Value> {
    let param_len = expect_closure(callback, runtime.heap.as_deref(), operation)?
        .code
        .params
        .len();
    match param_len {
        0 => {
            call_callback_with_protected_values(runtime, operation, callback, &[], protected_values)
        }
        1 => call_callback_with_protected_values(
            runtime,
            operation,
            callback,
            std::slice::from_ref(&value),
            protected_values,
        ),
        _ => {
            let callback_args = [Value::String(key), value];
            call_callback_with_protected_values(
                runtime,
                operation,
                callback,
                &callback_args,
                protected_values,
            )
        }
    }
}

fn is_truthy(value: &Value) -> bool {
    !matches!(value, Value::Missing | Value::Null | Value::Bool(false))
}
