use crate::{ExecutionBudget, HeapExecution, Value, VmError, VmErrorKind, VmResult};

use super::{expect_arity, index_value, make_string, string_value};

pub(crate) fn slice(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("slice", args, 2)?;
    let value = string_value(receiver, heap.as_deref(), "method slice")?;
    let start = index_value(&args[0], "method slice")?;
    let end = index_value(&args[1], "method slice")?;
    if start > end {
        return type_error("method slice range");
    }
    if start > value.len() {
        return Err(index_out_of_bounds(start, value.len()));
    }
    if end > value.len() {
        return Err(index_out_of_bounds(end, value.len()));
    }
    if !value.is_char_boundary(start) || !value.is_char_boundary(end) {
        return type_error("method slice boundary");
    }

    let value = value[start..end].to_owned();
    make_string(value, heap, budget, "method slice")
}

fn index_out_of_bounds(index: usize, len: usize) -> VmError {
    VmError::new(VmErrorKind::IndexOutOfBounds {
        index: i64::try_from(index).unwrap_or(i64::MAX),
        len,
    })
}

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}
