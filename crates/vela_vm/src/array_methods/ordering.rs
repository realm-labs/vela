use std::cmp::Ordering;

use crate::heap::HeapValue;
use crate::iteration;
use crate::method_runtime::{MethodRuntime, call_callback_with_protected_values};
use crate::{
    EqualityRuntime, ExecutionBudget, HeapExecution, Value, VmResult, stored_runtime_value,
    values_total_cmp_with_traits,
};

use super::{
    array_values, call_unary_callback, expect_arity, make_array_value, option_value, type_error,
};

pub(crate) fn sort(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("sort", args, 0)?;
    if let Value::HeapRef(reference) = receiver {
        let Some(HeapValue::Array(values)) =
            heap.as_deref().and_then(|heap| heap.heap.get(*reference))
        else {
            return type_error("method sort");
        };
        let values = sort_runtime_values(values, heap.as_deref(), "method sort")?;
        return make_array_value(values, heap, budget, "method sort");
    }
    let values = array_values(receiver, heap.as_deref(), "method sort")?;
    let values = sort_values_by_key(values, heap.as_deref(), "method sort", |value, _| {
        Ok(*value)
    })?;
    make_array_value(values, heap, budget, "method sort")
}

pub(crate) fn sort_with_ordering(
    receiver: &Value,
    args: &[Value],
    runtime: &mut EqualityRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("sort", args, 0)?;
    let values = array_values(receiver, runtime.heap.as_deref(), "method sort")?;
    let sorted = match sort_values_by_key(
        values.clone(),
        runtime.heap.as_deref(),
        "method sort",
        |value, _| Ok(*value),
    ) {
        Ok(sorted) => sorted,
        Err(_) => sort_values_by_ord(values, runtime, "method sort")?,
    };
    make_array_value(
        sorted,
        &mut runtime.heap,
        &mut runtime.budget,
        "method sort",
    )
}

pub(crate) fn min(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("min", args, 0)?;
    extremum(receiver, heap, budget, "method min", Extremum::Min)
}

pub(crate) fn max(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("max", args, 0)?;
    extremum(receiver, heap, budget, "method max", Extremum::Max)
}

pub(crate) fn min_with_ordering(
    receiver: &Value,
    args: &[Value],
    runtime: &mut EqualityRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("min", args, 0)?;
    extremum_with_ordering(receiver, runtime, "method min", Extremum::Min)
}

pub(crate) fn max_with_ordering(
    receiver: &Value,
    args: &[Value],
    runtime: &mut EqualityRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("max", args, 0)?;
    extremum_with_ordering(receiver, runtime, "method max", Extremum::Max)
}

pub(crate) fn sort_by(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("sort_by", args, 1)?;
    let values = array_values(receiver, runtime.heap.as_deref(), "method sort_by")?;
    let mut key_entries = Vec::<SortEntry>::with_capacity(values.len());
    let mut ord_entries = Vec::<OrdSortEntry>::with_capacity(values.len());
    let mut key_kind = None;
    let mut can_key_sort = true;
    iteration::try_for_each_over(values, &mut runtime, "method sort_by", |runtime, value| {
        let key_value = if runtime.heap.is_some() {
            call_callback_with_protected_values(
                runtime,
                "method sort_by",
                &args[0],
                std::slice::from_ref(&value),
                ord_entries
                    .iter()
                    .flat_map(|entry| [&entry.value, &entry.key]),
            )?
        } else {
            call_unary_callback(runtime, "method sort_by", &args[0], value, &[])?
        };
        if can_key_sort {
            match sort_key(&key_value, runtime.heap.as_deref(), "method sort_by") {
                Ok(key) => {
                    if let Some(expected) = key_kind {
                        if key.kind() != expected {
                            can_key_sort = false;
                        }
                    } else {
                        key_kind = Some(key.kind());
                    }
                    if can_key_sort {
                        key_entries.push(SortEntry {
                            key,
                            value,
                            index: key_entries.len(),
                        });
                    }
                }
                Err(_) => {
                    can_key_sort = false;
                }
            }
        }
        ord_entries.push(OrdSortEntry {
            key: key_value,
            value,
        });
        Ok(())
    })?;
    let values = if can_key_sort {
        sort_entries(key_entries)
    } else {
        let mut equality_runtime = EqualityRuntime {
            vm: runtime.vm,
            program: runtime.program,
            linked_program: runtime.linked_program,
            host: runtime.host.as_deref_mut(),
            heap: runtime.heap.as_deref_mut(),
            budget: runtime.budget.as_deref_mut(),
            caller_roots: runtime.caller_roots,
            inline_caches: runtime.inline_caches,
            bytecode_profiler: runtime.bytecode_profiler,
        };
        sort_entries_by_ord(ord_entries, &mut equality_runtime, "method sort_by")?
    };
    make_array_value(
        values,
        &mut runtime.heap,
        &mut runtime.budget,
        "method sort_by",
    )
}

