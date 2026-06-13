use crate::heap::HeapValue;
use crate::{
    ExecutionBudget, HeapExecution, StandardMethodInlineCacheTarget, Value, VmError, VmErrorKind,
    VmResult, allocate_heap_value,
};

pub(in crate::standard_method_cache) fn call_cached_set_materialization(
    receiver: &Value,
    target: StandardMethodInlineCacheTarget,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> Option<VmResult<Value>> {
    match target {
        StandardMethodInlineCacheTarget::Union
        | StandardMethodInlineCacheTarget::Intersection
        | StandardMethodInlineCacheTarget::Difference
        | StandardMethodInlineCacheTarget::SymmetricDifference => {
            let (method, operation, kind) = match target {
                StandardMethodInlineCacheTarget::Union => {
                    ("union", "method union", CachedSetCombination::Union)
                }
                StandardMethodInlineCacheTarget::Intersection => (
                    "intersection",
                    "method intersection",
                    CachedSetCombination::Intersection,
                ),
                StandardMethodInlineCacheTarget::Difference => (
                    "difference",
                    "method difference",
                    CachedSetCombination::Difference,
                ),
                StandardMethodInlineCacheTarget::SymmetricDifference => (
                    "symmetric_difference",
                    "method symmetric_difference",
                    CachedSetCombination::SymmetricDifference,
                ),
                _ => unreachable!("set combination target was validated above"),
            };
            let payload = {
                let values = set_values(receiver, heap.as_deref())?;
                match set_combination_payload(
                    values,
                    args,
                    heap.as_deref(),
                    method,
                    operation,
                    kind,
                ) {
                    Ok(payload) => payload,
                    Err(error) => return Some(Err(error)),
                }
            };
            Some(make_set(payload, heap, budget, operation))
        }
        _ => None,
    }
}

fn set_values<'a>(receiver: &Value, heap: Option<&'a HeapExecution<'_>>) -> Option<&'a [Value]> {
    let Value::HeapRef(reference) = receiver else {
        return None;
    };
    let Some(HeapValue::Set(values)) = heap.and_then(|heap| heap.heap.get(*reference)) else {
        return None;
    };
    Some(values)
}

fn set_combination_payload(
    values: &[Value],
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
    method: &str,
    operation: &'static str,
    kind: CachedSetCombination,
) -> VmResult<Vec<Value>> {
    crate::runtime_checks::expect_arity(method, args, 1)?;
    let other = set_values(&args[0], heap)
        .ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation }))?;
    match kind {
        CachedSetCombination::Union => set_union_payload(values, other, heap, operation),
        CachedSetCombination::Intersection => {
            set_intersection_payload(values, other, heap, operation)
        }
        CachedSetCombination::Difference => set_difference_payload(values, other, heap, operation),
        CachedSetCombination::SymmetricDifference => {
            set_symmetric_difference_payload(values, other, heap, operation)
        }
    }
}

fn set_union_payload(
    left: &[Value],
    right: &[Value],
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<Vec<Value>> {
    let mut combined = Vec::new();
    let mut combined_keys = Vec::new();
    for value in left {
        push_unique(&mut combined, &mut combined_keys, *value, heap, operation)?;
    }
    for value in right {
        push_unique(&mut combined, &mut combined_keys, *value, heap, operation)?;
    }
    Ok(combined)
}

fn set_intersection_payload(
    left: &[Value],
    right: &[Value],
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<Vec<Value>> {
    let right_keys = set_keys(right, heap, operation)?;
    let mut result = Vec::new();
    let mut result_keys = Vec::new();
    for value in left {
        let key = SetKey::from_value(value, heap, operation)?;
        if right_keys.contains(&key) {
            push_unique_with_key(&mut result, &mut result_keys, *value, key);
        }
    }
    Ok(result)
}

fn set_difference_payload(
    left: &[Value],
    right: &[Value],
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<Vec<Value>> {
    let right_keys = set_keys(right, heap, operation)?;
    let mut result = Vec::new();
    let mut result_keys = Vec::new();
    for value in left {
        let key = SetKey::from_value(value, heap, operation)?;
        if !right_keys.contains(&key) {
            push_unique_with_key(&mut result, &mut result_keys, *value, key);
        }
    }
    Ok(result)
}

fn set_symmetric_difference_payload(
    left: &[Value],
    right: &[Value],
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<Vec<Value>> {
    let left_keys = set_keys(left, heap, operation)?;
    let right_keys = set_keys(right, heap, operation)?;
    let mut result = Vec::new();
    let mut result_keys = Vec::new();
    for (value, key) in left.iter().zip(left_keys.iter()) {
        if !right_keys.contains(key) {
            push_unique_with_key(&mut result, &mut result_keys, *value, key.clone());
        }
    }
    for (value, key) in right.iter().zip(right_keys.iter()) {
        if !left_keys.contains(key) {
            push_unique_with_key(&mut result, &mut result_keys, *value, key.clone());
        }
    }
    Ok(result)
}

fn push_unique(
    values: &mut Vec<Value>,
    keys: &mut Vec<SetKey>,
    value: Value,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<()> {
    let key = SetKey::from_value(&value, heap, operation)?;
    push_unique_with_key(values, keys, value, key);
    Ok(())
}

fn push_unique_with_key(
    values: &mut Vec<Value>,
    keys: &mut Vec<SetKey>,
    value: Value,
    key: SetKey,
) {
    if keys.contains(&key) {
        return;
    }
    keys.push(key);
    values.push(value);
}

fn set_keys(
    values: &[Value],
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<Vec<SetKey>> {
    values
        .iter()
        .map(|value| SetKey::from_value(value, heap, operation))
        .collect()
}

fn make_set(
    value: Vec<Value>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<Value> {
    let Some(heap) = heap.as_deref_mut() else {
        return Err(VmError::new(VmErrorKind::TypeMismatch { operation }));
    };
    allocate_heap_value(HeapValue::Set(value), heap, budget.as_deref_mut())
}

#[derive(Clone, Copy)]
enum CachedSetCombination {
    Union,
    Intersection,
    Difference,
    SymmetricDifference,
}

#[derive(Clone, Debug, PartialEq)]
enum SetKey {
    Null,
    Bool(bool),
    Int(i64),
    Float(u64),
    String(String),
}

impl SetKey {
    fn from_value(
        value: &Value,
        heap: Option<&HeapExecution<'_>>,
        operation: &'static str,
    ) -> VmResult<Self> {
        match value {
            Value::Null => Ok(Self::Null),
            Value::Bool(value) => Ok(Self::Bool(*value)),
            Value::I64(value) => Ok(Self::Int(*value)),
            Value::F64(value) if value.is_finite() => Ok(Self::Float(value.to_bits())),
            Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
                Some(HeapValue::String(value)) => Ok(Self::String(value.clone())),
                _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
            },
            _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
        }
    }
}
