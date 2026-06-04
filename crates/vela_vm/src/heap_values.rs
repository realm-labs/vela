use std::collections::BTreeMap;
use std::sync::Arc;

use vela_bytecode::Constant;

use crate::budget::ExecutionBudget;
use crate::error::{VmError, VmErrorKind, VmResult};
use crate::heap::{HeapSlot, HeapValue};
use crate::heap_execution::HeapExecution;
use crate::script_object::ScriptFields;
use crate::value::{ClosureValue, Value};

pub(crate) fn value_from_constant(
    constant: &Constant,
    heap: Option<&mut HeapExecution<'_>>,
    mut budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    match (constant, heap) {
        (Constant::String(value), Some(heap)) => {
            allocate_heap_value(HeapValue::String(value.clone()), heap, budget)
        }
        (Constant::Array(values), Some(heap)) => {
            let values = values.iter().map(Value::from).collect::<Vec<_>>();
            let slots = values_to_heap_slots(&values, heap, budget.as_deref_mut())?;
            allocate_heap_value(HeapValue::Array(slots), heap, budget)
        }
        (Constant::Map(entries), Some(heap)) => {
            let values = entries
                .iter()
                .map(|(key, value)| (key.clone(), Value::from(value)))
                .collect::<BTreeMap<_, _>>();
            let slots = values_to_heap_map(&values, heap, budget.as_deref_mut())?;
            allocate_heap_value(HeapValue::Map(slots), heap, budget)
        }
        _ => Ok(Value::from(constant)),
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

pub(crate) fn values_to_heap_slots(
    values: &[Value],
    heap: &mut HeapExecution<'_>,
    mut budget: Option<&mut ExecutionBudget>,
) -> VmResult<Vec<HeapSlot>> {
    values
        .iter()
        .map(|value| value_to_heap_slot(value, heap, budget.as_deref_mut()))
        .collect()
}

pub(crate) fn values_to_heap_map(
    values: &BTreeMap<String, Value>,
    heap: &mut HeapExecution<'_>,
    mut budget: Option<&mut ExecutionBudget>,
) -> VmResult<BTreeMap<String, HeapSlot>> {
    values
        .iter()
        .map(|(key, value)| {
            Ok((
                key.clone(),
                value_to_heap_slot(value, heap, budget.as_deref_mut())?,
            ))
        })
        .collect()
}

pub(crate) fn values_to_heap_fields(
    owner: &str,
    values: &ScriptFields<Value>,
    heap: &mut HeapExecution<'_>,
    mut budget: Option<&mut ExecutionBudget>,
) -> VmResult<ScriptFields<HeapSlot>> {
    values
        .iter()
        .map(|(key, value)| {
            Ok((
                key.to_owned(),
                value_to_heap_slot(value, heap, budget.as_deref_mut())?,
            ))
        })
        .collect::<VmResult<Vec<_>>>()
        .map(|fields| ScriptFields::from_pairs(owner, fields))
}

fn values_into_heap_slots(
    values: Vec<Value>,
    heap: &mut HeapExecution<'_>,
    mut budget: Option<&mut ExecutionBudget>,
) -> VmResult<Vec<HeapSlot>> {
    values
        .into_iter()
        .map(|value| value_into_heap_slot(value, heap, budget.as_deref_mut()))
        .collect()
}

fn values_into_heap_map(
    values: BTreeMap<String, Value>,
    heap: &mut HeapExecution<'_>,
    mut budget: Option<&mut ExecutionBudget>,
) -> VmResult<BTreeMap<String, HeapSlot>> {
    values
        .into_iter()
        .map(|(key, value)| {
            Ok((
                key,
                value_into_heap_slot(value, heap, budget.as_deref_mut())?,
            ))
        })
        .collect()
}

fn values_into_heap_fields(
    owner: &str,
    values: ScriptFields<Value>,
    heap: &mut HeapExecution<'_>,
    mut budget: Option<&mut ExecutionBudget>,
) -> VmResult<ScriptFields<HeapSlot>> {
    values
        .into_pairs()
        .map(|(key, value)| {
            Ok((
                key,
                value_into_heap_slot(value, heap, budget.as_deref_mut())?,
            ))
        })
        .collect::<VmResult<Vec<_>>>()
        .map(|fields| ScriptFields::from_pairs(owner, fields))
}

pub(crate) fn enum_variant_owner(enum_name: &str, variant: &str) -> String {
    format!("{enum_name}::{variant}")
}

pub(crate) fn value_to_heap_slot(
    value: &Value,
    heap: &mut HeapExecution<'_>,
    mut budget: Option<&mut ExecutionBudget>,
) -> VmResult<HeapSlot> {
    match value {
        Value::Null => Ok(HeapSlot::Null),
        Value::Bool(value) => Ok(HeapSlot::Bool(*value)),
        Value::Int(value) => Ok(HeapSlot::Int(*value)),
        Value::Float(value) => Ok(HeapSlot::Float(*value)),
        Value::HeapRef(reference) => Ok(HeapSlot::Ref(*reference)),
        Value::HostRef(reference) => Ok(HeapSlot::HostRef(*reference)),
        Value::PathProxy(proxy) => Ok(HeapSlot::PathProxy(proxy.clone())),
        Value::String(value) => {
            let Value::HeapRef(reference) =
                allocate_heap_value(HeapValue::String(value.clone()), heap, budget)?
            else {
                unreachable!("heap allocation always returns a heap ref");
            };
            Ok(HeapSlot::Ref(reference))
        }
        Value::Array(values) => {
            let slots = values_to_heap_slots(values, heap, budget.as_deref_mut())?;
            let Value::HeapRef(reference) =
                allocate_heap_value(HeapValue::Array(slots), heap, budget)?
            else {
                unreachable!("heap allocation always returns a heap ref");
            };
            Ok(HeapSlot::Ref(reference))
        }
        Value::Set(values) => {
            let slots = values_to_heap_slots(values, heap, budget.as_deref_mut())?;
            let Value::HeapRef(reference) =
                allocate_heap_value(HeapValue::Set(slots), heap, budget)?
            else {
                unreachable!("heap allocation always returns a heap ref");
            };
            Ok(HeapSlot::Ref(reference))
        }
        Value::Map(values) => {
            let slots = values_to_heap_map(values, heap, budget.as_deref_mut())?;
            let Value::HeapRef(reference) =
                allocate_heap_value(HeapValue::Map(slots), heap, budget)?
            else {
                unreachable!("heap allocation always returns a heap ref");
            };
            Ok(HeapSlot::Ref(reference))
        }
        Value::Record { type_name, fields } => {
            let slots = values_to_heap_fields(type_name, fields, heap, budget.as_deref_mut())?;
            let Value::HeapRef(reference) = allocate_heap_value(
                HeapValue::Record {
                    type_name: type_name.clone(),
                    fields: slots,
                },
                heap,
                budget,
            )?
            else {
                unreachable!("heap allocation always returns a heap ref");
            };
            Ok(HeapSlot::Ref(reference))
        }
        Value::Enum {
            enum_name,
            variant,
            fields,
        } => {
            let owner = enum_variant_owner(enum_name, variant);
            let slots = values_to_heap_fields(&owner, fields, heap, budget.as_deref_mut())?;
            let Value::HeapRef(reference) = allocate_heap_value(
                HeapValue::Enum {
                    enum_name: enum_name.clone(),
                    variant: variant.clone(),
                    fields: slots,
                },
                heap,
                budget,
            )?
            else {
                unreachable!("heap allocation always returns a heap ref");
            };
            Ok(HeapSlot::Ref(reference))
        }
        Value::Range(_) | Value::Closure(_) | Value::Iterator(_) | Value::Missing => {
            Err(VmError::new(VmErrorKind::TypeMismatch {
                operation: "heap slot",
            }))
        }
    }
}

fn value_into_heap_slot(
    value: Value,
    heap: &mut HeapExecution<'_>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<HeapSlot> {
    match value {
        Value::Null => Ok(HeapSlot::Null),
        Value::Bool(value) => Ok(HeapSlot::Bool(value)),
        Value::Int(value) => Ok(HeapSlot::Int(value)),
        Value::Float(value) => Ok(HeapSlot::Float(value)),
        Value::HeapRef(reference) => Ok(HeapSlot::Ref(reference)),
        Value::HostRef(reference) => Ok(HeapSlot::HostRef(reference)),
        Value::PathProxy(proxy) => Ok(HeapSlot::PathProxy(proxy)),
        Value::String(_)
        | Value::Array(_)
        | Value::Set(_)
        | Value::Map(_)
        | Value::Record { .. }
        | Value::Enum { .. } => {
            if let Value::HeapRef(reference) = store_owned_heap_value(value, heap, budget)? {
                Ok(HeapSlot::Ref(reference))
            } else {
                unreachable!("heap allocation always returns a heap ref");
            }
        }
        Value::Range(_) | Value::Closure(_) | Value::Iterator(_) | Value::Missing => {
            Err(VmError::new(VmErrorKind::TypeMismatch {
                operation: "heap slot",
            }))
        }
    }
}

fn store_owned_heap_value(
    value: Value,
    heap: &mut HeapExecution<'_>,
    mut budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    match value {
        Value::String(value) => allocate_heap_value(HeapValue::String(value), heap, budget),
        Value::Array(values) => {
            let slots = values_into_heap_slots(values, heap, budget.as_deref_mut())?;
            allocate_heap_value(HeapValue::Array(slots), heap, budget)
        }
        Value::Set(values) => {
            let slots = values_into_heap_slots(values, heap, budget.as_deref_mut())?;
            allocate_heap_value(HeapValue::Set(slots), heap, budget)
        }
        Value::Map(values) => {
            let slots = values_into_heap_map(values, heap, budget.as_deref_mut())?;
            allocate_heap_value(HeapValue::Map(slots), heap, budget)
        }
        Value::Record { type_name, fields } => {
            let slots = values_into_heap_fields(&type_name, fields, heap, budget.as_deref_mut())?;
            allocate_heap_value(
                HeapValue::Record {
                    type_name,
                    fields: slots,
                },
                heap,
                budget,
            )
        }
        Value::Enum {
            enum_name,
            variant,
            fields,
        } => {
            let owner = enum_variant_owner(&enum_name, &variant);
            let slots = values_into_heap_fields(&owner, fields, heap, budget.as_deref_mut())?;
            allocate_heap_value(
                HeapValue::Enum {
                    enum_name,
                    variant,
                    fields: slots,
                },
                heap,
                budget,
            )
        }
        _ => unreachable!("only owned heap aggregate values can be stored"),
    }
}

pub(crate) fn value_from_heap_slot(slot: &HeapSlot) -> Value {
    match slot {
        HeapSlot::Null => Value::Null,
        HeapSlot::Bool(value) => Value::Bool(*value),
        HeapSlot::Int(value) => Value::Int(*value),
        HeapSlot::Float(value) => Value::Float(*value),
        HeapSlot::Ref(reference) => Value::HeapRef(*reference),
        HeapSlot::HostRef(reference) => Value::HostRef(*reference),
        HeapSlot::PathProxy(proxy) => Value::PathProxy(proxy.clone()),
    }
}

pub(crate) fn materialize_values(
    values: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Vec<Value>> {
    values
        .iter()
        .map(|value| materialize_value(value, heap))
        .collect()
}

pub(crate) fn materialize_value(
    value: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    match value {
        Value::HeapRef(reference) => {
            let Some(heap_value) = heap.and_then(|heap| heap.heap.get(*reference)) else {
                return Err(VmError::new(VmErrorKind::TypeMismatch {
                    operation: "heap ref",
                }));
            };
            materialize_heap_value(heap_value, heap)
        }
        Value::Array(values) => Ok(Value::Array(materialize_values(values, heap)?)),
        Value::Set(values) => Ok(Value::Set(materialize_values(values, heap)?)),
        Value::Map(values) => values
            .iter()
            .map(|(key, value)| Ok((key.clone(), materialize_value(value, heap)?)))
            .collect::<VmResult<BTreeMap<_, _>>>()
            .map(Value::Map),
        Value::Record { type_name, fields } => fields
            .iter()
            .map(|(key, value)| Ok((key.to_owned(), materialize_value(value, heap)?)))
            .collect::<VmResult<Vec<_>>>()
            .map(|fields| Value::Record {
                type_name: type_name.clone(),
                fields: ScriptFields::from_pairs(type_name, fields),
            }),
        Value::Enum {
            enum_name,
            variant,
            fields,
        } => fields
            .iter()
            .map(|(key, value)| Ok((key.to_owned(), materialize_value(value, heap)?)))
            .collect::<VmResult<Vec<_>>>()
            .map(|fields| Value::Enum {
                enum_name: enum_name.clone(),
                variant: variant.clone(),
                fields: ScriptFields::from_pairs(&enum_variant_owner(enum_name, variant), fields),
            }),
        Value::Closure(closure) => closure
            .captures
            .iter()
            .map(|capture| materialize_value(capture, heap))
            .collect::<VmResult<Vec<_>>>()
            .map(|captures| {
                Value::Closure(ClosureValue {
                    code: Arc::clone(&closure.code),
                    captures,
                })
            }),
        Value::Null
        | Value::Bool(_)
        | Value::Int(_)
        | Value::Float(_)
        | Value::String(_)
        | Value::Range(_)
        | Value::HostRef(_)
        | Value::PathProxy(_) => Ok(value.clone()),
        Value::Iterator(_) | Value::Missing => Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "materialize",
        })),
    }
}

