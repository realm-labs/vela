mod map;
mod option_result;
mod set;

use crate::heap::HeapValue;
use crate::option_result::option_value;
use crate::{
    ExecutionBudget, HeapExecution, StandardMethodInlineCacheTarget, Value, VmError, VmErrorKind,
    VmResult, allocate_heap_value, stored_runtime_value,
};
pub(super) use map::call_cached_map_materialization;
pub(super) use option_result::call_cached_option_result_materialization;
pub(super) use set::call_cached_set_materialization;
use vela_common::ScalarValue;

pub(super) fn call_cached_array_lookup_option(
    receiver: &Value,
    target: StandardMethodInlineCacheTarget,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> Option<VmResult<Value>> {
    let (method, operation) = match target {
        StandardMethodInlineCacheTarget::First => ("first", "method first"),
        StandardMethodInlineCacheTarget::Last => ("last", "method last"),
        StandardMethodInlineCacheTarget::IndexOf => ("index_of", "method index_of"),
        _ => return None,
    };
    let slots = array_slots(receiver, heap.as_deref(), operation)?;
    let payload =
        match target {
            StandardMethodInlineCacheTarget::First => {
                match crate::runtime_checks::expect_arity(method, args, 0) {
                    Ok(()) => {}
                    Err(error) => return Some(Err(error)),
                }
                slots.first().map(stored_runtime_value)
            }
            StandardMethodInlineCacheTarget::Last => {
                match crate::runtime_checks::expect_arity(method, args, 0) {
                    Ok(()) => {}
                    Err(error) => return Some(Err(error)),
                }
                slots.last().map(stored_runtime_value)
            }
            StandardMethodInlineCacheTarget::IndexOf => {
                match crate::runtime_checks::expect_arity(method, args, 1) {
                    Ok(()) => {}
                    Err(error) => return Some(Err(error)),
                }
                let index = match slots.iter().enumerate().find_map(|(index, value)| {
                    match crate::values_equal(
                        &stored_runtime_value(value),
                        &args[0],
                        heap.as_deref(),
                    ) {
                        Ok(true) => Some(Ok(index)),
                        Ok(false) => None,
                        Err(error) => Some(Err(error)),
                    }
                }) {
                    Some(Ok(index)) => Some(index),
                    Some(Err(error)) => return Some(Err(error)),
                    None => None,
                };
                match index.map(index_value).transpose() {
                    Ok(payload) => payload,
                    Err(error) => return Some(Err(error)),
                }
            }
            _ => return None,
        };
    Some(make_option(payload, heap, budget))
}

pub(super) fn call_cached_array_materialization(
    receiver: &Value,
    target: StandardMethodInlineCacheTarget,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> Option<VmResult<Value>> {
    match target {
        StandardMethodInlineCacheTarget::Slice => {
            let payload = {
                let slots = array_slots(receiver, heap.as_deref(), "method slice")?;
                match array_slice_payload(slots, args) {
                    Ok(payload) => payload,
                    Err(error) => return Some(Err(error)),
                }
            };
            Some(make_array(payload, heap, budget, "method slice"))
        }
        StandardMethodInlineCacheTarget::Reverse => {
            let payload = {
                let slots = array_slots(receiver, heap.as_deref(), "method reverse")?;
                match array_reverse_payload(slots, args) {
                    Ok(payload) => payload,
                    Err(error) => return Some(Err(error)),
                }
            };
            Some(make_array(payload, heap, budget, "method reverse"))
        }
        StandardMethodInlineCacheTarget::Distinct => {
            let payload = {
                let heap_ref = heap.as_deref();
                let slots = array_slots(receiver, heap_ref, "method distinct")?;
                match array_distinct_payload(slots, args, heap_ref) {
                    Ok(payload) => payload,
                    Err(error) => return Some(Err(error)),
                }
            };
            Some(make_array(payload, heap, budget, "method distinct"))
        }
        StandardMethodInlineCacheTarget::Join => {
            let payload = {
                let heap_ref = heap.as_deref();
                let slots = array_slots(receiver, heap_ref, "method join")?;
                match array_join_payload(slots, args, heap_ref) {
                    Ok(payload) => payload,
                    Err(error) => return Some(Err(error)),
                }
            };
            Some(make_string(payload, heap, budget, "method join"))
        }
        StandardMethodInlineCacheTarget::Sort => {
            let payload = {
                let heap_ref = heap.as_deref();
                let slots = array_slots(receiver, heap_ref, "method sort")?;
                match array_sort_payload(slots, args, heap_ref) {
                    Ok(payload) => payload,
                    Err(error) => return Some(Err(error)),
                }
            };
            Some(make_array(payload, heap, budget, "method sort"))
        }
        StandardMethodInlineCacheTarget::Min | StandardMethodInlineCacheTarget::Max => {
            let (method, operation, extremum) = match target {
                StandardMethodInlineCacheTarget::Min => {
                    ("min", "method min", CachedArrayExtremum::Min)
                }
                StandardMethodInlineCacheTarget::Max => {
                    ("max", "method max", CachedArrayExtremum::Max)
                }
                _ => unreachable!("array extrema target was validated above"),
            };
            let payload = {
                let heap_ref = heap.as_deref();
                let slots = array_slots(receiver, heap_ref, operation)?;
                match array_extremum_payload(slots, args, heap_ref, method, operation, extremum) {
                    Ok(payload) => payload,
                    Err(error) => return Some(Err(error)),
                }
            };
            Some(make_option(payload, heap, budget))
        }
        _ => None,
    }
}

pub(super) fn call_cached_map_get_option(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> Option<VmResult<Value>> {
    let values = map::map_values(receiver, heap.as_deref())?;
    let payload = match crate::runtime_checks::expect_arity("get", args, 1).and_then(|()| {
        let key = crate::string_methods::string_value(&args[0], heap.as_deref(), "map key")?;
        Ok(values.get(key).map(stored_runtime_value))
    }) {
        Ok(payload) => payload,
        Err(error) => return Some(Err(error)),
    };
    Some(make_option(payload, heap, budget))
}

pub(super) fn call_cached_string_parse_option(
    receiver: &Value,
    target: StandardMethodInlineCacheTarget,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> Option<VmResult<Value>> {
    let (method, payload): (&str, fn(&str) -> Option<Value>) = match target {
        StandardMethodInlineCacheTarget::ParseInt => ("parse_int", parse_int_payload),
        StandardMethodInlineCacheTarget::ParseFloat => ("parse_float", parse_float_payload),
        StandardMethodInlineCacheTarget::ParseBool => ("parse_bool", parse_bool_payload),
        _ => return None,
    };
    let value = string_receiver(receiver, heap.as_deref())?;
    let payload = match crate::runtime_checks::expect_arity(method, args, 0) {
        Ok(()) => payload(value),
        Err(error) => return Some(Err(error)),
    };
    Some(make_option(payload, heap, budget))
}

pub(super) fn call_cached_string_option(
    receiver: &Value,
    target: StandardMethodInlineCacheTarget,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> Option<VmResult<Value>> {
    let value = string_receiver(receiver, heap.as_deref())?;
    match target {
        StandardMethodInlineCacheTarget::Find => {
            let payload =
                match crate::runtime_checks::expect_arity("find", args, 1).and_then(|()| {
                    let needle = crate::string_methods::string_value(
                        &args[0],
                        heap.as_deref(),
                        "method find",
                    )?;
                    Ok(value.find(needle).map(|byte_index| {
                        let char_index = value[..byte_index].chars().count();
                        Value::i64(i64::try_from(char_index).unwrap_or(i64::MAX))
                    }))
                }) {
                    Ok(payload) => payload,
                    Err(error) => return Some(Err(error)),
                };
            Some(make_option(payload, heap, budget))
        }
        StandardMethodInlineCacheTarget::CharAt => {
            let payload = match char_at_payload(value, args) {
                Ok(payload) => payload,
                Err(error) => return Some(Err(error)),
            };
            Some(make_string_option(payload, heap, budget, "method char_at"))
        }
        StandardMethodInlineCacheTarget::SplitOnce => {
            let payload = match split_once_payload(value, args, heap.as_deref()) {
                Ok(payload) => payload,
                Err(error) => return Some(Err(error)),
            };
            Some(make_string_array_option(
                payload,
                heap,
                budget,
                "method split_once",
            ))
        }
        StandardMethodInlineCacheTarget::StripPrefix => {
            let payload = match strip_affix_payload(
                value,
                args,
                heap.as_deref(),
                "strip_prefix",
                "method strip_prefix",
                AffixKind::Prefix,
            ) {
                Ok(payload) => payload,
                Err(error) => return Some(Err(error)),
            };
            Some(make_string_option(
                payload,
                heap,
                budget,
                "method strip_prefix",
            ))
        }
        StandardMethodInlineCacheTarget::StripSuffix => {
            let payload = match strip_affix_payload(
                value,
                args,
                heap.as_deref(),
                "strip_suffix",
                "method strip_suffix",
                AffixKind::Suffix,
            ) {
                Ok(payload) => payload,
                Err(error) => return Some(Err(error)),
            };
            Some(make_string_option(
                payload,
                heap,
                budget,
                "method strip_suffix",
            ))
        }
        _ => None,
    }
}

pub(super) fn call_cached_string_array(
    receiver: &Value,
    target: StandardMethodInlineCacheTarget,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> Option<VmResult<Value>> {
    let value = string_receiver(receiver, heap.as_deref())?;
    let payload = match target {
        StandardMethodInlineCacheTarget::Split => split_payload(value, args, heap.as_deref()),
        StandardMethodInlineCacheTarget::SplitLines => split_lines_payload(value, args),
        StandardMethodInlineCacheTarget::SplitWhitespace => split_whitespace_payload(value, args),
        _ => return None,
    };
    Some(
        payload
            .and_then(|payload| make_string_array(payload, heap, budget, split_operation(target))),
    )
}

pub(super) fn call_cached_bytes_materialization(
    receiver: &Value,
    target: StandardMethodInlineCacheTarget,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> Option<VmResult<Value>> {
    match target {
        StandardMethodInlineCacheTarget::Slice => {
            let payload = {
                let value = match crate::bytes_methods::bytes_value(
                    receiver,
                    heap.as_deref(),
                    "method slice",
                ) {
                    Ok(value) => value,
                    Err(error) => return Some(Err(error)),
                };
                match crate::bytes_methods::slice_payload(value, args) {
                    Ok(payload) => payload,
                    Err(error) => return Some(Err(error)),
                }
            };
            Some(make_bytes(payload, heap, budget, "method slice"))
        }
        StandardMethodInlineCacheTarget::ToHex => {
            let payload = {
                let value = match crate::bytes_methods::bytes_value(
                    receiver,
                    heap.as_deref(),
                    "method to_hex",
                ) {
                    Ok(value) => value,
                    Err(error) => return Some(Err(error)),
                };
                match crate::bytes_methods::to_hex_payload(value, args) {
                    Ok(payload) => payload,
                    Err(error) => return Some(Err(error)),
                }
            };
            Some(make_string(payload, heap, budget, "method to_hex"))
        }
        _ => None,
    }
}

pub(super) fn call_cached_string_transform(
    receiver: &Value,
    target: StandardMethodInlineCacheTarget,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> Option<VmResult<Value>> {
    let value = string_receiver(receiver, heap.as_deref())?;
    if target == StandardMethodInlineCacheTarget::Repeat {
        let payload = match repeat_payload(value, args) {
            Ok(payload) => payload,
            Err(error) => return Some(Err(error)),
        };
        return Some(make_string(payload, heap, budget, "method repeat"));
    }
    if target == StandardMethodInlineCacheTarget::Replace {
        let payload = match replace_payload(value, args, heap.as_deref()) {
            Ok(payload) => payload,
            Err(error) => return Some(Err(error)),
        };
        return Some(make_string(payload, heap, budget, "method replace"));
    }
    if target == StandardMethodInlineCacheTarget::Slice {
        let payload = match slice_payload(value, args) {
            Ok(payload) => payload,
            Err(error) => return Some(Err(error)),
        };
        return Some(make_string(payload, heap, budget, "method slice"));
    }
    let (method, operation, transform): (&str, &'static str, fn(&str) -> String) = match target {
        StandardMethodInlineCacheTarget::ToUpper => {
            ("to_upper", "method to_upper", str::to_uppercase)
        }
        StandardMethodInlineCacheTarget::ToLower => {
            ("to_lower", "method to_lower", str::to_lowercase)
        }
        StandardMethodInlineCacheTarget::Trim => ("trim", "method trim", trim_payload),
        StandardMethodInlineCacheTarget::TrimStart => {
            ("trim_start", "method trim_start", trim_start_payload)
        }
        StandardMethodInlineCacheTarget::TrimEnd => {
            ("trim_end", "method trim_end", trim_end_payload)
        }
        _ => return None,
    };
    let payload = match crate::runtime_checks::expect_arity(method, args, 0) {
        Ok(()) => transform(value),
        Err(error) => return Some(Err(error)),
    };
    Some(make_string(payload, heap, budget, operation))
}

fn array_slots<'a>(
    receiver: &Value,
    heap: Option<&'a HeapExecution<'_>>,
    _operation: &'static str,
) -> Option<&'a [Value]> {
    let Value::HeapRef(reference) = receiver else {
        return None;
    };
    let Some(HeapValue::Array(values)) = heap.and_then(|heap| heap.heap.get(*reference)) else {
        return None;
    };
    Some(values)
}

fn string_receiver<'a>(receiver: &Value, heap: Option<&'a HeapExecution<'_>>) -> Option<&'a str> {
    let Value::HeapRef(reference) = receiver else {
        return None;
    };
    let Some(HeapValue::String(value)) = heap.and_then(|heap| heap.heap.get(*reference)) else {
        return None;
    };
    Some(value)
}

fn index_value(index: usize) -> VmResult<Value> {
    let index = i64::try_from(index).map_err(|_| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "method index_of",
        })
    })?;
    Ok(Value::Scalar(ScalarValue::I64(index)))
}

fn array_slice_payload(values: &[Value], args: &[Value]) -> VmResult<Vec<Value>> {
    crate::runtime_checks::expect_arity("slice", args, 2)?;
    let start = array_index_value(&args[0], "method slice")?;
    let end = array_index_value(&args[1], "method slice")?;
    if start > end {
        return Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "method slice",
        }));
    }
    if start > values.len() {
        return Err(index_out_of_bounds(start, values.len()));
    }
    if end > values.len() {
        return Err(index_out_of_bounds(end, values.len()));
    }
    Ok(values[start..end]
        .iter()
        .map(stored_runtime_value)
        .collect())
}

