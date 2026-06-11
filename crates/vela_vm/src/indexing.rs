use crate::heap::HeapValue;
use crate::{
    ExecutionBudget, HeapExecution, Value, VmError, VmErrorKind, VmResult, store_runtime_value,
    stored_runtime_value,
};

pub(crate) fn get_index(
    base: &Value,
    index: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    match base {
        Value::HeapRef(reference) => {
            let Some(heap_value) = heap.and_then(|heap| heap.heap.get(*reference)) else {
                return Err(VmError::new(VmErrorKind::TypeMismatch {
                    operation: "index",
                }));
            };
            match heap_value {
                HeapValue::Array(values) => {
                    let index = array_index(index)?;
                    values.get(index).map(stored_runtime_value).ok_or_else(|| {
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
                        .map(stored_runtime_value)
                        .ok_or_else(|| VmError::new(VmErrorKind::UnknownMapKey { key }))
                }
                HeapValue::String(_)
                | HeapValue::Set(_)
                | HeapValue::Record { .. }
                | HeapValue::Enum { .. }
                | HeapValue::Closure(_)
                | HeapValue::Iterator(_)
                | HeapValue::PathProxy(_) => Err(VmError::new(VmErrorKind::TypeMismatch {
                    operation: "index",
                })),
            }
        }
        _ => Err(VmError::new(VmErrorKind::TypeMismatch {
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
                    | HeapValue::Enum { .. }
                    | HeapValue::Closure(_)
                    | HeapValue::Iterator(_)
                    | HeapValue::PathProxy(_),
                )
                | None => Err(VmError::new(VmErrorKind::TypeMismatch {
                    operation: "index assignment",
                })),
            }
        }
        _ => Err(VmError::new(VmErrorKind::TypeMismatch {
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
    let slot = store_runtime_value(src, heap, budget)?;
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
    let slot = store_runtime_value(src, heap, budget)?;
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
        Value::Scalar(vela_common::ScalarValue::I64(index)) if *index >= 0 => Ok(*index as usize),
        _ => Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "array index",
        })),
    }
}

fn map_key(index: &Value, heap: Option<&HeapExecution<'_>>) -> VmResult<String> {
    match index {
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::String(key)) => Ok(key.clone()),
            _ => Err(VmError::new(VmErrorKind::TypeMismatch {
                operation: "map key",
            })),
        },
        _ => Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "map key",
        })),
    }
}
