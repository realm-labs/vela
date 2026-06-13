use crate::heap::HeapValue;
use crate::heap_values::{allocate_heap_value, stored_runtime_value};
use crate::option_result::option_value;
use crate::{
    ExecutionBudget, HeapExecution, Value, VmError, VmErrorKind, VmResult, runtime_checks,
};

use super::{IteratorState, allocate_iterator};

pub(crate) fn is_iterator(receiver: &Value, heap: Option<&HeapExecution<'_>>) -> bool {
    match receiver {
        Value::HeapRef(reference) => matches!(
            heap.and_then(|heap| heap.heap.get(*reference)),
            Some(HeapValue::Iterator(_))
        ),
        _ => false,
    }
}

pub(crate) fn iter_method(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    runtime_checks::expect_arity("iter", args, 0)?;
    let iterator = match receiver {
        Value::Range(range) => IteratorState::from_range_cursor(range.cursor()),
        Value::HeapRef(reference) => {
            match heap.as_deref().and_then(|heap| heap.heap.get(*reference)) {
                Some(HeapValue::Array(values) | HeapValue::Set(values)) => {
                    IteratorState::from_values(values.iter().map(stored_runtime_value).collect())
                }
                Some(HeapValue::Map(values)) => {
                    IteratorState::from_values(values.values().map(stored_runtime_value).collect())
                }
                _ => return type_error("method iter"),
            }
        }
        _ => return type_error("method iter"),
    };
    allocate_iterator(iterator, heap, budget, "method iter")
}

pub(crate) fn chars_method(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    runtime_checks::expect_arity("chars", args, 0)?;
    let values = string_value(receiver, heap.as_deref(), "method chars")?
        .chars()
        .map(Value::Char)
        .collect();
    allocate_iterator(
        IteratorState::from_values(values),
        heap,
        budget,
        "method chars",
    )
}

pub(crate) fn string_bytes_method(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    runtime_checks::expect_arity("bytes", args, 0)?;
    let values = string_value(receiver, heap.as_deref(), "method bytes")?
        .bytes()
        .map(Value::U8)
        .collect();
    allocate_iterator(
        IteratorState::from_values(values),
        heap,
        budget,
        "method bytes",
    )
}

pub(crate) fn next_method(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    runtime_checks::expect_arity("next", args, 0)?;
    let next = with_iterator_mut(
        receiver,
        heap,
        "method next",
        |iterator| Ok(iterator.next()),
    )?;
    let Some(heap_ref) = heap.as_deref_mut() else {
        return type_error("method next");
    };
    option_value(next, heap_ref, budget.as_deref_mut())
}

pub(crate) fn count_method(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
) -> VmResult<Value> {
    runtime_checks::expect_arity("count", args, 0)?;
    let count = with_iterator_mut(receiver, heap, "method count", |iterator| {
        let mut count = 0_i64;
        while iterator.next().is_some() {
            count = count.checked_add(1).ok_or_else(|| {
                VmError::new(VmErrorKind::TypeMismatch {
                    operation: "method count",
                })
            })?;
        }
        Ok(count)
    })?;
    Ok(Value::i64(count))
}

pub(crate) fn collect_array_method(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    runtime_checks::expect_arity("collect_array", args, 0)?;
    let values = with_iterator_mut(receiver, heap, "method collect_array", |iterator| {
        let mut values = Vec::new();
        while let Some(value) = iterator.next() {
            values.push(value);
        }
        Ok(values)
    })?;
    let Some(heap_ref) = heap.as_deref_mut() else {
        return type_error("method collect_array");
    };
    allocate_heap_value(HeapValue::Array(values), heap_ref, budget.as_deref_mut())
}

fn with_iterator_mut<T>(
    receiver: &Value,
    heap: &mut Option<&mut HeapExecution<'_>>,
    operation: &'static str,
    f: impl FnOnce(&mut IteratorState) -> VmResult<T>,
) -> VmResult<T> {
    match receiver {
        Value::HeapRef(reference) => {
            let Some(HeapValue::Iterator(iterator)) = heap
                .as_deref_mut()
                .and_then(|heap| heap.heap.get_mut(*reference).ok())
            else {
                return type_error(operation);
            };
            f(iterator)
        }
        _ => type_error(operation),
    }
}

fn string_value<'a>(
    value: &'a Value,
    heap: Option<&'a HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<&'a str> {
    match value {
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::String(value)) => Ok(value),
            _ => type_error(operation),
        },
        _ => type_error(operation),
    }
}

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}
