use crate::runtime_view::ArrayView;
use crate::{
    HeapExecution, Value, VmError, VmErrorKind, VmResult, value_from_heap_slot, values_equal,
};

use super::{expect_arity, option_value};

pub(crate) fn first(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("first", args, 0)?;
    first_value(receiver, heap)
}

pub(crate) fn last(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("last", args, 0)?;
    last_value(receiver, heap)
}

pub(crate) fn contains(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    expect_arity("contains", args, 1)?;
    array_contains(receiver, &args[0], heap)
}

pub(crate) fn index_of(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("index_of", args, 1)?;
    array_index_of(receiver, &args[0], heap)
}

fn first_value(receiver: &Value, heap: Option<&HeapExecution<'_>>) -> VmResult<Value> {
    let values = ArrayView::from_value(receiver, heap, "method first")?;
    Ok(values.first_owned().map_or_else(
        || option_value("None", None),
        |value| option_value("Some", Some(value)),
    ))
}

fn last_value(receiver: &Value, heap: Option<&HeapExecution<'_>>) -> VmResult<Value> {
    let values = ArrayView::from_value(receiver, heap, "method last")?;
    Ok(values.last_owned().map_or_else(
        || option_value("None", None),
        |value| option_value("Some", Some(value)),
    ))
}

fn array_contains(
    receiver: &Value,
    needle: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    match ArrayView::from_value(receiver, heap, "method contains")? {
        ArrayView::Values(values) => {
            for value in values {
                if values_equal(value, needle, heap)? {
                    return Ok(true);
                }
            }
        }
        ArrayView::Slots(values, _) => {
            for value in values {
                if values_equal(&value_from_heap_slot(value), needle, heap)? {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

fn array_index_of(
    receiver: &Value,
    needle: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    match ArrayView::from_value(receiver, heap, "method index_of")? {
        ArrayView::Values(values) => {
            for (index, value) in values.iter().enumerate() {
                if values_equal(value, needle, heap)? {
                    return index_option(index);
                }
            }
        }
        ArrayView::Slots(values, _) => {
            for (index, value) in values.iter().enumerate() {
                if values_equal(&value_from_heap_slot(value), needle, heap)? {
                    return index_option(index);
                }
            }
        }
    }
    Ok(option_value("None", None))
}

fn index_option(index: usize) -> VmResult<Value> {
    let index = i64::try_from(index).map_err(|_| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "method index_of",
        })
    })?;
    Ok(option_value("Some", Some(Value::Int(index))))
}
