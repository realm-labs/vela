use std::collections::BTreeMap;
use std::sync::Arc;

use vela_bytecode::Constant;
use vela_host::value::HostValue;

use crate::budget::ExecutionBudget;
use crate::error::{VmError, VmErrorKind, VmResult};
use crate::heap::HeapValue;
use crate::heap_execution::HeapExecution;
use crate::option_result::std_enum_identity_for_names;
use crate::owned_value::{OwnedClosureValue, OwnedIteratorState, OwnedValue};
use crate::script_object::ScriptFields;
use crate::value::{ClosureCode, ClosureValue, Value};

pub(crate) fn value_from_constant(
    constant: &Constant,
    heap: Option<&mut HeapExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    match constant {
        Constant::Null => Ok(Value::Null),
        Constant::Bool(value) => Ok(Value::Bool(*value)),
        Constant::Int(value) => Ok(Value::Int(*value)),
        Constant::Float(value) => Ok(Value::Float(*value)),
        Constant::String(value) => {
            let Some(heap) = heap else {
                return Err(type_error("constant string"));
            };
            allocate_heap_value(HeapValue::String(value.clone()), heap, budget)
        }
        Constant::Array(values) => {
            let Some(mut heap) = heap else {
                return Err(type_error("constant array"));
            };
            let mut budget = budget;
            let values = values
                .iter()
                .map(|value| value_from_constant(value, Some(&mut heap), budget.as_deref_mut()))
                .collect::<VmResult<Vec<_>>>()?;
            allocate_heap_value(HeapValue::Array(values), heap, budget)
        }
        Constant::Map(entries) => {
            let Some(mut heap) = heap else {
                return Err(type_error("constant map"));
            };
            let mut budget = budget;
            let values = entries
                .iter()
                .map(|(key, value)| {
                    Ok((
                        key.clone(),
                        value_from_constant(value, Some(&mut heap), budget.as_deref_mut())?,
                    ))
                })
                .collect::<VmResult<BTreeMap<_, _>>>()?;
            allocate_heap_value(HeapValue::Map(values), heap, budget)
        }
    }
}

pub(crate) fn allocate_heap_value(
    value: HeapValue,
    heap: &mut HeapExecution<'_>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    let reference = if let Some(budget) = budget {
        heap.heap.allocate_with_budget(value, budget)?
    } else {
        heap.heap.allocate(value)
    };
    Ok(Value::HeapRef(reference))
}

pub(crate) fn store_runtime_value(
    value: &Value,
    heap: &mut HeapExecution<'_>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    store_value_in_heap(*value, heap, budget)
}

pub(crate) fn stored_runtime_value(value: &Value) -> Value {
    *value
}

#[allow(dead_code)]
pub(crate) fn make_string_value(
    value: String,
    heap: &mut HeapExecution<'_>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    allocate_heap_value(HeapValue::String(value), heap, budget)
}

#[allow(dead_code)]
pub(crate) fn make_array_value(
    values: Vec<Value>,
    heap: &mut HeapExecution<'_>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    allocate_heap_value(HeapValue::Array(values), heap, budget)
}

#[allow(dead_code)]
pub(crate) fn make_map_value(
    values: BTreeMap<String, Value>,
    heap: &mut HeapExecution<'_>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    allocate_heap_value(HeapValue::Map(values), heap, budget)
}

#[allow(dead_code)]
pub(crate) fn make_set_value(
    values: Vec<Value>,
    heap: &mut HeapExecution<'_>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    allocate_heap_value(HeapValue::Set(values), heap, budget)
}

#[allow(dead_code)]
pub(crate) fn make_enum_value(
    enum_name: impl Into<String>,
    variant: impl Into<String>,
    fields: Vec<(String, Value)>,
    heap: &mut HeapExecution<'_>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    let enum_name = enum_name.into();
    let variant = variant.into();
    let owner = enum_variant_owner(&enum_name, &variant);
    let identity = std_enum_identity_for_names(&enum_name, &variant);
    allocate_heap_value(
        HeapValue::Enum {
            enum_name,
            variant,
            identity,
            fields: ScriptFields::from_pairs(&owner, fields),
        },
        heap,
        budget,
    )
}

pub(crate) fn enum_variant_owner(enum_name: &str, variant: &str) -> String {
    format!("{enum_name}::{variant}")
}

