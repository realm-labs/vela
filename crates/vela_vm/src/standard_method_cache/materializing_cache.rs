mod array_mutation;
mod map;
mod map_mutation;
mod option_result;
mod set;
mod set_mutation;

use crate::heap::HeapValue;
use crate::option_result::option_value;
use crate::{
    ExecutionBudget, HeapExecution, StandardMethodInlineCacheTarget, Value, VmError, VmErrorKind,
    VmResult, allocate_heap_value,
};
pub(super) use array_mutation::call_cached_array_mutation;
pub(super) use map::call_cached_map_materialization;
pub(super) use map_mutation::call_cached_map_mutation;
pub(super) use option_result::call_cached_option_result_materialization;
pub(super) use set::call_cached_set_materialization;
pub(super) use set_mutation::call_cached_set_mutation;

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
        _ => return None,
    };
    let slots = array_slots(receiver, heap.as_deref(), operation)?;
    let payload = match target {
        StandardMethodInlineCacheTarget::First => {
            match crate::runtime_checks::expect_arity(method, args, 0) {
                Ok(()) => {}
                Err(error) => return Some(Err(error)),
            }
            slots.first().copied()
        }
        StandardMethodInlineCacheTarget::Last => {
            match crate::runtime_checks::expect_arity(method, args, 0) {
                Ok(()) => {}
                Err(error) => return Some(Err(error)),
            }
            slots.last().copied()
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
    let payload = match crate::runtime_checks::expect_arity("get", args, 1)
        .and_then(|()| values.get(&args[0], heap.as_deref(), "method get"))
    {
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
        StandardMethodInlineCacheTarget::ParseI8 => ("parse_i8", parse_i8_payload),
        StandardMethodInlineCacheTarget::ParseI16 => ("parse_i16", parse_i16_payload),
        StandardMethodInlineCacheTarget::ParseI32 => ("parse_i32", parse_i32_payload),
        StandardMethodInlineCacheTarget::ParseI64 => ("parse_i64", parse_i64_payload),
        StandardMethodInlineCacheTarget::ParseU8 => ("parse_u8", parse_u8_payload),
        StandardMethodInlineCacheTarget::ParseU16 => ("parse_u16", parse_u16_payload),
        StandardMethodInlineCacheTarget::ParseU32 => ("parse_u32", parse_u32_payload),
        StandardMethodInlineCacheTarget::ParseU64 => ("parse_u64", parse_u64_payload),
        StandardMethodInlineCacheTarget::ParseF32 => ("parse_f32", parse_f32_payload),
        StandardMethodInlineCacheTarget::ParseF64 => ("parse_f64", parse_f64_payload),
        StandardMethodInlineCacheTarget::ParseBool => ("parse_bool", parse_bool_payload),
        StandardMethodInlineCacheTarget::ParseChar => ("parse_char", parse_char_payload),
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
                        Value::i64(i64::try_from(byte_index).unwrap_or(i64::MAX))
                    }))
                }) {
                    Ok(payload) => payload,
                    Err(error) => return Some(Err(error)),
                };
            Some(make_option(payload, heap, budget))
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
    Ok(values[start..end].to_vec())
}

fn array_reverse_payload(values: &[Value], args: &[Value]) -> VmResult<Vec<Value>> {
    crate::runtime_checks::expect_arity("reverse", args, 0)?;
    Ok(values.iter().rev().copied().collect())
}

fn array_join_payload(
    values: &[Value],
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<String> {
    crate::runtime_checks::expect_arity("join", args, 1)?;
    let separator = crate::string_methods::string_value(&args[0], heap, "method join")?;
    let parts = values
        .iter()
        .map(|value| array_join_string(value, heap))
        .collect::<VmResult<Vec<_>>>()?;
    let mut capacity = separator
        .len()
        .saturating_mul(values.len().saturating_sub(1));
    for part in &parts {
        capacity = capacity.saturating_add(part.len());
    }

    let mut joined = String::with_capacity(capacity);
    for (index, part) in parts.into_iter().enumerate() {
        if index > 0 {
            joined.push_str(separator);
        }
        joined.push_str(part);
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

fn array_index_value(value: &Value, operation: &'static str) -> VmResult<usize> {
    match value {
        Value::I64(value) if *value >= 0 => Ok(*value as usize),
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
    }
}

fn parse_i8_payload(value: &str) -> Option<Value> {
    value.parse::<i8>().ok().map(Value::I8)
}

fn parse_i16_payload(value: &str) -> Option<Value> {
    value.parse::<i16>().ok().map(Value::I16)
}

fn parse_i32_payload(value: &str) -> Option<Value> {
    value.parse::<i32>().ok().map(Value::I32)
}

fn parse_i64_payload(value: &str) -> Option<Value> {
    value.parse::<i64>().ok().map(Value::I64)
}

fn parse_u8_payload(value: &str) -> Option<Value> {
    value.parse::<u8>().ok().map(Value::U8)
}

fn parse_u16_payload(value: &str) -> Option<Value> {
    value.parse::<u16>().ok().map(Value::U16)
}

fn parse_u32_payload(value: &str) -> Option<Value> {
    value.parse::<u32>().ok().map(Value::U32)
}

fn parse_u64_payload(value: &str) -> Option<Value> {
    value.parse::<u64>().ok().map(Value::U64)
}

fn parse_f32_payload(value: &str) -> Option<Value> {
    value
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite())
        .map(Value::F32)
}

fn parse_f64_payload(value: &str) -> Option<Value> {
    value
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .map(Value::F64)
}

fn parse_bool_payload(value: &str) -> Option<Value> {
    match value {
        "true" => Some(Value::Bool(true)),
        "false" => Some(Value::Bool(false)),
        _ => None,
    }
}

fn parse_char_payload(value: &str) -> Option<Value> {
    let mut chars = value.chars();
    let first = chars.next()?;
    if chars.next().is_none() {
        Some(Value::Char(first))
    } else {
        None
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
    if start > end {
        return Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "method slice range",
        }));
    }
    if start > value.len() {
        return Err(index_out_of_bounds(start, value.len()));
    }
    if end > value.len() {
        return Err(index_out_of_bounds(end, value.len()));
    }
    if !value.is_char_boundary(start) || !value.is_char_boundary(end) {
        return Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "method slice boundary",
        }));
    }

    Ok(value[start..end].to_owned())
}

fn index_out_of_bounds(index: usize, len: usize) -> VmError {
    VmError::new(VmErrorKind::IndexOutOfBounds {
        index: i64::try_from(index).unwrap_or(i64::MAX),
        len,
    })
}

fn char_index_value_with_operation(value: &Value, operation: &'static str) -> VmResult<usize> {
    match value {
        Value::I64(value) if *value >= 0 => Ok(*value as usize),
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
