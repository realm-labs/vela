use std::collections::BTreeMap;

use crate::heap::HeapValue;
use crate::script_object::ScriptFields;
use crate::{
    ExecutionBudget, HeapExecution, StandardMethodInlineCacheTarget, Value, VmError, VmErrorKind,
    VmResult, allocate_heap_value,
};

pub(in crate::standard_method_cache) fn call_cached_map_materialization(
    receiver: &Value,
    target: StandardMethodInlineCacheTarget,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> Option<VmResult<Value>> {
    match target {
        StandardMethodInlineCacheTarget::Keys => {
            let payload = {
                let values = map_values(receiver, heap.as_deref())?;
                match map_keys_payload(values, args) {
                    Ok(payload) => payload,
                    Err(error) => return Some(Err(error)),
                }
            };
            Some(super::make_string_array(
                payload,
                heap,
                budget,
                "method keys",
            ))
        }
        StandardMethodInlineCacheTarget::Values => {
            let payload = {
                let values = map_values(receiver, heap.as_deref())?;
                match map_values_payload(values, args) {
                    Ok(payload) => payload,
                    Err(error) => return Some(Err(error)),
                }
            };
            Some(super::make_array(payload, heap, budget, "method values"))
        }
        StandardMethodInlineCacheTarget::Entries => {
            let payload = {
                let values = map_values(receiver, heap.as_deref())?;
                match map_entries_payload(values, args) {
                    Ok(payload) => payload,
                    Err(error) => return Some(Err(error)),
                }
            };
            Some(make_map_entry_array(
                payload,
                heap,
                budget,
                "method entries",
            ))
        }
        StandardMethodInlineCacheTarget::Merge => {
            let payload = {
                let values = map_values(receiver, heap.as_deref())?;
                match map_merge_payload(values, args, heap.as_deref()) {
                    Ok(payload) => payload,
                    Err(error) => return Some(Err(error)),
                }
            };
            Some(make_map(payload, heap, budget, "method merge"))
        }
        _ => None,
    }
}

pub(super) fn map_values<'a>(
    receiver: &Value,
    heap: Option<&'a HeapExecution<'_>>,
) -> Option<&'a BTreeMap<String, Value>> {
    let Value::HeapRef(reference) = receiver else {
        return None;
    };
    let Some(HeapValue::Map(values)) = heap.and_then(|heap| heap.heap.get(*reference)) else {
        return None;
    };
    Some(values)
}

fn map_keys_payload(values: &BTreeMap<String, Value>, args: &[Value]) -> VmResult<Vec<String>> {
    crate::runtime_checks::expect_arity("keys", args, 0)?;
    Ok(values.keys().cloned().collect())
}

fn map_values_payload(values: &BTreeMap<String, Value>, args: &[Value]) -> VmResult<Vec<Value>> {
    crate::runtime_checks::expect_arity("values", args, 0)?;
    Ok(values.values().copied().collect())
}

fn map_entries_payload(
    values: &BTreeMap<String, Value>,
    args: &[Value],
) -> VmResult<Vec<(String, Value)>> {
    crate::runtime_checks::expect_arity("entries", args, 0)?;
    Ok(values
        .iter()
        .map(|(key, value)| (key.clone(), *value))
        .collect())
}

fn map_merge_payload(
    values: &BTreeMap<String, Value>,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<BTreeMap<String, Value>> {
    crate::runtime_checks::expect_arity("merge", args, 1)?;
    let other = map_values(&args[0], heap).ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "method merge",
        })
    })?;
    let mut merged = values.clone();
    merged.extend(other.iter().map(|(key, value)| (key.clone(), *value)));
    Ok(merged)
}

fn make_map_entry_array(
    values: Vec<(String, Value)>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<Value> {
    let values = values
        .into_iter()
        .map(|(key, value)| make_map_entry(key, value, heap, budget, operation))
        .collect::<VmResult<Vec<_>>>()?;
    super::make_array(values, heap, budget, operation)
}

fn make_map_entry(
    key: String,
    value: Value,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<Value> {
    let key = super::make_string(key, heap, budget, operation)?;
    let Some(heap) = heap.as_deref_mut() else {
        return Err(VmError::new(VmErrorKind::TypeMismatch { operation }));
    };
    allocate_heap_value(
        HeapValue::Record {
            type_name: "MapEntry".to_owned(),
            identity: None,
            fields: ScriptFields::two("MapEntry", "key", key, "value", value),
        },
        heap,
        budget.as_deref_mut(),
    )
}

fn make_map(
    value: BTreeMap<String, Value>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<Value> {
    let Some(heap) = heap.as_deref_mut() else {
        return Err(VmError::new(VmErrorKind::TypeMismatch { operation }));
    };
    allocate_heap_value(HeapValue::Map(value), heap, budget.as_deref_mut())
}
