use crate::heap::HeapValue;
use crate::{HeapExecution, Value, VmError, VmErrorKind, VmResult, stored_runtime_value};

pub(crate) fn get_record_field_value(
    value: &Value,
    field: &str,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    match value {
        Value::HeapRef(reference) => {
            let Some(HeapValue::Record { type_name, fields }) =
                heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error("record field");
            };
            fields.get(field).map(stored_runtime_value).ok_or_else(|| {
                VmError::new(VmErrorKind::UnknownRecordField {
                    type_name: type_name.clone(),
                    field: field.to_owned(),
                })
            })
        }
        _ => type_error("record field"),
    }
}

pub(crate) fn get_record_slot_value(
    value: &Value,
    field: &str,
    slot: usize,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    match value {
        Value::HeapRef(reference) => {
            let Some(HeapValue::Record { type_name, fields }) =
                heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error("record slot");
            };
            fields
                .get_slot(slot, field)
                .map(stored_runtime_value)
                .ok_or_else(|| {
                    VmError::new(VmErrorKind::UnknownRecordField {
                        type_name: type_name.clone(),
                        field: field.to_owned(),
                    })
                })
        }
        _ => type_error("record slot"),
    }
}

pub(crate) fn get_enum_field_value(
    value: &Value,
    field: &str,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    match value {
        Value::HeapRef(reference) => {
            let Some(HeapValue::Enum {
                enum_name,
                variant,
                fields,
            }) = heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error("enum field");
            };
            fields.get(field).map(stored_runtime_value).ok_or_else(|| {
                VmError::new(VmErrorKind::UnknownEnumField {
                    enum_name: enum_name.clone(),
                    variant: variant.clone(),
                    field: field.to_owned(),
                })
            })
        }
        _ => type_error("enum field"),
    }
}

pub(crate) fn get_enum_slot_value(
    value: &Value,
    field: &str,
    slot: usize,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    match value {
        Value::HeapRef(reference) => {
            let Some(HeapValue::Enum {
                enum_name,
                variant,
                fields,
            }) = heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error("enum slot");
            };
            fields
                .get_slot(slot, field)
                .map(stored_runtime_value)
                .ok_or_else(|| {
                    VmError::new(VmErrorKind::UnknownEnumField {
                        enum_name: enum_name.clone(),
                        variant: variant.clone(),
                        field: field.to_owned(),
                    })
                })
        }
        _ => type_error("enum slot"),
    }
}

pub(crate) fn enum_tag_equal(
    value: &Value,
    enum_name: &str,
    variant: &str,
    heap: Option<&HeapExecution<'_>>,
) -> bool {
    match value {
        Value::HeapRef(reference) => matches!(
            heap.and_then(|heap| heap.heap.get(*reference)),
            Some(HeapValue::Enum {
                enum_name: value_enum,
                variant: value_variant,
                ..
            }) if value_enum == enum_name && value_variant == variant
        ),
        _ => false,
    }
}

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}
