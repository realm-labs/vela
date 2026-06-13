use crate::collection_mutation::check_collection_len;
use crate::heap::HeapValue;
use crate::heap_values::allocate_heap_value;
use crate::method_runtime::MethodRuntime;
use crate::option_result::option_value;
use crate::runtime_checks::is_truthy;
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
                Some(HeapValue::Array(values)) => {
                    IteratorState::from_array_source(*reference, values.len())
                }
                Some(HeapValue::Set(values)) => {
                    IteratorState::from_set_source(*reference, values.len())
                }
                Some(HeapValue::Map(values)) => IteratorState::from_map_values_source(
                    *reference,
                    values.keys().cloned().collect(),
                ),
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
    let Value::HeapRef(reference) = receiver else {
        return type_error("method chars");
    };
    if !matches!(
        heap.as_deref().and_then(|heap| heap.heap.get(*reference)),
        Some(HeapValue::String(_))
    ) {
        return type_error("method chars");
    }
    allocate_iterator(
        IteratorState::from_string_chars_source(*reference),
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
    let Value::HeapRef(reference) = receiver else {
        return type_error("method bytes");
    };
    if !matches!(
        heap.as_deref().and_then(|heap| heap.heap.get(*reference)),
        Some(HeapValue::String(_))
    ) {
        return type_error("method bytes");
    }
    allocate_iterator(
        IteratorState::from_string_bytes_source(*reference),
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

pub(crate) fn next_method_runtime(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    runtime_checks::expect_arity("next", args, 0)?;
    let next = with_taken_iterator(
        receiver,
        &mut runtime,
        "method next",
        |iterator, runtime| iterator.next_with_runtime(runtime, "method next", &[]),
    )?;
    let Some(heap_ref) = runtime.heap.as_deref_mut() else {
        return type_error("method next");
    };
    option_value(next, heap_ref, runtime.budget.as_deref_mut())
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

pub(crate) fn count_method_runtime(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    runtime_checks::expect_arity("count", args, 0)?;
    let count = with_taken_iterator(
        receiver,
        &mut runtime,
        "method count",
        |iterator, runtime| {
            let mut count = 0_i64;
            while iterator
                .next_with_runtime(runtime, "method count", &[])?
                .is_some()
            {
                count = count.checked_add(1).ok_or_else(|| {
                    VmError::new(VmErrorKind::TypeMismatch {
                        operation: "method count",
                    })
                })?;
            }
            Ok(count)
        },
    )?;
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
    check_collect_array_len(values.len(), budget.as_deref())?;
    let Some(heap_ref) = heap.as_deref_mut() else {
        return type_error("method collect_array");
    };
    allocate_heap_value(HeapValue::Array(values), heap_ref, budget.as_deref_mut())
}

pub(crate) fn collect_array_method_runtime(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    runtime_checks::expect_arity("collect_array", args, 0)?;
    let values = with_taken_iterator(
        receiver,
        &mut runtime,
        "method collect_array",
        |iterator, runtime| collect_values(iterator, runtime, "method collect_array"),
    )?;
    check_collect_array_len(values.len(), runtime.budget.as_deref())?;
    let Some(heap_ref) = runtime.heap.as_deref_mut() else {
        return type_error("method collect_array");
    };
    allocate_heap_value(
        HeapValue::Array(values),
        heap_ref,
        runtime.budget.as_deref_mut(),
    )
}

pub(crate) fn map_method(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    runtime_checks::expect_arity("map", args, 1)?;
    let source = take_iterator(receiver, &mut runtime.heap, "method map")?;
    let iterator = IteratorState::map(source, args[0]);
    allocate_iterator(
        iterator,
        &mut runtime.heap,
        &mut runtime.budget,
        "method map",
    )
}

pub(crate) fn filter_method(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    runtime_checks::expect_arity("filter", args, 1)?;
    let source = take_iterator(receiver, &mut runtime.heap, "method filter")?;
    let iterator = IteratorState::filter(source, args[0]);
    allocate_iterator(
        iterator,
        &mut runtime.heap,
        &mut runtime.budget,
        "method filter",
    )
}

pub(crate) fn take_method(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    runtime_checks::expect_arity("take", args, 1)?;
    let count = count_arg(args[0], "method take")?;
    let source = take_iterator_from_heap(receiver, heap, "method take")?;
    allocate_iterator(
        IteratorState::take(source, count),
        heap,
        budget,
        "method take",
    )
}

pub(crate) fn skip_method(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    runtime_checks::expect_arity("skip", args, 1)?;
    let count = count_arg(args[0], "method skip")?;
    let source = take_iterator_from_heap(receiver, heap, "method skip")?;
    allocate_iterator(
        IteratorState::skip(source, count),
        heap,
        budget,
        "method skip",
    )
}

pub(crate) fn any_method(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    runtime_checks::expect_arity("any", args, 1)?;
    let result = with_taken_iterator(receiver, &mut runtime, "method any", |iterator, runtime| {
        callback_any(iterator, runtime, "method any", args[0])
    })?;
    Ok(Value::Bool(result))
}

pub(crate) fn all_method(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    runtime_checks::expect_arity("all", args, 1)?;
    let result = with_taken_iterator(receiver, &mut runtime, "method all", |iterator, runtime| {
        callback_all(iterator, runtime, "method all", args[0])
    })?;
    Ok(Value::Bool(result))
}

pub(crate) fn find_method(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    runtime_checks::expect_arity("find", args, 1)?;
    let found = with_taken_iterator(
        receiver,
        &mut runtime,
        "method find",
        |iterator, runtime| callback_find(iterator, runtime, "method find", args[0]),
    )?;
    let Some(heap_ref) = runtime.heap.as_deref_mut() else {
        return type_error("method find");
    };
    option_value(found, heap_ref, runtime.budget.as_deref_mut())
}

pub(crate) fn collect_values(
    iterator: &mut IteratorState,
    runtime: &mut MethodRuntime<'_, '_, '_>,
    operation: &'static str,
) -> VmResult<Vec<Value>> {
    let mut values = Vec::new();
    while let Some(value) = iterator.next_with_runtime(runtime, operation, &values)? {
        values.push(value);
    }
    Ok(values)
}

pub(crate) fn collect_values_over<T>(
    items: impl IntoIterator<Item = T>,
    runtime: &mut MethodRuntime<'_, '_, '_>,
    _operation: &'static str,
    mut callback: impl FnMut(&mut MethodRuntime<'_, '_, '_>, T, &[Value]) -> VmResult<Value>,
) -> VmResult<Vec<Value>> {
    let mut values = Vec::new();
    for item in items {
        let value = callback(runtime, item, &values)?;
        values.push(value);
    }
    Ok(values)
}

pub(crate) fn filter_items_over<T>(
    items: impl IntoIterator<Item = T>,
    runtime: &mut MethodRuntime<'_, '_, '_>,
    _operation: &'static str,
    mut predicate: impl FnMut(&mut MethodRuntime<'_, '_, '_>, &T, &[Value]) -> VmResult<Value>,
    mut protected_value: impl FnMut(&T) -> Value,
) -> VmResult<Vec<T>> {
    let mut kept = Vec::new();
    let mut protected_values = Vec::new();
    for item in items {
        if is_truthy(&predicate(runtime, &item, &protected_values)?) {
            protected_values.push(protected_value(&item));
            kept.push(item);
        }
    }
    Ok(kept)
}

pub(crate) fn try_for_each_over<T>(
    items: impl IntoIterator<Item = T>,
    runtime: &mut MethodRuntime<'_, '_, '_>,
    _operation: &'static str,
    mut f: impl FnMut(&mut MethodRuntime<'_, '_, '_>, T) -> VmResult<()>,
) -> VmResult<()> {
    for item in items {
        f(runtime, item)?;
    }
    Ok(())
}

pub(crate) fn callback_any(
    iterator: &mut IteratorState,
    runtime: &mut MethodRuntime<'_, '_, '_>,
    operation: &'static str,
    callback: Value,
) -> VmResult<bool> {
    while let Some(value) = iterator.next_with_runtime(runtime, operation, &[])? {
        let protected = iterator.protected_values();
        if is_truthy(&crate::method_runtime::call_callback(
            runtime,
            operation,
            &callback,
            &[value],
            &protected,
        )?) {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn callback_all(
    iterator: &mut IteratorState,
    runtime: &mut MethodRuntime<'_, '_, '_>,
    operation: &'static str,
    callback: Value,
) -> VmResult<bool> {
    while let Some(value) = iterator.next_with_runtime(runtime, operation, &[])? {
        let protected = iterator.protected_values();
        if !is_truthy(&crate::method_runtime::call_callback(
            runtime,
            operation,
            &callback,
            &[value],
            &protected,
        )?) {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(crate) fn callback_find(
    iterator: &mut IteratorState,
    runtime: &mut MethodRuntime<'_, '_, '_>,
    operation: &'static str,
    callback: Value,
) -> VmResult<Option<Value>> {
    while let Some(value) = iterator.next_with_runtime(runtime, operation, &[])? {
        let protected = iterator.protected_values();
        if is_truthy(&crate::method_runtime::call_callback(
            runtime,
            operation,
            &callback,
            &[value],
            &protected,
        )?) {
            return Ok(Some(value));
        }
    }
    Ok(None)
}

pub(crate) fn callback_count(
    iterator: &mut IteratorState,
    runtime: &mut MethodRuntime<'_, '_, '_>,
    operation: &'static str,
    callback: Value,
) -> VmResult<i64> {
    let mut count = 0_i64;
    while let Some(value) = iterator.next_with_runtime(runtime, operation, &[])? {
        let protected = iterator.protected_values();
        if is_truthy(&crate::method_runtime::call_callback(
            runtime,
            operation,
            &callback,
            &[value],
            &protected,
        )?) {
            count = count
                .checked_add(1)
                .ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation }))?;
        }
    }
    Ok(count)
}

pub(crate) fn callback_any_over<T>(
    items: impl IntoIterator<Item = T>,
    runtime: &mut MethodRuntime<'_, '_, '_>,
    _operation: &'static str,
    mut callback: impl FnMut(&mut MethodRuntime<'_, '_, '_>, &T) -> VmResult<Value>,
) -> VmResult<bool> {
    for item in items {
        if is_truthy(&callback(runtime, &item)?) {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn callback_all_over<T>(
    items: impl IntoIterator<Item = T>,
    runtime: &mut MethodRuntime<'_, '_, '_>,
    _operation: &'static str,
    mut callback: impl FnMut(&mut MethodRuntime<'_, '_, '_>, &T) -> VmResult<Value>,
) -> VmResult<bool> {
    for item in items {
        if !is_truthy(&callback(runtime, &item)?) {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(crate) fn callback_find_over<T>(
    items: impl IntoIterator<Item = T>,
    runtime: &mut MethodRuntime<'_, '_, '_>,
    _operation: &'static str,
    mut callback: impl FnMut(&mut MethodRuntime<'_, '_, '_>, &T) -> VmResult<Value>,
) -> VmResult<Option<T>> {
    for item in items {
        if is_truthy(&callback(runtime, &item)?) {
            return Ok(Some(item));
        }
    }
    Ok(None)
}

pub(crate) fn callback_count_over<T>(
    items: impl IntoIterator<Item = T>,
    runtime: &mut MethodRuntime<'_, '_, '_>,
    operation: &'static str,
    mut callback: impl FnMut(&mut MethodRuntime<'_, '_, '_>, &T) -> VmResult<Value>,
) -> VmResult<i64> {
    let mut count = 0_i64;
    for item in items {
        if is_truthy(&callback(runtime, &item)?) {
            count = count
                .checked_add(1)
                .ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation }))?;
        }
    }
    Ok(count)
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

pub(crate) fn take_iterator_from_heap(
    receiver: &Value,
    heap: &mut Option<&mut HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<IteratorState> {
    match receiver {
        Value::HeapRef(reference) => {
            let Some(HeapValue::Iterator(iterator)) = heap
                .as_deref_mut()
                .and_then(|heap| heap.heap.get_mut(*reference).ok())
            else {
                return type_error(operation);
            };
            Ok(std::mem::replace(iterator, IteratorState::empty()))
        }
        _ => type_error(operation),
    }
}

pub(crate) fn restore_iterator_to_heap(
    receiver: Value,
    heap: &mut Option<&mut HeapExecution<'_>>,
    iterator: IteratorState,
    operation: &'static str,
) -> VmResult<()> {
    match receiver {
        Value::HeapRef(reference) => {
            let Some(HeapValue::Iterator(slot)) = heap
                .as_deref_mut()
                .and_then(|heap| heap.heap.get_mut(reference).ok())
            else {
                return type_error(operation);
            };
            *slot = iterator;
            Ok(())
        }
        _ => type_error(operation),
    }
}

fn take_iterator(
    receiver: &Value,
    heap: &mut Option<&mut HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<IteratorState> {
    take_iterator_from_heap(receiver, heap, operation)
}

fn with_taken_iterator<T>(
    receiver: &Value,
    runtime: &mut MethodRuntime<'_, '_, '_>,
    operation: &'static str,
    f: impl FnOnce(&mut IteratorState, &mut MethodRuntime<'_, '_, '_>) -> VmResult<T>,
) -> VmResult<T> {
    let mut iterator = take_iterator(receiver, &mut runtime.heap, operation)?;
    let result = f(&mut iterator, runtime);
    restore_iterator_to_heap(*receiver, &mut runtime.heap, iterator, operation)?;
    result
}

fn count_arg(value: Value, operation: &'static str) -> VmResult<usize> {
    match value {
        Value::I64(value) if value >= 0 => Ok(value as usize),
        _ => type_error(operation),
    }
}

fn check_collect_array_len(len: usize, budget: Option<&ExecutionBudget>) -> VmResult<()> {
    check_collection_len("array", 0, len, budget, |budget| {
        budget.collection_limits().max_array_len
    })
}

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}
