mod aggregation;
mod higher_order;
mod lookup;
mod mutation;
mod ordering;
mod transform;

pub(crate) use aggregation::{group_by, sum, sum_values};
pub(crate) use higher_order::{all, any, count, filter, find, map};
pub(crate) use lookup::{contains_with_equality, first, index_of_with_equality, last};
pub(crate) use mutation::{clear, extend, insert, pop, push, remove_at};
pub(crate) use ordering::{max_with_ordering, min_with_ordering, sort_by, sort_with_ordering};
pub(crate) use transform::{distinct_with_equality, join, reverse, slice};

use crate::collection_mutation::check_collection_len;
use crate::heap::HeapValue;
use crate::method_runtime::{MethodRuntime, call_callback};
use crate::option_result::{StdEnumVariant, std_enum_identity};
use crate::script_map::ScriptMap;
use crate::script_object::ScriptFields;
use crate::{
    ExecutionBudget, HeapExecution, Value, VmError, VmErrorKind, VmResult, allocate_heap_value,
    stored_runtime_value,
};

pub(super) fn string_value<'a>(
    value: &'a Value,
    heap: Option<&'a HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<&'a str> {
    match value {
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::String(value)) => Ok(value),
            _ => type_error(operation),
        },
        _ => type_error(operation),
    }
}

pub(crate) fn is_array(receiver: &Value, heap: Option<&HeapExecution<'_>>) -> bool {
    match receiver {
        Value::HeapRef(reference) => {
            matches!(
                heap.and_then(|heap| heap.heap.get(*reference)),
                Some(HeapValue::Array(_))
            )
        }
        _ => false,
    }
}

pub(super) fn array_values(
    receiver: &Value,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<Vec<Value>> {
    match receiver {
        Value::HeapRef(reference) => {
            let Some(HeapValue::Array(values)) = heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error(operation);
            };
            Ok(values.iter().map(stored_runtime_value).collect())
        }
        _ => type_error(operation),
    }
}

pub(super) fn index_value(value: &Value, operation: &'static str) -> VmResult<usize> {
    match value {
        Value::I64(value) if *value >= 0 => Ok(*value as usize),
        _ => type_error(operation),
    }
}

pub(super) fn call_unary_callback(
    runtime: &mut MethodRuntime<'_, '_, '_>,
    operation: &'static str,
    callback: &Value,
    value: Value,
    protected_values: &[Value],
) -> VmResult<Value> {
    call_callback(runtime, operation, callback, &[value], protected_values)
}

pub(super) fn expect_arity(name: &str, args: &[Value], expected: usize) -> VmResult<()> {
    if args.len() == expected {
        return Ok(());
    }
    Err(VmError::new(VmErrorKind::ArityMismatch {
        name: name.to_owned(),
        expected,
        actual: args.len(),
    }))
}

pub(super) fn option_value(
    variant: &str,
    payload: Option<Value>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    let Some(heap) = heap.as_deref_mut() else {
        return type_error("Option");
    };
    let fields = match (variant, payload) {
        ("Some", Some(payload)) => ScriptFields::single("Option.Some", "0", payload),
        ("None", None) => ScriptFields::empty("Option.None"),
        _ => return type_error("Option"),
    };
    let variant_id = match variant {
        "Some" => StdEnumVariant::Some,
        "None" => StdEnumVariant::None,
        _ => return type_error("Option"),
    };
    allocate_heap_value(
        HeapValue::Enum {
            enum_name: "Option".to_owned(),
            variant: variant.to_owned(),
            identity: Some(std_enum_identity(variant_id)),
            fields,
        },
        heap,
        budget.as_deref_mut(),
    )
}

pub(crate) fn make_string_value(
    value: String,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<Value> {
    let Some(heap) = heap.as_deref_mut() else {
        return type_error(operation);
    };
    allocate_heap_value(HeapValue::String(value), heap, budget.as_deref_mut())
}

pub(crate) fn make_array_value(
    values: Vec<Value>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<Value> {
    check_collection_len("array", 0, values.len(), budget.as_deref(), |budget| {
        budget.collection_limits().max_array_len
    })?;
    let Some(heap) = heap.as_deref_mut() else {
        return type_error(operation);
    };
    allocate_heap_value(HeapValue::Array(values), heap, budget.as_deref_mut())
}

pub(crate) fn make_script_map_value(
    values: ScriptMap,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<Value> {
    check_collection_len("map", 0, values.len(), budget.as_deref(), |budget| {
        budget.collection_limits().max_map_entries
    })?;
    let Some(heap) = heap.as_deref_mut() else {
        return type_error(operation);
    };
    allocate_heap_value(HeapValue::Map(values), heap, budget.as_deref_mut())
}

pub(super) fn index_out_of_bounds(index: usize, len: usize) -> VmError {
    VmError::new(VmErrorKind::IndexOutOfBounds {
        index: i64::try_from(index).unwrap_or(i64::MAX),
        len,
    })
}

pub(super) fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}

#[cfg(test)]
mod tests;