fn array_reverse_payload(values: &[Value], args: &[Value]) -> VmResult<Vec<Value>> {
    crate::runtime_checks::expect_arity("reverse", args, 0)?;
    Ok(values.iter().rev().map(stored_runtime_value).collect())
}

fn array_distinct_payload(
    values: &[Value],
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Vec<Value>> {
    crate::runtime_checks::expect_arity("distinct", args, 0)?;
    let mut distinct = Vec::new();
    'values: for value in values {
        let value = stored_runtime_value(value);
        for existing in &distinct {
            if crate::values_equal(existing, &value, heap)? {
                continue 'values;
            }
        }
        distinct.push(value);
    }
    Ok(distinct)
}

fn array_join_payload(
    values: &[Value],
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<String> {
    crate::runtime_checks::expect_arity("join", args, 1)?;
    let separator = crate::string_methods::string_value(&args[0], heap, "method join")?;
    let mut capacity = separator
        .len()
        .saturating_mul(values.len().saturating_sub(1));
    for value in values {
        capacity = capacity.saturating_add(array_join_string(value, heap)?.len());
    }

    let mut joined = String::with_capacity(capacity);
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            joined.push_str(separator);
        }
        joined.push_str(array_join_string(value, heap)?);
    }
    Ok(joined)
}

