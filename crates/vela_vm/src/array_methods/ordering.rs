use std::cmp::Ordering;

use crate::heap::HeapValue;
use crate::method_runtime::MethodRuntime;
use crate::{HeapExecution, Value, VmResult};

use super::{array_values, call_unary_callback, expect_arity, option_value, type_error};

pub(crate) fn sort(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("sort", args, 0)?;
    let values = array_values(receiver, heap, "method sort")?;
    sort_values_by_key(values, heap, "method sort", |value, _| Ok(value.clone()))
}

pub(crate) fn min(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("min", args, 0)?;
    extremum(receiver, heap, "method min", Extremum::Min)
}

pub(crate) fn max(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("max", args, 0)?;
    extremum(receiver, heap, "method max", Extremum::Max)
}

pub(crate) fn sort_by(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("sort_by", args, 1)?;
    if runtime.heap.is_none()
        && let Value::Array(values) = receiver
    {
        let mut entries = Vec::<SortEntry>::with_capacity(values.len());
        let mut key_kind = None;
        for value in values {
            let key_value =
                call_unary_callback(&mut runtime, "method sort_by", &args[0], value.clone(), &[])?;
            push_sort_entry(
                &mut entries,
                &mut key_kind,
                value.clone(),
                key_value,
                runtime.heap.as_deref(),
                "method sort_by",
            )?;
        }
        return sort_entries(entries);
    }
    let values = array_values(receiver, runtime.heap.as_deref(), "method sort_by")?;
    let mut entries = Vec::<SortEntry>::with_capacity(values.len());
    let mut key_kind = None;
    for value in values {
        let protected;
        let protected_values = if runtime.heap.is_some() {
            protected = entries
                .iter()
                .map(|entry| entry.value.clone())
                .collect::<Vec<_>>();
            protected.as_slice()
        } else {
            &[]
        };
        let key_value = call_unary_callback(
            &mut runtime,
            "method sort_by",
            &args[0],
            value.clone(),
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
    sort_entries(entries)
}

fn sort_values_by_key(
    values: Vec<Value>,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
    mut key_fn: impl FnMut(&Value, &[SortEntry]) -> VmResult<Value>,
) -> VmResult<Value> {
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
    sort_entries(entries)
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

fn sort_entries(mut entries: Vec<SortEntry>) -> VmResult<Value> {
    entries.sort_by(|left, right| {
        left.key
            .compare(&right.key)
            .then_with(|| left.index.cmp(&right.index))
    });
    Ok(Value::Array(
        entries.into_iter().map(|entry| entry.value).collect(),
    ))
}

#[derive(Clone, Copy)]
enum Extremum {
    Min,
    Max,
}

fn extremum(
    receiver: &Value,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
    extremum: Extremum,
) -> VmResult<Value> {
    let values = array_values(receiver, heap, operation)?;
    let Some((first, rest)) = values.split_first() else {
        return Ok(option_value("None", None));
    };
    let mut best = first.clone();
    let mut best_key = sort_key(first, heap, operation)?;
    let key_kind = best_key.kind();
    for value in rest {
        let key = sort_key(value, heap, operation)?;
        if key.kind() != key_kind {
            return type_error(operation);
        }
        let ordering = key.compare(&best_key);
        let replace = match extremum {
            Extremum::Min => ordering.is_lt(),
            Extremum::Max => ordering.is_gt(),
        };
        if replace {
            best = value.clone();
            best_key = key;
        }
    }
    Ok(option_value("Some", Some(best)))
}

struct SortEntry {
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
        Value::Int(value) => Ok(SortKey::Int(*value)),
        Value::Float(value) if value.is_finite() => Ok(SortKey::Float(*value)),
        Value::String(value) => Ok(SortKey::String(value.clone())),
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::String(value)) => Ok(SortKey::String(value.clone())),
            _ => type_error(operation),
        },
        _ => type_error(operation),
    }
}