#[allow(dead_code)]
pub(crate) fn owned_to_value(
    value: OwnedValue,
    heap: &mut HeapExecution<'_>,
    mut budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    match value {
        OwnedValue::Missing => Ok(Value::Missing),
        OwnedValue::Null => Ok(Value::Null),
        OwnedValue::Bool(value) => Ok(Value::Bool(value)),
        OwnedValue::Int(value) => Ok(Value::Int(value)),
        OwnedValue::Float(value) => Ok(Value::Float(value)),
        OwnedValue::Range(value) => Ok(Value::Range(value)),
        OwnedValue::HostRef(value) => Ok(Value::HostRef(value)),
        OwnedValue::String(value) => {
            allocate_heap_value(HeapValue::String(value), heap, budget.as_deref_mut())
        }
        OwnedValue::Array(values) => {
            let values = values
                .into_iter()
                .map(|value| owned_to_value(value, heap, budget.as_deref_mut()))
                .collect::<VmResult<Vec<_>>>()?;
            allocate_heap_value(HeapValue::Array(values), heap, budget)
        }
        OwnedValue::Set(values) => {
            let values = values
                .into_iter()
                .map(|value| owned_to_value(value, heap, budget.as_deref_mut()))
                .collect::<VmResult<Vec<_>>>()?;
            allocate_heap_value(HeapValue::Set(values), heap, budget)
        }
        OwnedValue::Map(values) => {
            let values = values
                .into_iter()
                .map(|(key, value)| Ok((key, owned_to_value(value, heap, budget.as_deref_mut())?)))
                .collect::<VmResult<BTreeMap<_, _>>>()?;
            allocate_heap_value(HeapValue::Map(values), heap, budget)
        }
        OwnedValue::Record { type_name, fields } => {
            let fields = fields
                .into_pairs()
                .map(|(key, value)| Ok((key, owned_to_value(value, heap, budget.as_deref_mut())?)))
                .collect::<VmResult<Vec<_>>>()?;
            allocate_heap_value(
                HeapValue::Record {
                    fields: ScriptFields::from_pairs(&type_name, fields),
                    identity: None,
                    type_name,
                },
                heap,
                budget,
            )
        }
        OwnedValue::Enum {
            enum_name,
            variant,
            fields,
        } => {
            let owner = enum_variant_owner(&enum_name, &variant);
            let identity = std_enum_identity_for_names(&enum_name, &variant);
            let fields = fields
                .into_pairs()
                .map(|(key, value)| Ok((key, owned_to_value(value, heap, budget.as_deref_mut())?)))
                .collect::<VmResult<Vec<_>>>()?;
            allocate_heap_value(
                HeapValue::Enum {
                    fields: ScriptFields::from_pairs(&owner, fields),
                    enum_name,
                    variant,
                    identity,
                },
                heap,
                budget,
            )
        }
        OwnedValue::Closure(closure) => {
            let captures = closure
                .captures
                .into_iter()
                .map(|capture| owned_to_value(capture, heap, budget.as_deref_mut()))
                .collect::<VmResult<Vec<_>>>()?;
            allocate_heap_value(
                HeapValue::Closure(ClosureValue {
                    code: ClosureCode::Unlinked(Arc::clone(&closure.code)),
                    captures,
                }),
                heap,
                budget,
            )
        }
        OwnedValue::Iterator(iterator) => {
            let values = iterator
                .values()
                .iter()
                .cloned()
                .map(|value| owned_to_value(value, heap, budget.as_deref_mut()))
                .collect::<VmResult<Vec<_>>>()?;
            allocate_heap_value(
                HeapValue::Iterator(crate::iteration::IteratorState::from_values_at(
                    values,
                    iterator.next_index(),
                )),
                heap,
                budget,
            )
        }
        OwnedValue::PathProxy(proxy) => {
            allocate_heap_value(HeapValue::PathProxy(proxy), heap, budget)
        }
    }
}