fn materialize_heap_value(value: &HeapValue, heap: Option<&HeapExecution<'_>>) -> VmResult<Value> {
    match value {
        HeapValue::String(value) => Ok(Value::String(value.clone())),
        HeapValue::Array(values) => values
            .iter()
            .map(|value| materialize_heap_slot(value, heap))
            .collect::<VmResult<Vec<_>>>()
            .map(Value::Array),
        HeapValue::Map(values) => values
            .iter()
            .map(|(key, value)| Ok((key.clone(), materialize_heap_slot(value, heap)?)))
            .collect::<VmResult<BTreeMap<_, _>>>()
            .map(Value::Map),
        HeapValue::Record { type_name, fields } => fields
            .iter()
            .map(|(key, value)| Ok((key.to_owned(), materialize_heap_slot(value, heap)?)))
            .collect::<VmResult<Vec<_>>>()
            .map(|fields| Value::Record {
                type_name: type_name.clone(),
                fields: ScriptFields::from_pairs(type_name, fields),
            }),
        HeapValue::Enum {
            enum_name,
            variant,
            fields,
        } => fields
            .iter()
            .map(|(key, value)| Ok((key.to_owned(), materialize_heap_slot(value, heap)?)))
            .collect::<VmResult<Vec<_>>>()
            .map(|fields| Value::Enum {
                enum_name: enum_name.clone(),
                variant: variant.clone(),
                fields: ScriptFields::from_pairs(&enum_variant_owner(enum_name, variant), fields),
            }),
        HeapValue::Set(values) => values
            .iter()
            .map(|value| materialize_heap_slot(value, heap))
            .collect::<VmResult<Vec<_>>>()
            .map(Value::Set),
    }
}

