use crate::heap::HeapValue;
use crate::script_set::ScriptSet;
use crate::{
    ExecutionBudget, HeapExecution, StandardMethodInlineCacheTarget, Value, VmError, VmErrorKind,
    VmResult, allocate_heap_value, collection_mutation::check_collection_len, set_methods,
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
                    ("union", "method union", set_methods::SetCombination::Union)
                }
                StandardMethodInlineCacheTarget::Intersection => (
                    "intersection",
                    "method intersection",
                    set_methods::SetCombination::Intersection,
                ),
                StandardMethodInlineCacheTarget::Difference => (
                    "difference",
                    "method difference",
                    set_methods::SetCombination::Difference,
                ),
                StandardMethodInlineCacheTarget::SymmetricDifference => (
                    "symmetric_difference",
                    "method symmetric_difference",
                    set_methods::SetCombination::SymmetricDifference,
                ),
                _ => unreachable!("set combination target was validated above"),
            };
            if let Err(error) = crate::runtime_checks::expect_arity(method, args, 1) {
                return Some(Err(error));
            }
            let payload = {
                let values = set_values(receiver, heap.as_deref())?;
                let Some(other) = set_values(&args[0], heap.as_deref()) else {
                    return Some(Err(VmError::new(VmErrorKind::TypeMismatch { operation })));
                };
                match set_methods::combination_payload(
                    values,
                    other,
                    heap.as_deref(),
                    kind,
                    operation,
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

fn set_values<'a>(receiver: &Value, heap: Option<&'a HeapExecution<'_>>) -> Option<&'a ScriptSet> {
    let Value::HeapRef(reference) = receiver else {
        return None;
    };
    let Some(HeapValue::Set(values)) = heap.and_then(|heap| heap.heap.get(*reference)) else {
        return None;
    };
    Some(values)
}

fn make_set(
    value: Vec<Value>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<Value> {
    check_collection_len("set", 0, value.len(), budget.as_deref(), |budget| {
        budget.collection_limits().max_set_len
    })?;
    let Some(heap) = heap.as_deref_mut() else {
        return Err(VmError::new(VmErrorKind::TypeMismatch { operation }));
    };
    let value = ScriptSet::from_values(value, Some(&*heap), operation)?;
    allocate_heap_value(HeapValue::Set(value), heap, budget.as_deref_mut())
}