#[allow(dead_code)]
pub(crate) fn value_to_owned(
    value: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<OwnedValue> {
    match value {
        Value::Missing => Ok(OwnedValue::Missing),
        Value::Null => Ok(OwnedValue::Null),
        Value::Bool(value) => Ok(OwnedValue::Bool(*value)),
        Value::Int(value) => Ok(OwnedValue::Int(*value)),
        Value::Float(value) => Ok(OwnedValue::Float(*value)),
        Value::Range(value) => Ok(OwnedValue::Range(*value)),
        Value::HostRef(value) => Ok(OwnedValue::HostRef(*value)),
        Value::HeapRef(reference) => {
            let Some(heap_value) = heap.and_then(|heap| heap.heap.get(*reference)) else {
                return Err(type_error("heap ref"));
            };
            heap_value_to_owned(heap_value, heap)
        }
    }
}

pub(crate) fn materialize_value(
    value: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<OwnedValue> {
    value_to_owned(value, heap)
}

#[allow(dead_code)]
fn heap_value_to_owned(
    value: &HeapValue,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<OwnedValue> {
    match value {
        HeapValue::String(value) => Ok(OwnedValue::String(value.clone())),
        HeapValue::Array(values) => values
            .iter()
            .map(|value| value_to_owned(value, heap))
            .collect::<VmResult<Vec<_>>>()
            .map(OwnedValue::Array),
        HeapValue::Map(values) => values
            .iter()
            .map(|(key, value)| Ok((key.clone(), value_to_owned(value, heap)?)))
            .collect::<VmResult<BTreeMap<_, _>>>()
            .map(OwnedValue::Map),
        HeapValue::Set(values) => values
            .iter()
            .map(|value| value_to_owned(value, heap))
            .collect::<VmResult<Vec<_>>>()
            .map(OwnedValue::Set),
        HeapValue::Record {
            type_name, fields, ..
        } => fields
            .iter()
            .map(|(key, value)| Ok((key.to_owned(), value_to_owned(value, heap)?)))
            .collect::<VmResult<Vec<_>>>()
            .map(|fields| OwnedValue::Record {
                type_name: type_name.clone(),
                fields: ScriptFields::from_pairs(type_name, fields),
            }),
        HeapValue::Enum {
            enum_name,
            variant,
            fields,
            ..
        } => fields
            .iter()
            .map(|(key, value)| Ok((key.to_owned(), value_to_owned(value, heap)?)))
            .collect::<VmResult<Vec<_>>>()
            .map(|fields| OwnedValue::Enum {
                enum_name: enum_name.clone(),
                variant: variant.clone(),
                fields: ScriptFields::from_pairs(&enum_variant_owner(enum_name, variant), fields),
            }),
        HeapValue::Closure(closure) => {
            let ClosureCode::Unlinked(code) = &closure.code else {
                return Err(VmError::new(VmErrorKind::TypeMismatch {
                    operation: "linked closure materialization",
                }));
            };
            closure
                .captures
                .iter()
                .map(|capture| value_to_owned(capture, heap))
                .collect::<VmResult<Vec<_>>>()
                .map(|captures| {
                    OwnedValue::Closure(OwnedClosureValue {
                        code: Arc::clone(code),
                        captures,
                    })
                })
        }
        HeapValue::Iterator(iterator) => iterator
            .values()
            .iter()
            .map(|value| value_to_owned(value, heap))
            .collect::<VmResult<Vec<_>>>()
            .map(|values| OwnedValue::Iterator(OwnedIteratorState::from_runtime(iterator, values))),
        HeapValue::PathProxy(proxy) => Ok(OwnedValue::PathProxy(proxy.clone())),
    }
}

#[allow(dead_code)]
pub(crate) fn host_to_value(
    value: HostValue,
    heap: &mut HeapExecution<'_>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    match value {
        HostValue::Null => Ok(Value::Null),
        HostValue::Bool(value) => Ok(Value::Bool(value)),
        HostValue::Int(value) => Ok(Value::Int(value)),
        HostValue::Float(value) => Ok(Value::Float(value)),
        HostValue::String(value) => allocate_heap_value(HeapValue::String(value), heap, budget),
        HostValue::HostRef(value) => Ok(Value::HostRef(value)),
    }
}

#[allow(dead_code)]
pub(crate) fn value_to_host(
    value: &Value,
    operation: &'static str,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<HostValue> {
    match value {
        Value::Null => Ok(HostValue::Null),
        Value::Bool(value) => Ok(HostValue::Bool(*value)),
        Value::Int(value) => Ok(HostValue::Int(*value)),
        Value::Float(value) => Ok(HostValue::Float(*value)),
        Value::HostRef(value) => Ok(HostValue::HostRef(*value)),
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::String(value)) => Ok(HostValue::String(value.clone())),
            Some(
                HeapValue::Array(_)
                | HeapValue::Map(_)
                | HeapValue::Set(_)
                | HeapValue::Record { .. }
                | HeapValue::Enum { .. },
            ) => Err(type_error(operation)),
            _ => Err(type_error(operation)),
        },
        Value::Missing | Value::Range(_) => Err(type_error(operation)),
    }
}