fn materialize_heap_slot(slot: &HeapSlot, heap: Option<&HeapExecution<'_>>) -> VmResult<Value> {
    match slot {
        HeapSlot::Ref(reference) => materialize_value(&Value::HeapRef(*reference), heap),
        _ => Ok(value_from_heap_slot(slot)),
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
        (Value::String(lhs), Value::String(rhs)) => Some(lhs == rhs),
        (
            Value::Null | Value::Bool(_) | Value::Int(_) | Value::Float(_) | Value::String(_),
            Value::Null | Value::Bool(_) | Value::Int(_) | Value::Float(_) | Value::String(_),
        ) => Some(false),
        _ => None,
    }
}

pub(crate) fn store_value_in_heap_if_needed(
    value: Value,
    heap: Option<&mut HeapExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    let Some(heap) = heap else {
        return Ok(value);
    };
    match value {
        Value::String(value) => allocate_heap_value(HeapValue::String(value), heap, budget),
        Value::Array(_)
        | Value::Set(_)
        | Value::Map(_)
        | Value::Record { .. }
        | Value::Enum { .. } => store_owned_heap_value(value, heap, budget),
        Value::Null
        | Value::Bool(_)
        | Value::Int(_)
        | Value::Float(_)
        | Value::HeapRef(_)
        | Value::HostRef(_)
        | Value::PathProxy(_)
        | Value::Range(_)
        | Value::Closure(_)
        | Value::Iterator(_) => Ok(value),
        Value::Missing => Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "missing value",
        })),
    }
}

pub(crate) fn finish_managed_heap_result(
    result: VmResult<Value>,
    heap: &mut HeapExecution<'_>,
    budget: &mut ExecutionBudget,
) -> VmResult<Value> {
    let result = result.and_then(|value| materialize_value(&value, Some(heap)));
    heap.heap.collect_full_with_budget(&[], Some(budget));
    result
}
