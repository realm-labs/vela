use crate::collection_mutation;
use crate::heap::HeapValue;
use crate::heap_values::allocate_heap_value;
use crate::value_key::ValueKey;
use crate::{
    CallFrame, ExecutionBudget, HeapExecution, Value, VmError, VmErrorKind, VmResult,
    store_runtime_value, stored_runtime_value,
};
use vela_bytecode::Register;

pub(crate) fn dispatch_get_index(
    frame: &mut CallFrame,
    heap: Option<&HeapExecution<'_>>,
    dst: Register,
    base: Register,
    index: Register,
) -> VmResult<()> {
    let value = get_index(&frame.read(base)?, &frame.read(index)?, heap)?;
    frame.write(dst, value)
}

pub(crate) fn dispatch_get_string_key_index(
    frame: &mut CallFrame,
    heap: Option<&HeapExecution<'_>>,
    dst: Register,
    base: Register,
    key: &str,
) -> VmResult<()> {
    let value = get_string_key_index(&frame.read(base)?, key, heap)?;
    frame.write(dst, value)
}

pub(crate) fn dispatch_set_index(
    frame: &mut CallFrame,
    heap: Option<&mut HeapExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
    base: Register,
    index: Register,
    src: Register,
) -> VmResult<()> {
    let mut base_value = frame.read(base)?;
    set_index(
        &mut base_value,
        &frame.read(index)?,
        &frame.read(src)?,
        heap,
        budget,
    )?;
    frame.write(base, base_value)
}

pub(crate) fn dispatch_set_string_key_index(
    frame: &mut CallFrame,
    heap: Option<&mut HeapExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
    base: Register,
    key: &str,
    src: Register,
) -> VmResult<()> {
    set_string_key_index(&frame.read(base)?, key, &frame.read(src)?, heap, budget)
}

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
                    let payload = values.get(index, heap, "index")?;
                    payload.ok_or_else(|| {
                        VmError::new(VmErrorKind::UnknownMapKey {
                            key: map_key_label(index, heap),
                        })
                    })
                }
                HeapValue::Bytes(values) => {
                    let (index, diagnostic_index) = bytes_index(index, values.len())?;
                    values
                        .get(index)
                        .map(|value| Value::U8(*value))
                        .ok_or_else(|| {
                            VmError::new(VmErrorKind::IndexOutOfBounds {
                                index: diagnostic_index,
                                len: values.len(),
                            })
                        })
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

pub(crate) fn get_string_key_index(
    base: &Value,
    key: &str,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    let Value::HeapRef(reference) = base else {
        return Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "index",
        }));
    };
    let Some(heap_value) = heap.and_then(|heap| heap.heap.get(*reference)) else {
        return Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "index",
        }));
    };
    match heap_value {
        HeapValue::Map(values) => values
            .get_keyed(&ValueKey::String(key.to_owned()))
            .ok_or_else(|| {
                VmError::new(VmErrorKind::UnknownMapKey {
                    key: key.to_owned(),
                })
            }),
        HeapValue::Array(_)
        | HeapValue::Bytes(_)
        | HeapValue::String(_)
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
                    | HeapValue::Bytes(_)
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

pub(crate) fn set_string_key_index(
    base: &Value,
    key: &str,
    src: &Value,
    heap: Option<&mut HeapExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<()> {
    let Value::HeapRef(reference) = base else {
        return Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "index assignment",
        }));
    };
    let Some(heap) = heap else {
        return Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "index assignment",
        }));
    };
    match heap.heap.get(*reference) {
        Some(HeapValue::Map(_)) => {
            set_heap_map_string_key_index(*reference, key, src, heap, budget)
        }
        Some(
            HeapValue::Array(_)
            | HeapValue::String(_)
            | HeapValue::Bytes(_)
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
    heap.heap
        .note_container_value_replaced_or_removed(reference);
    Ok(())
}

fn set_heap_map_index(
    reference: crate::heap::GcRef,
    index: &Value,
    src: &Value,
    heap: &mut HeapExecution<'_>,
    mut budget: Option<&mut ExecutionBudget>,
) -> VmResult<()> {
    let key = store_runtime_value(index, heap, budget.as_deref_mut())?;
    let slot = store_runtime_value(src, heap, budget.as_deref_mut())?;
    collection_mutation::insert_map_slot(heap, reference, key, slot, budget, "index assignment")?;
    Ok(())
}

fn set_heap_map_string_key_index(
    reference: crate::heap::GcRef,
    key: &str,
    src: &Value,
    heap: &mut HeapExecution<'_>,
    mut budget: Option<&mut ExecutionBudget>,
) -> VmResult<()> {
    let key = allocate_heap_value(
        HeapValue::String(key.to_owned()),
        heap,
        budget.as_deref_mut(),
    )?;
    let slot = store_runtime_value(src, heap, budget.as_deref_mut())?;
    collection_mutation::insert_map_slot(heap, reference, key, slot, budget, "index assignment")?;
    Ok(())
}

fn array_index(index: &Value) -> VmResult<usize> {
    match index {
        Value::I64(index) if *index >= 0 => Ok(*index as usize),
        _ => Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "array index",
        })),
    }
}

fn bytes_index(index: &Value, len: usize) -> VmResult<(usize, i64)> {
    match index {
        Value::I64(index) if *index >= 0 => Ok((*index as usize, *index)),
        Value::I64(index) => Err(VmError::new(VmErrorKind::IndexOutOfBounds {
            index: *index,
            len,
        })),
        _ => Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "bytes index",
        })),
    }
}

fn map_key_label(index: &Value, heap: Option<&HeapExecution<'_>>) -> String {
    map_key(index, heap).unwrap_or_else(|_| format!("{index:?}"))
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
