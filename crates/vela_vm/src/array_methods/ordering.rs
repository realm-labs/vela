use std::cmp::Ordering;

use crate::heap::HeapValue;
use crate::method_runtime::MethodRuntime;
use crate::{ExecutionBudget, HeapExecution, Value, VmResult, stored_runtime_value};

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

pub(crate) fn sort_by(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("sort_by", args, 1)?;
    let values = array_values(receiver, runtime.heap.as_deref(), "method sort_by")?;
    let mut entries = Vec::<SortEntry>::with_capacity(values.len());
    let mut key_kind = None;
    for value in values {
        let protected;
        let protected_values = if runtime.heap.is_some() {
            protected = entries.iter().map(|entry| entry.value).collect::<Vec<_>>();
            protected.as_slice()
        } else {
            &[]
        };
        let key_value = call_unary_callback(
            &mut runtime,
            "method sort_by",
            &args[0],
            value,
            protected_values,
        )?;
        push_sort_entry(
            &mut entries,
            &mut key_kind,
            value,
            key_value,
            runtime.heap.as_deref(),
            "method sort_by",
        )?;
    }
    let values = sort_entries(entries);
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

#[derive(Clone, Copy, Eq, PartialEq)]
enum SortKeyKind {
    Numeric,
    String,
}

enum SortKey {
    Int(i64),
    Float(f64),
    String(String),
}

impl SortKey {
    fn kind(&self) -> SortKeyKind {
        match self {
            Self::Int(_) | Self::Float(_) => SortKeyKind::Numeric,
            Self::String(_) => SortKeyKind::String,
        }
    }

    fn compare(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Int(left), Self::Int(right)) => left.cmp(right),
            (Self::Int(left), Self::Float(right)) => {
                (*left as f64).partial_cmp(right).unwrap_or(Ordering::Equal)
            }
            (Self::Float(left), Self::Int(right)) => left
                .partial_cmp(&(*right as f64))
                .unwrap_or(Ordering::Equal),
            (Self::Float(left), Self::Float(right)) => {
                left.partial_cmp(right).unwrap_or(Ordering::Equal)
            }
            (Self::String(left), Self::String(right)) => left.cmp(right),
            (Self::Int(_) | Self::Float(_), Self::String(_))
            | (Self::String(_), Self::Int(_) | Self::Float(_)) => Ordering::Equal,
        }
    }
}

fn sort_key(
    value: &Value,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<SortKey> {
    match value {
        Value::Scalar(vela_common::ScalarValue::I64(value)) => Ok(SortKey::Int(*value)),
        Value::Scalar(vela_common::ScalarValue::F64(value)) if value.is_finite() => {
            Ok(SortKey::Float(*value))
        }
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
        Value::Scalar(vela_common::ScalarValue::I64(value)) => Ok(SortKey::Int(*value)),
        Value::Scalar(vela_common::ScalarValue::F64(value)) if value.is_finite() => {
            Ok(SortKey::Float(*value))
        }
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::String(value)) => Ok(SortKey::String(value.clone())),
            _ => type_error(operation),
        },
        _ => type_error(operation),
    }
}
