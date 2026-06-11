use crate::heap::HeapValue;
use crate::{
    CallFrame, ExecutionBudget, HeapExecution, Value, VmError, VmErrorKind, VmResult,
    record_fields, stored_runtime_value,
};
use vela_bytecode::Register;
use vela_def::{TypeId, VariantId};

pub(crate) fn dispatch_get_record_field(
    frame: &mut CallFrame,
    heap: Option<&mut HeapExecution<'_>>,
    dst: Register,
    record: Register,
    field: &str,
) -> VmResult<()> {
    let value = get_record_field_value(frame.read(record)?, field, heap.as_deref())?;
    frame.write(dst, value)
}

pub(crate) fn dispatch_get_record_slot(
    frame: &mut CallFrame,
    heap: Option<&mut HeapExecution<'_>>,
    dst: Register,
    record: Register,
    field: &str,
    slot: usize,
) -> VmResult<()> {
    let value = get_record_slot_value(frame.read(record)?, field, slot, heap.as_deref())?;
    frame.write(dst, value)
}

pub(crate) fn dispatch_set_record_field(
    frame: &mut CallFrame,
    heap: Option<&mut HeapExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
    record: Register,
    field: &str,
    src: Register,
) -> VmResult<()> {
    let mut record_value = *frame.read(record)?;
    let src = *frame.read(src)?;
    record_fields::set_record_field_value(&mut record_value, field, &src, heap, budget)?;
    frame.write(record, record_value)
}

pub(crate) fn dispatch_set_record_slot(
    frame: &mut CallFrame,
    heap: Option<&mut HeapExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
    record: Register,
    field: &str,
    slot: usize,
    src: Register,
) -> VmResult<()> {
    let mut record_value = *frame.read(record)?;
    let src = *frame.read(src)?;
    record_fields::set_record_slot_value(&mut record_value, field, slot, &src, heap, budget)?;
    frame.write(record, record_value)
}

pub(crate) fn dispatch_get_enum_field(
    frame: &mut CallFrame,
    heap: Option<&mut HeapExecution<'_>>,
    dst: Register,
    value: Register,
    field: &str,
) -> VmResult<()> {
    let value = get_enum_field_value(frame.read(value)?, field, heap.as_deref())?;
    frame.write(dst, value)
}

pub(crate) fn dispatch_get_enum_slot(
    frame: &mut CallFrame,
    heap: Option<&mut HeapExecution<'_>>,
    dst: Register,
    value: Register,
    field: &str,
    slot: usize,
) -> VmResult<()> {
    let value = get_enum_slot_value(frame.read(value)?, field, slot, heap.as_deref())?;
    frame.write(dst, value)
}

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
                ..
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
                ..
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

pub(crate) fn enum_tag_id_equal(
    value: &Value,
    type_id: TypeId,
    variant_id: VariantId,
    heap: Option<&HeapExecution<'_>>,
) -> bool {
    match value {
        Value::HeapRef(reference) => matches!(
            heap.and_then(|heap| heap.heap.get(*reference)),
            Some(HeapValue::Enum {
                identity: Some(identity),
                ..
            }) if identity.type_id == type_id && identity.variant_id == variant_id
        ),
        _ => false,
    }
}

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}
