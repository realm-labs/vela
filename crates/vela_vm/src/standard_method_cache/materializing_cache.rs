use crate::heap::HeapValue;
use crate::option_result::option_value;
use crate::{
    ExecutionBudget, HeapExecution, StandardMethodInlineCacheTarget, Value, VmError, VmErrorKind,
    VmResult, allocate_heap_value, stored_runtime_value,
};
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

pub(super) fn call_cached_map_get_option(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> Option<VmResult<Value>> {
    let values = map_values(receiver, heap.as_deref())?;
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

fn map_values<'a>(
    receiver: &Value,
    heap: Option<&'a HeapExecution<'_>>,
) -> Option<&'a std::collections::BTreeMap<String, Value>> {
    let Value::HeapRef(reference) = receiver else {
        return None;
    };
    let Some(HeapValue::Map(values)) = heap.and_then(|heap| heap.heap.get(*reference)) else {
        return None;
    };
    Some(values)
}

fn index_value(index: usize) -> VmResult<Value> {
    let index = i64::try_from(index).map_err(|_| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "method index_of",
        })
    })?;
    Ok(Value::Scalar(ScalarValue::I64(index)))
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

fn char_index_value(value: &Value) -> VmResult<usize> {
    match value {
        Value::Scalar(ScalarValue::I64(value)) if *value >= 0 => Ok(*value as usize),
        _ => Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "method char_at",
        })),
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