fn array_join_string<'a>(
    value: &'a Value,
    heap: Option<&'a HeapExecution<'_>>,
) -> VmResult<&'a str> {
    match value {
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::String(value)) => Ok(value),
            _ => Err(VmError::new(VmErrorKind::TypeMismatch {
                operation: "method join",
            })),
        },
        _ => Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "method join",
        })),
    }
}

fn array_sort_payload(
    values: &[Value],
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Vec<Value>> {
    crate::runtime_checks::expect_arity("sort", args, 0)?;
    let mut entries = Vec::with_capacity(values.len());
    let mut key_kind = None;
    for value in values {
        let key = cached_sort_key(value, heap, "method sort")?;
        if let Some(expected) = key_kind {
            if key.kind() != expected {
                return Err(VmError::new(VmErrorKind::TypeMismatch {
                    operation: "method sort",
                }));
            }
        } else {
            key_kind = Some(key.kind());
        }
        entries.push(CachedSortEntry {
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

fn array_extremum_payload(
    values: &[Value],
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
    method: &str,
    operation: &'static str,
    extremum: CachedArrayExtremum,
) -> VmResult<Option<Value>> {
    crate::runtime_checks::expect_arity(method, args, 0)?;
    let Some((first, rest)) = values.split_first() else {
        return Ok(None);
    };
    let mut best = first;
    let mut best_key = cached_sort_key(first, heap, operation)?;
    let key_kind = best_key.kind();
    for value in rest {
        let key = cached_sort_key(value, heap, operation)?;
        if key.kind() != key_kind {
            return Err(VmError::new(VmErrorKind::TypeMismatch { operation }));
        }
        let ordering = key.compare(&best_key);
        let replace = match extremum {
            CachedArrayExtremum::Min => ordering.is_lt(),
            CachedArrayExtremum::Max => ordering.is_gt(),
        };
        if replace {
            best = value;
            best_key = key;
        }
    }
    Ok(Some(stored_runtime_value(best)))
}

#[derive(Clone, Copy)]
enum CachedArrayExtremum {
    Min,
    Max,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum CachedSortKeyKind {
    Numeric,
    String,
}

enum CachedSortKey {
    Int(i64),
    Float(f64),
    String(String),
}

struct CachedSortEntry {
    key: CachedSortKey,
    value: Value,
    index: usize,
}

impl CachedSortKey {
    fn kind(&self) -> CachedSortKeyKind {
        match self {
            Self::Int(_) | Self::Float(_) => CachedSortKeyKind::Numeric,
            Self::String(_) => CachedSortKeyKind::String,
        }
    }

    fn compare(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (Self::Int(left), Self::Int(right)) => left.cmp(right),
            (Self::Int(left), Self::Float(right)) => (*left as f64)
                .partial_cmp(right)
                .unwrap_or(std::cmp::Ordering::Equal),
            (Self::Float(left), Self::Int(right)) => left
                .partial_cmp(&(*right as f64))
                .unwrap_or(std::cmp::Ordering::Equal),
            (Self::Float(left), Self::Float(right)) => {
                left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal)
            }
            (Self::String(left), Self::String(right)) => left.cmp(right),
            (Self::Int(_) | Self::Float(_), Self::String(_))
            | (Self::String(_), Self::Int(_) | Self::Float(_)) => std::cmp::Ordering::Equal,
        }
    }
}

fn cached_sort_key(
    value: &Value,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<CachedSortKey> {
    match value {
        Value::Scalar(ScalarValue::I64(value)) => Ok(CachedSortKey::Int(*value)),
        Value::Scalar(ScalarValue::F64(value)) if value.is_finite() => {
            Ok(CachedSortKey::Float(*value))
        }
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::String(value)) => Ok(CachedSortKey::String(value.clone())),
            _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
        },
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
    }
}

fn array_index_value(value: &Value, operation: &'static str) -> VmResult<usize> {
    match value {
        Value::Scalar(ScalarValue::I64(value)) if *value >= 0 => Ok(*value as usize),
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
    }
}

fn parse_int_payload(value: &str) -> Option<Value> {
    value.parse::<i64>().ok().map(Value::i64)
}

fn parse_float_payload(value: &str) -> Option<Value> {
    value
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .map(Value::f64)
}

fn parse_bool_payload(value: &str) -> Option<Value> {
    match value {
        "true" => Some(Value::Bool(true)),
        "false" => Some(Value::Bool(false)),
        _ => None,
    }
}

fn strip_affix_payload(
    value: &str,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
    method: &str,
    operation: &'static str,
    affix_kind: AffixKind,
) -> VmResult<Option<String>> {
    crate::runtime_checks::expect_arity(method, args, 1)?;
    let affix = crate::string_methods::string_value(&args[0], heap, operation)?;
    let stripped = match affix_kind {
        AffixKind::Prefix => value.strip_prefix(affix),
        AffixKind::Suffix => value.strip_suffix(affix),
    };
    Ok(stripped.map(str::to_owned))
}

fn char_at_payload(value: &str, args: &[Value]) -> VmResult<Option<String>> {
    crate::runtime_checks::expect_arity("char_at", args, 1)?;
    let index = char_index_value(&args[0])?;
    Ok(value.chars().nth(index).map(|ch| ch.to_string()))
}

fn split_once_payload(
    value: &str,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Option<Vec<String>>> {
    crate::runtime_checks::expect_arity("split_once", args, 1)?;
    let separator = crate::string_methods::string_value(&args[0], heap, "method split_once")?;
    Ok(value
        .split_once(separator)
        .map(|(before, after)| vec![before.to_owned(), after.to_owned()]))
}

fn split_payload(
    value: &str,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Vec<String>> {
    crate::runtime_checks::expect_arity("split", args, 1)?;
    let separator = crate::string_methods::string_value(&args[0], heap, "method split")?;
    Ok(value.split(separator).map(str::to_owned).collect())
}

fn split_lines_payload(value: &str, args: &[Value]) -> VmResult<Vec<String>> {
    crate::runtime_checks::expect_arity("split_lines", args, 0)?;
    Ok(value.lines().map(str::to_owned).collect())
}

fn split_whitespace_payload(value: &str, args: &[Value]) -> VmResult<Vec<String>> {
    crate::runtime_checks::expect_arity("split_whitespace", args, 0)?;
    Ok(value.split_whitespace().map(str::to_owned).collect())
}

fn split_operation(target: StandardMethodInlineCacheTarget) -> &'static str {
    match target {
        StandardMethodInlineCacheTarget::Split => "method split",
        StandardMethodInlineCacheTarget::SplitLines => "method split_lines",
        StandardMethodInlineCacheTarget::SplitWhitespace => "method split_whitespace",
        _ => "method split",
    }
}

fn trim_payload(value: &str) -> String {
    value.trim().to_owned()
}

fn trim_start_payload(value: &str) -> String {
    value.trim_start().to_owned()
}

fn trim_end_payload(value: &str) -> String {
    value.trim_end().to_owned()
}

fn repeat_payload(value: &str, args: &[Value]) -> VmResult<String> {
    crate::runtime_checks::expect_arity("repeat", args, 1)?;
    let count = char_index_value_with_operation(&args[0], "method repeat")?;
    value.len().checked_mul(count).ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "method repeat",
        })
    })?;
    Ok(value.repeat(count))
}

