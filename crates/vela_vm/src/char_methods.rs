use crate::{
    ExecutionBudget, HeapExecution, Value, VmError, VmErrorKind, VmResult, allocate_heap_value,
    expect_arity,
};

pub(crate) fn is_char(value: &Value) -> bool {
    matches!(value, Value::Char(_))
}

pub(crate) fn to_string(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("to_string", args, 0)?;
    let ch = char_value(receiver, "method to_string")?;
    let Some(heap) = heap.as_deref_mut() else {
        return type_error("method to_string");
    };
    allocate_heap_value(
        crate::heap::HeapValue::String(ch.to_string()),
        heap,
        budget.as_deref_mut(),
    )
}

pub(crate) fn is_whitespace(receiver: &Value, args: &[Value]) -> VmResult<Value> {
    expect_arity("is_whitespace", args, 0)?;
    Ok(Value::Bool(
        char_value(receiver, "method is_whitespace")?.is_whitespace(),
    ))
}

pub(crate) fn is_ascii(receiver: &Value, args: &[Value]) -> VmResult<Value> {
    expect_arity("is_ascii", args, 0)?;
    Ok(Value::Bool(
        char_value(receiver, "method is_ascii")?.is_ascii(),
    ))
}

pub(crate) fn is_ascii_digit(receiver: &Value, args: &[Value]) -> VmResult<Value> {
    expect_arity("is_ascii_digit", args, 0)?;
    Ok(Value::Bool(
        char_value(receiver, "method is_ascii_digit")?.is_ascii_digit(),
    ))
}

fn char_value(receiver: &Value, operation: &'static str) -> VmResult<char> {
    match receiver {
        Value::Char(value) => Ok(*value),
        _ => type_error(operation),
    }
}

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}
