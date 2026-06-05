mod aggregation;
mod higher_order;
mod lookup;
mod mutation;
mod ordering;
mod transform;

pub(crate) use aggregation::{group_by, sum};
pub(crate) use higher_order::{all, any, count, filter, find, map};
pub(crate) use lookup::{contains, first, index_of, last};
pub(crate) use mutation::{clear, extend, insert, pop, push, remove_at};
pub(crate) use ordering::{max, min, sort, sort_by};
pub(crate) use transform::{distinct, join, reverse, slice};

use crate::heap::HeapValue;
use crate::method_runtime::{MethodRuntime, call_callback};
use crate::script_object::ScriptFields;
use crate::{
    ExecutionBudget, HeapExecution, Value, VmError, VmErrorKind, VmResult, allocate_heap_value,
    value_from_heap_slot,
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
            Ok(values.iter().map(value_from_heap_slot).collect())
        }
        _ => type_error(operation),
    }
}

pub(super) fn index_value(value: &Value, operation: &'static str) -> VmResult<usize> {
    match value {
        Value::Int(value) if *value >= 0 => Ok(*value as usize),
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

pub(super) fn is_truthy(value: &Value) -> bool {
    !matches!(value, Value::Missing | Value::Null | Value::Bool(false))
}

pub(super) fn option_value(
    variant: &str,
    payload: Option<Value>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    let fields = payload
        .map(|payload| vec![("0".to_owned(), payload)])
        .unwrap_or_default();
    make_enum_value("Option", variant, fields, heap, budget, "Option")
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
    let Some(heap) = heap.as_deref_mut() else {
        return type_error(operation);
    };
    allocate_heap_value(HeapValue::Array(values), heap, budget.as_deref_mut())
}

pub(crate) fn make_map_value(
    values: std::collections::BTreeMap<String, Value>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<Value> {
    let Some(heap) = heap.as_deref_mut() else {
        return type_error(operation);
    };
    allocate_heap_value(HeapValue::Map(values), heap, budget.as_deref_mut())
}

pub(crate) fn make_enum_value(
    enum_name: &str,
    variant: &str,
    fields: Vec<(String, Value)>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<Value> {
    let Some(heap) = heap.as_deref_mut() else {
        return type_error(operation);
    };
    allocate_heap_value(
        HeapValue::Enum {
            enum_name: enum_name.to_owned(),
            variant: variant.to_owned(),
            fields: ScriptFields::from_pairs(&format!("{enum_name}.{variant}"), fields),
        },
        heap,
        budget.as_deref_mut(),
    )
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