fn replace_payload(
    value: &str,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<String> {
    crate::runtime_checks::expect_arity("replace", args, 2)?;
    let from = crate::string_methods::string_value(&args[0], heap, "method replace")?;
    let to = crate::string_methods::string_value(&args[1], heap, "method replace")?;
    Ok(value.replace(from, to))
}

fn slice_payload(value: &str, args: &[Value]) -> VmResult<String> {
    crate::runtime_checks::expect_arity("slice", args, 2)?;
    let start = char_index_value_with_operation(&args[0], "method slice")?;
    let end = char_index_value_with_operation(&args[1], "method slice")?;
    let char_len = value.chars().count();
    if start > end {
        return Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "method slice range",
        }));
    }
    if start > char_len {
        return Err(index_out_of_bounds(start, char_len));
    }
    if end > char_len {
        return Err(index_out_of_bounds(end, char_len));
    }

    let start_byte = char_byte_index(value, start);
    let end_byte = char_byte_index(value, end);
    Ok(value[start_byte..end_byte].to_owned())
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

fn char_index_value(value: &Value) -> VmResult<usize> {
    char_index_value_with_operation(value, "method char_at")
}

fn char_index_value_with_operation(value: &Value, operation: &'static str) -> VmResult<usize> {
    match value {
        Value::Scalar(ScalarValue::I64(value)) if *value >= 0 => Ok(*value as usize),
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
    }
}

