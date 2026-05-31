use crate::{HeapExecution, Value, VmError, VmErrorKind, VmResult, values_equal};

use super::{array_values, expect_arity, option_value};

pub(crate) fn first(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("first", args, 0)?;
    let values = array_values(receiver, heap, "method first")?;
    Ok(values.first().cloned().map_or_else(
        || option_value("None", None),
        |value| option_value("Some", Some(value)),
    ))
}

pub(crate) fn last(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("last", args, 0)?;
    let values = array_values(receiver, heap, "method last")?;
    Ok(values.last().cloned().map_or_else(
        || option_value("None", None),
        |value| option_value("Some", Some(value)),
    ))
}

pub(crate) fn contains(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    expect_arity("contains", args, 1)?;
    let values = array_values(receiver, heap, "method contains")?;
    for value in values {
        if values_equal(&value, &args[0], heap)? {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn index_of(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("index_of", args, 1)?;
    let values = array_values(receiver, heap, "method index_of")?;
    for (index, value) in values.into_iter().enumerate() {
        if values_equal(&value, &args[0], heap)? {
            let index = i64::try_from(index).map_err(|_| {
                VmError::new(VmErrorKind::TypeMismatch {
                    operation: "method index_of",
                })
            })?;
            return Ok(option_value("Some", Some(Value::Int(index))));
        }
    }
    Ok(option_value("None", None))
}
