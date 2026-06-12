use crate::heap::HeapValue;
use crate::option_result::option_value;
use crate::{
    ExecutionBudget, HeapExecution, StandardMethodInlineCacheTarget, Value, VmError, VmErrorKind,
    VmResult, stored_runtime_value,
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