#[derive(Clone, Copy)]
enum AffixKind {
    Prefix,
    Suffix,
}

fn make_string_option(
    payload: Option<String>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<Value> {
    let payload = payload
        .map(|value| make_string(value, heap, budget, operation))
        .transpose()?;
    make_option(payload, heap, budget)
}

fn make_string_array_option(
    payload: Option<Vec<String>>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<Value> {
    let payload = payload
        .map(|values| make_string_array(values, heap, budget, operation))
        .transpose()?;
    make_option(payload, heap, budget)
}

fn make_string_array(
    values: Vec<String>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<Value> {
    let values = values
        .into_iter()
        .map(|value| make_string(value, heap, budget, operation))
        .collect::<VmResult<Vec<_>>>()?;
    make_array(values, heap, budget, operation)
}

fn make_array(
    value: Vec<Value>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<Value> {
    let Some(heap) = heap.as_deref_mut() else {
        return Err(VmError::new(VmErrorKind::TypeMismatch { operation }));
    };
    allocate_heap_value(HeapValue::Array(value), heap, budget.as_deref_mut())
}

fn make_bytes(
    value: Vec<u8>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<Value> {
    let Some(heap) = heap.as_deref_mut() else {
        return Err(VmError::new(VmErrorKind::TypeMismatch { operation }));
    };
    allocate_heap_value(HeapValue::Bytes(value), heap, budget.as_deref_mut())
}

fn make_string(
    value: String,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<Value> {
    let Some(heap) = heap.as_deref_mut() else {
        return Err(VmError::new(VmErrorKind::TypeMismatch { operation }));
    };
    allocate_heap_value(HeapValue::String(value), heap, budget.as_deref_mut())
}

fn make_option(
    payload: Option<Value>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    let Some(heap) = heap.as_deref_mut() else {
        return Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "Option",
        }));
    };
    option_value(payload, heap, budget.as_deref_mut())
}
