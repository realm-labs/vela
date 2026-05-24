use crate::heap::HeapValue;
use crate::{
    ExecutionBudget, HeapExecution, Value, VmError, VmErrorKind, VmResult, value_from_heap_slot,
    value_to_heap_slot,
};

pub(crate) fn get_index(
    base: &Value,
    index: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    match base {
        Value::Array(values) => {
            let index = array_index(index)?;
            values.get(index).cloned().ok_or_else(|| {
                VmError::new(VmErrorKind::IndexOutOfBounds {
                    index: i64::try_from(index).unwrap_or(i64::MAX),
                    len: values.len(),
                })
            })
        }
        Value::Map(values) => {
            let key = map_key(index, heap)?;
            values
                .get(&key)
                .cloned()
                .ok_or_else(|| VmError::new(VmErrorKind::UnknownMapKey { key }))
        }
        Value::HeapRef(reference) => {
            let Some(heap_value) = heap.and_then(|heap| heap.heap.get(*reference)) else {
                return Err(VmError::new(VmErrorKind::TypeMismatch {
                    operation: "index",
                }));
            };
            match heap_value {
                HeapValue::Array(values) => {
                    let index = array_index(index)?;
                    values.get(index).map(value_from_heap_slot).ok_or_else(|| {
                        VmError::new(VmErrorKind::IndexOutOfBounds {
                            index: i64::try_from(index).unwrap_or(i64::MAX),
                            len: values.len(),
                        })
                    })
                }
                HeapValue::Map(values) => {
                    let key = map_key(index, heap)?;
                    values
                        .get(&key)
                        .map(value_from_heap_slot)
                        .ok_or_else(|| VmError::new(VmErrorKind::UnknownMapKey { key }))
                }
                HeapValue::String(_)
                | HeapValue::Set(_)
                | HeapValue::Record { .. }
                | HeapValue::Enum { .. } => Err(VmError::new(VmErrorKind::TypeMismatch {
                    operation: "index",
                })),
            }
        }
        Value::Null
        | Value::Missing
        | Value::Bool(_)
        | Value::Int(_)
        | Value::Float(_)
        | Value::String(_)
        | Value::Set(_)
        | Value::Record { .. }
        | Value::Enum { .. }
        | Value::Range(_)
        | Value::Closure(_)
        | Value::Iterator(_)
        | Value::HostRef(_) => Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "index",
        })),
    }
}

pub(crate) fn set_index(
    base: &mut Value,
    index: &Value,
    src: &Value,
    heap: Option<&mut HeapExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<()> {
    match base {
        Value::Array(values) => {
            let index = array_index(index)?;
            let len = values.len();
            let slot = values.get_mut(index).ok_or_else(|| {
                VmError::new(VmErrorKind::IndexOutOfBounds {
                    index: i64::try_from(index).unwrap_or(i64::MAX),
                    len,
                })
            })?;
            *slot = src.clone();
            Ok(())
        }
        Value::Map(values) => {
            let key = map_key(index, heap.as_deref())?;
            values.insert(key, src.clone());
            Ok(())
        }
        Value::HeapRef(reference) => {
            let Some(heap) = heap else {
                return Err(VmError::new(VmErrorKind::TypeMismatch {
                    operation: "index assignment",
                }));
            };
            match heap.heap.get(*reference) {
                Some(HeapValue::Array(_)) => {
                    set_heap_array_index(*reference, index, src, heap, budget)
                }
                Some(HeapValue::Map(_)) => set_heap_map_index(*reference, index, src, heap, budget),
                Some(
                    HeapValue::String(_)
                    | HeapValue::Set(_)
                    | HeapValue::Record { .. }
                    | HeapValue::Enum { .. },
                )
                | None => Err(VmError::new(VmErrorKind::TypeMismatch {
                    operation: "index assignment",
                })),
            }
        }
        Value::Null
        | Value::Missing
        | Value::Bool(_)
        | Value::Int(_)
        | Value::Float(_)
        | Value::String(_)
        | Value::Set(_)
        | Value::Record { .. }
        | Value::Enum { .. }
        | Value::Range(_)
        | Value::Closure(_)
        | Value::Iterator(_)
        | Value::HostRef(_) => Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "index assignment",
        })),
    }
}

fn set_heap_array_index(
    reference: crate::heap::GcRef,
    index: &Value,
    src: &Value,
    heap: &mut HeapExecution<'_>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<()> {
    let index = array_index(index)?;
    let slot = value_to_heap_slot(src, heap, budget)?;
    let HeapValue::Array(values) = heap.heap.get_mut(reference).map_err(|_| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "index assignment",
        })
    })?
    else {
        return Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "index assignment",
        }));
    };
    let len = values.len();
    let target = values.get_mut(index).ok_or_else(|| {
        VmError::new(VmErrorKind::IndexOutOfBounds {
            index: i64::try_from(index).unwrap_or(i64::MAX),
            len,
        })
    })?;
    *target = slot;
    Ok(())
}

fn set_heap_map_index(
    reference: crate::heap::GcRef,
    index: &Value,
    src: &Value,
    heap: &mut HeapExecution<'_>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<()> {
    let key = map_key(index, Some(&*heap))?;
    let slot = value_to_heap_slot(src, heap, budget)?;
    let HeapValue::Map(values) = heap.heap.get_mut(reference).map_err(|_| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "index assignment",
        })
    })?
    else {
        return Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "index assignment",
        }));
    };
    values.insert(key, slot);
    Ok(())
}

fn array_index(index: &Value) -> VmResult<usize> {
    match index {
        Value::Int(index) if *index >= 0 => Ok(*index as usize),
        Value::Null
        | Value::Missing
        | Value::Bool(_)
        | Value::Int(_)
        | Value::Float(_)
        | Value::String(_)
        | Value::Array(_)
        | Value::Set(_)
        | Value::Map(_)
        | Value::Record { .. }
        | Value::Enum { .. }
        | Value::HeapRef(_)
        | Value::Range(_)
        | Value::Closure(_)
        | Value::Iterator(_)
        | Value::HostRef(_) => Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "array index",
        })),
    }
}

fn map_key(index: &Value, heap: Option<&HeapExecution<'_>>) -> VmResult<String> {
    match index {
        Value::String(key) => Ok(key.clone()),
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::String(key)) => Ok(key.clone()),
            _ => Err(VmError::new(VmErrorKind::TypeMismatch {
                operation: "map key",
            })),
        },
        Value::Null
        | Value::Missing
        | Value::Bool(_)
        | Value::Int(_)
        | Value::Float(_)
        | Value::Array(_)
        | Value::Set(_)
        | Value::Map(_)
        | Value::Record { .. }
        | Value::Enum { .. }
        | Value::Range(_)
        | Value::Closure(_)
        | Value::Iterator(_)
        | Value::HostRef(_) => Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "map key",
        })),
    }
}