pub(crate) fn store_value_in_heap_if_needed(
    value: Value,
    heap: Option<&mut HeapExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    let Some(heap) = heap else {
        return if matches!(value, Value::Missing) {
            Err(type_error("missing value"))
        } else {
            Ok(value)
        };
    };
    store_value_in_heap(value, heap, budget)
}

fn store_value_in_heap(
    value: Value,
    _heap: &mut HeapExecution<'_>,
    _budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    match value {
        Value::Missing => Err(type_error("missing value")),
        Value::Null
        | Value::Bool(_)
        | Value::Int(_)
        | Value::Float(_)
        | Value::Range(_)
        | Value::HostRef(_)
        | Value::HeapRef(_) => Ok(value),
    }
}

pub(crate) fn values_equal(
    lhs: &Value,
    rhs: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    if let Some(equal) = scalar_values_equal(lhs, rhs) {
        return Ok(equal);
    }
    if let Some(equal) = heap_string_values_equal(lhs, rhs, heap) {
        return Ok(equal);
    }
    let lhs = materialize_value(lhs, heap)?;
    let rhs = materialize_value(rhs, heap)?;
    Ok(lhs == rhs)
}

fn scalar_values_equal(lhs: &Value, rhs: &Value) -> Option<bool> {
    match (lhs, rhs) {
        (Value::Null, Value::Null) => Some(true),
        (Value::Bool(lhs), Value::Bool(rhs)) => Some(lhs == rhs),
        (Value::Int(lhs), Value::Int(rhs)) => Some(lhs == rhs),
        (Value::Float(lhs), Value::Float(rhs)) => Some(lhs == rhs),
        (
            Value::Null | Value::Bool(_) | Value::Int(_) | Value::Float(_),
            Value::Null | Value::Bool(_) | Value::Int(_) | Value::Float(_),
        ) => Some(false),
        _ => None,
    }
}

fn heap_string_values_equal(
    lhs: &Value,
    rhs: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> Option<bool> {
    let (Value::HeapRef(lhs), Value::HeapRef(rhs)) = (lhs, rhs) else {
        return None;
    };
    let heap = heap?;
    let lhs = match heap.heap.get(*lhs)? {
        HeapValue::String(value) => value,
        _ => return None,
    };
    let rhs = match heap.heap.get(*rhs)? {
        HeapValue::String(value) => value,
        _ => return None,
    };
    Some(lhs == rhs)
}

fn type_error(operation: &'static str) -> VmError {
    VmError::new(VmErrorKind::TypeMismatch { operation })
}

#[cfg(test)]
mod tests {
    use crate::heap::ScriptHeap;

    use super::*;

    #[test]
    fn heap_string_equality_compares_borrowed_string_slots() {
        let mut heap = ScriptHeap::new();
        let gold = Value::HeapRef(heap.allocate(HeapValue::String("gold".to_owned())));
        let gold_again = Value::HeapRef(heap.allocate(HeapValue::String("gold".to_owned())));
        let xp = Value::HeapRef(heap.allocate(HeapValue::String("xp".to_owned())));
        let array = Value::HeapRef(heap.allocate(HeapValue::Array(Vec::new())));
        let heap = HeapExecution::new(&mut heap);

        assert_eq!(
            heap_string_values_equal(&gold, &gold_again, Some(&heap)),
            Some(true)
        );
        assert_eq!(
            heap_string_values_equal(&gold, &xp, Some(&heap)),
            Some(false)
        );
        assert_eq!(heap_string_values_equal(&gold, &array, Some(&heap)), None);
        assert_eq!(heap_string_values_equal(&gold, &gold_again, None), None);
    }
}
