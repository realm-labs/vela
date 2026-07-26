use crate::heap::HeapValue;
use crate::{ExecutionBudget, HeapExecution, Value, VmError, VmErrorKind, VmResult};

pub(crate) fn set_record_field_value(
    value: &mut Value,
    field: &str,
    src: &Value,
    heap: Option<&mut HeapExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<()> {
    match value {
        Value::HeapRef(reference) => {
            let Some(heap) = heap else {
                return type_error("record field assignment");
            };
            let type_name = match heap.heap.get(*reference) {
                Some(HeapValue::Record { fields, .. }) if fields.contains_key(field) => {
                    fields.owner_name().to_owned()
                }
                Some(HeapValue::Record { fields, .. }) => {
                    return Err(VmError::new(VmErrorKind::UnknownRecordField {
                        type_name: fields.owner_name().to_owned(),
                        field: field.to_owned(),
                    }));
                }
                Some(
                    HeapValue::String(_)
                    | HeapValue::Bytes(_)
                    | HeapValue::Range(_)
                    | HeapValue::Tuple(_)
                    | HeapValue::Array(_)
                    | HeapValue::Map(_)
                    | HeapValue::Set(_)
                    | HeapValue::Enum { .. }
                    | HeapValue::Closure(_)
                    | HeapValue::Iterator(_)
                    | HeapValue::PathProxy(_),
                )
                | None => return type_error("record field assignment"),
            };
            let slot = crate::store_runtime_value(src, heap, budget)?;
            let HeapValue::Record { fields, .. } = heap.heap.get_mut(*reference).map_err(|_| {
                VmError::new(VmErrorKind::UnknownRecordField {
                    type_name: type_name.clone(),
                    field: field.to_owned(),
                })
            })?
            else {
                return type_error("record field assignment");
            };
            fields.set_existing(field, slot).map_err(|_| {
                VmError::new(VmErrorKind::UnknownRecordField {
                    type_name,
                    field: field.to_owned(),
                })
            })?;
            Ok(())
        }
        _ => type_error("record field assignment"),
    }
}

pub(crate) fn set_record_slot_value(
    value: &mut Value,
    field: &str,
    slot: usize,
    src: &Value,
    heap: Option<&mut HeapExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<()> {
    match value {
        Value::HeapRef(reference) => {
            let Some(heap) = heap else {
                return type_error("record slot assignment");
            };
            let type_name = match heap.heap.get(*reference) {
                Some(HeapValue::Record { fields, .. })
                    if fields.get_slot(slot, field).is_some() =>
                {
                    fields.owner_name().to_owned()
                }
                Some(HeapValue::Record { fields, .. }) => {
                    return Err(VmError::new(VmErrorKind::UnknownRecordField {
                        type_name: fields.owner_name().to_owned(),
                        field: field.to_owned(),
                    }));
                }
                Some(
                    HeapValue::String(_)
                    | HeapValue::Bytes(_)
                    | HeapValue::Range(_)
                    | HeapValue::Tuple(_)
                    | HeapValue::Array(_)
                    | HeapValue::Map(_)
                    | HeapValue::Set(_)
                    | HeapValue::Enum { .. }
                    | HeapValue::Closure(_)
                    | HeapValue::Iterator(_)
                    | HeapValue::PathProxy(_),
                )
                | None => return type_error("record slot assignment"),
            };
            let stored_value = crate::store_runtime_value(src, heap, budget)?;
            let HeapValue::Record { fields, .. } = heap.heap.get_mut(*reference).map_err(|_| {
                VmError::new(VmErrorKind::UnknownRecordField {
                    type_name: type_name.clone(),
                    field: field.to_owned(),
                })
            })?
            else {
                return type_error("record slot assignment");
            };
            fields
                .set_slot_existing(slot, field, stored_value)
                .map_err(|_| {
                    VmError::new(VmErrorKind::UnknownRecordField {
                        type_name,
                        field: field.to_owned(),
                    })
                })
        }
        _ => type_error("record slot assignment"),
    }
}

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}
