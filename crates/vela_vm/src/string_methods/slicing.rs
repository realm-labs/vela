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
    let char_len = value.chars().count();
    if start > end {
        return type_error("method slice range");
    }
    if start > char_len {
        return Err(index_out_of_bounds(start, char_len));
    }
    if end > char_len {
        return Err(index_out_of_bounds(end, char_len));
    }

    let start_byte = char_byte_index(value, start);
    let end_byte = char_byte_index(value, end);
    let value = value[start_byte..end_byte].to_owned();
    make_string(value, heap, budget, "method slice")
}

fn char_byte_index(value: &str, index: usize) -> usize {
    if index == 0 {
        return 0;
    }
    value
        .char_indices()
        .nth(index)
        .map_or(value.len(), |(byte, _)| byte)
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
