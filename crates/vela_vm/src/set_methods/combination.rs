use crate::{HeapExecution, Value, VmResult};

use super::{SetKey, expect_arity, push_unique, set_keys, set_values};

pub(crate) fn union(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("union", args, 1)?;
    let mut combined = Vec::new();
    for value in set_values(receiver, heap, "method union")? {
        push_unique(&mut combined, value, heap, "method union")?;
    }
    for value in set_values(&args[0], heap, "method union")? {
        push_unique(&mut combined, value, heap, "method union")?;
    }
    Ok(Value::Set(combined))
}

pub(crate) fn intersection(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("intersection", args, 1)?;
    let right = set_keys(
        &set_values(&args[0], heap, "method intersection")?,
        heap,
        "method intersection",
    )?;
    let mut result = Vec::new();
    for value in set_values(receiver, heap, "method intersection")? {
        let key = SetKey::from_value(&value, heap, "method intersection")?;
        if right.contains(&key) {
            push_unique(&mut result, value, heap, "method intersection")?;
        }
    }
    Ok(Value::Set(result))
}

pub(crate) fn difference(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("difference", args, 1)?;
    let right = set_keys(
        &set_values(&args[0], heap, "method difference")?,
        heap,
        "method difference",
    )?;
    let mut result = Vec::new();
    for value in set_values(receiver, heap, "method difference")? {
        let key = SetKey::from_value(&value, heap, "method difference")?;
        if !right.contains(&key) {
            push_unique(&mut result, value, heap, "method difference")?;
        }
    }
    Ok(Value::Set(result))
}

pub(crate) fn symmetric_difference(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("symmetric_difference", args, 1)?;
    let left_values = set_values(receiver, heap, "method symmetric_difference")?;
    let right_values = set_values(&args[0], heap, "method symmetric_difference")?;
    let left_keys = set_keys(&left_values, heap, "method symmetric_difference")?;
    let right_keys = set_keys(&right_values, heap, "method symmetric_difference")?;

    let mut result = Vec::new();
    for (value, key) in left_values.into_iter().zip(left_keys.iter()) {
        if !right_keys.contains(key) {
            push_unique(&mut result, value, heap, "method symmetric_difference")?;
        }
    }
    for (value, key) in right_values.into_iter().zip(right_keys.iter()) {
        if !left_keys.contains(key) {
            push_unique(&mut result, value, heap, "method symmetric_difference")?;
        }
    }
    Ok(Value::Set(result))
}

pub(crate) fn is_subset(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    expect_arity("is_subset", args, 1)?;
    set_contains_all(receiver, &args[0], heap, "method is_subset")
}

pub(crate) fn is_superset(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    expect_arity("is_superset", args, 1)?;
    set_contains_all(&args[0], receiver, heap, "method is_superset")
}

pub(crate) fn is_disjoint(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    expect_arity("is_disjoint", args, 1)?;
    let right = set_keys(
        &set_values(&args[0], heap, "method is_disjoint")?,
        heap,
        "method is_disjoint",
    )?;
    for value in set_values(receiver, heap, "method is_disjoint")? {
        let key = SetKey::from_value(&value, heap, "method is_disjoint")?;
        if right.contains(&key) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn set_contains_all(
    subset: &Value,
    superset: &Value,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<bool> {
    let superset = set_keys(&set_values(superset, heap, operation)?, heap, operation)?;
    for value in set_values(subset, heap, operation)? {
        let key = SetKey::from_value(&value, heap, operation)?;
        if !superset.contains(&key) {
            return Ok(false);
        }
    }
    Ok(true)
}
