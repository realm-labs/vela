use crate::option_result::option_value;
use crate::script_set::ScriptSet;
use crate::{ExecutionBudget, HeapExecution, Value, VmError, VmErrorKind, VmResult};

pub(super) fn option(
    payload: Option<Value>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<Value> {
    let Some(heap) = heap.as_deref_mut() else {
        return Err(VmError::new(VmErrorKind::TypeMismatch { operation }));
    };
    option_value(payload, heap, budget.as_deref_mut())
}

pub(super) fn set(
    values: Vec<Value>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<Value> {
    let Some(heap) = heap.as_deref_mut() else {
        return Err(VmError::new(VmErrorKind::TypeMismatch { operation }));
    };
    let values = ScriptSet::from_values(values, Some(&*heap), operation)?;
    crate::collection_mutation::check_collection_len(
        "set",
        0,
        values.len(),
        budget.as_deref(),
        |budget| budget.collection_limits().max_set_len,
    )?;
    crate::heap_values::allocate_heap_value(
        crate::heap::HeapValue::Set(values),
        heap,
        budget.as_deref_mut(),
    )
}
