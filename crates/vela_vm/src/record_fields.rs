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
        Value::Record { type_name, fields } => {
            let Some(slot) = fields.get_mut(field) else {
                return Err(VmError::new(VmErrorKind::UnknownRecordField {
                    type_name: type_name.clone(),
                    field: field.to_owned(),
                }));
            };
            *slot = src.clone();
            Ok(())
        }
        Value::HeapRef(reference) => {
            let Some(heap) = heap else {
                return type_error("record field assignment");
            };
            let type_name = match heap.heap.get(*reference) {
                Some(HeapValue::Record { type_name, fields }) if fields.contains_key(field) => {
                    type_name.clone()
                }
                Some(HeapValue::Record { type_name, .. }) => {
                    return Err(VmError::new(VmErrorKind::UnknownRecordField {
                        type_name: type_name.clone(),
                        field: field.to_owned(),
                    }));
                }
                Some(
                    HeapValue::String(_)
                    | HeapValue::Array(_)
                    | HeapValue::Map(_)
                    | HeapValue::Set(_)
                    | HeapValue::Enum { .. },
                )
                | None => return type_error("record field assignment"),
            };
            let slot = crate::value_to_heap_slot(src, heap, budget)?;
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
        Value::Null
        | Value::Missing
        | Value::Bool(_)
        | Value::Int(_)
        | Value::Float(_)
        | Value::String(_)
        | Value::Array(_)
        | Value::Set(_)
        | Value::Map(_)
        | Value::Enum { .. }
        | Value::Closure(_)
        | Value::Range(_)
        | Value::Iterator(_)
        | Value::HostRef(_) => type_error("record field assignment"),
    }
}

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}