fn sort_values_by_key(
    values: Vec<Value>,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
    mut key_fn: impl FnMut(&Value, &[SortEntry]) -> VmResult<Value>,
) -> VmResult<Vec<Value>> {
    let mut entries = Vec::<SortEntry>::with_capacity(values.len());
    let mut key_kind = None;
    for value in values {
        let key_value = key_fn(&value, &entries)?;
        push_sort_entry(
            &mut entries,
            &mut key_kind,
            value,
            key_value,
            heap,
            operation,
        )?;
    }
    Ok(sort_entries(entries))
}

fn push_sort_entry(
    entries: &mut Vec<SortEntry>,
    key_kind: &mut Option<SortKeyKind>,
    value: Value,
    key_value: Value,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<()> {
    let key = sort_key(&key_value, heap, operation)?;
    if let Some(expected) = *key_kind {
        if key.kind() != expected {
            return type_error(operation);
        }
    } else {
        *key_kind = Some(key.kind());
    }
    entries.push(SortEntry {
        key,
        value,
        index: entries.len(),
    });
    Ok(())
}

fn sort_entries(mut entries: Vec<SortEntry>) -> Vec<Value> {
    entries.sort_by(|left, right| {
        left.key
            .compare(&right.key)
            .then_with(|| left.index.cmp(&right.index))
    });
    entries.into_iter().map(|entry| entry.value).collect()
}

fn sort_values_by_ord(
    values: Vec<Value>,
    runtime: &mut EqualityRuntime<'_, '_, '_>,
    operation: &'static str,
) -> VmResult<Vec<Value>> {
    let entries = values
        .into_iter()
        .map(|value| OrdSortEntry { key: value, value })
        .collect();
    sort_entries_by_ord(entries, runtime, operation)
}

fn sort_entries_by_ord(
    mut entries: Vec<OrdSortEntry>,
    runtime: &mut EqualityRuntime<'_, '_, '_>,
    operation: &'static str,
) -> VmResult<Vec<Value>> {
    let protected_root_len = runtime.heap.as_deref_mut().map(|heap| {
        let protected_root_len = runtime.caller_roots.push_to_heap(heap);
        for entry in &entries {
            heap.protect_values(&[entry.key, entry.value]);
        }
        protected_root_len
    });
    let result: VmResult<()> = (|| {
        for index in 1..entries.len() {
            let mut current = index;
            while current > 0 {
                let ordering = values_total_cmp_with_traits(
                    &entries[current].key,
                    &entries[current - 1].key,
                    runtime,
                    operation,
                )?;
                if ordering != Ordering::Less {
                    break;
                }
                entries.swap(current, current - 1);
                current -= 1;
            }
        }
        Ok(())
    })();
    if let (Some(heap), Some(protected_root_len)) =
        (runtime.heap.as_deref_mut(), protected_root_len)
    {
        heap.truncate_protected_roots(protected_root_len);
    }
    result?;
    Ok(entries.into_iter().map(|entry| entry.value).collect())
}

fn sort_runtime_values(
    values: &[Value],
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<Vec<Value>> {
    let mut entries = Vec::<RuntimeValueSortEntry>::with_capacity(values.len());
    let mut key_kind = None;
    for value in values {
        let key = sort_key_from_runtime_value(value, heap, operation)?;
        if let Some(expected) = key_kind {
            if key.kind() != expected {
                return type_error(operation);
            }
        } else {
            key_kind = Some(key.kind());
        }
        entries.push(RuntimeValueSortEntry {
            key,
            value: *value,
            index: entries.len(),
        });
    }
    entries.sort_by(|left, right| {
        left.key
            .compare(&right.key)
            .then_with(|| left.index.cmp(&right.index))
    });
    Ok(entries
        .into_iter()
        .map(|entry| stored_runtime_value(&entry.value))
        .collect())
}

#[derive(Clone, Copy)]
enum Extremum {
    Min,
    Max,
}

fn extremum(
    receiver: &Value,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
    extremum: Extremum,
) -> VmResult<Value> {
    match receiver {
        Value::HeapRef(reference) => {
            let Some(HeapValue::Array(values)) =
                heap.as_deref().and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error(operation);
            };
            let values = values.clone();
            let result = runtime_value_extremum(&values, heap.as_deref(), operation, extremum)?;
            match result {
                Some(value) => option_value("Some", Some(value), heap, budget),
                None => option_value("None", None, heap, budget),
            }
        }
        _ => type_error(operation),
    }
}

fn runtime_value_extremum(
    values: &[Value],
    read_heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
    extremum: Extremum,
) -> VmResult<Option<Value>> {
    let Some((first, rest)) = values.split_first() else {
        return Ok(None);
    };
    let mut best = first;
    let mut best_key = sort_key_from_runtime_value(first, read_heap, operation)?;
    let key_kind = best_key.kind();
    for value in rest {
        let key = sort_key_from_runtime_value(value, read_heap, operation)?;
        if key.kind() != key_kind {
            return type_error(operation);
        }
        let ordering = key.compare(&best_key);
        let replace = match extremum {
            Extremum::Min => ordering.is_lt(),
            Extremum::Max => ordering.is_gt(),
        };
        if replace {
            best = value;
            best_key = key;
        }
    }
    Ok(Some(stored_runtime_value(best)))
}

fn extremum_with_ordering(
    receiver: &Value,
    runtime: &mut EqualityRuntime<'_, '_, '_>,
    operation: &'static str,
    extremum: Extremum,
) -> VmResult<Value> {
    let values = array_values(receiver, runtime.heap.as_deref(), operation)?;
    let Some((first, rest)) = values.split_first() else {
        return option_value("None", None, &mut runtime.heap, &mut runtime.budget);
    };
    let mut best = *first;
    with_protected_values(&values, runtime, |runtime| {
        for value in rest {
            let ordering = values_total_cmp_with_traits(value, &best, runtime, operation)?;
            let replace = match extremum {
                Extremum::Min => ordering.is_lt(),
                Extremum::Max => ordering.is_gt(),
            };
            if replace {
                best = *value;
            }
        }
        Ok(())
    })?;
    option_value("Some", Some(best), &mut runtime.heap, &mut runtime.budget)
}

fn with_protected_values<T>(
    values: &[Value],
    runtime: &mut EqualityRuntime<'_, '_, '_>,
    action: impl FnOnce(&mut EqualityRuntime<'_, '_, '_>) -> VmResult<T>,
) -> VmResult<T> {
    let protected_root_len = runtime.heap.as_deref_mut().map(|heap| {
        let protected_root_len = runtime.caller_roots.push_to_heap(heap);
        heap.protect_values(values);
        protected_root_len
    });
    let result = action(runtime);
    if let (Some(heap), Some(protected_root_len)) =
        (runtime.heap.as_deref_mut(), protected_root_len)
    {
        heap.truncate_protected_roots(protected_root_len);
    }
    result
}

struct SortEntry {
    key: SortKey,
    value: Value,
    index: usize,
}

struct RuntimeValueSortEntry {
    key: SortKey,
    value: Value,
    index: usize,
}

struct OrdSortEntry {
    key: Value,
    value: Value,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum SortKeyKind {
    Numeric,
    String,
}

enum SortKey {
    Int(i64),
    String(String),
}

impl SortKey {
    fn kind(&self) -> SortKeyKind {
        match self {
            Self::Int(_) => SortKeyKind::Numeric,
            Self::String(_) => SortKeyKind::String,
        }
    }

    fn compare(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Int(left), Self::Int(right)) => left.cmp(right),
            (Self::String(left), Self::String(right)) => left.cmp(right),
            (Self::Int(_), Self::String(_)) | (Self::String(_), Self::Int(_)) => Ordering::Equal,
        }
    }
}

fn sort_key(
    value: &Value,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<SortKey> {
    match value {
        Value::I64(value) => Ok(SortKey::Int(*value)),
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::String(value)) => Ok(SortKey::String(value.clone())),
            _ => type_error(operation),
        },
        _ => type_error(operation),
    }
}

fn sort_key_from_runtime_value(
    value: &Value,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<SortKey> {
    match value {
        Value::I64(value) => Ok(SortKey::Int(*value)),
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::String(value)) => Ok(SortKey::String(value.clone())),
            _ => type_error(operation),
        },
        _ => type_error(operation),
    }
}
