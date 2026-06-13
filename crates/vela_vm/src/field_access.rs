use crate::heap::HeapValue;
use crate::{
    CallFrame, ExecutionBudget, HeapExecution, RecordFieldInlineCacheEntry, Value, VmError,
    VmErrorKind, VmInlineCaches, VmResult, record_fields, stored_runtime_value,
};
use vela_bytecode::{
    CacheSiteId, DebugNameId, FieldSlot, LinkedProgram, Register, TypeHandle, VariantHandle,
};
use vela_def::{TypeId, VariantId};

pub(crate) fn dispatch_get_record_field(
    frame: &mut CallFrame,
    heap: Option<&mut HeapExecution<'_>>,
    dst: Register,
    record: Register,
    field: &str,
) -> VmResult<()> {
    let value = get_record_field_value(&frame.read(record)?, field, heap.as_deref())?;
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
    let value = get_record_slot_value(&frame.read(record)?, field, slot, heap.as_deref())?;
    frame.write(dst, value)
}

pub(crate) fn dispatch_linked_get_record_slot(
    frame: &mut CallFrame,
    heap: Option<&mut HeapExecution<'_>>,
    program: &LinkedProgram,
    read: LinkedRecordSlotRead,
    inline_caches: Option<&dyn VmInlineCaches>,
    cache_site: Option<CacheSiteId>,
) -> VmResult<()> {
    let field_name = program.debug_name(read.debug_name);
    let record_value = frame.read(read.record)?;
    let value = get_linked_record_slot_value(
        &record_value,
        field_name,
        read.field,
        heap.as_deref(),
        inline_caches,
        cache_site,
    )?;
    frame.write(read.dst, value)
}

pub(crate) fn dispatch_set_record_field(
    frame: &mut CallFrame,
    heap: Option<&mut HeapExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
    record: Register,
    field: &str,
    src: Register,
) -> VmResult<()> {
    let mut record_value = frame.read(record)?;
    let src = frame.read(src)?;
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
    let mut record_value = frame.read(record)?;
    let src = frame.read(src)?;
    record_fields::set_record_slot_value(&mut record_value, field, slot, &src, heap, budget)?;
    frame.write(record, record_value)
}

pub(crate) struct LinkedRecordSlotRead {
    pub(crate) dst: Register,
    pub(crate) record: Register,
    pub(crate) field: FieldSlot,
    pub(crate) debug_name: DebugNameId,
}

pub(crate) struct LinkedRecordSlotWrite {
    pub(crate) record: Register,
    pub(crate) field: FieldSlot,
    pub(crate) debug_name: DebugNameId,
    pub(crate) src: Register,
}

pub(crate) fn dispatch_linked_set_record_slot(
    frame: &mut CallFrame,
    heap: Option<&mut HeapExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
    program: &LinkedProgram,
    write: LinkedRecordSlotWrite,
    inline_caches: Option<&dyn VmInlineCaches>,
    cache_site: Option<CacheSiteId>,
) -> VmResult<()> {
    let mut heap = heap;
    let mut budget = budget;
    let mut record_value = frame.read(write.record)?;
    let src = frame.read(write.src)?;
    let field_name = program.debug_name(write.debug_name);
    if !set_linked_record_slot_value(
        &mut record_value,
        field_name,
        &src,
        heap.as_deref_mut(),
        budget.as_deref_mut(),
        inline_caches,
        cache_site,
    )? {
        record_fields::set_record_slot_value(
            &mut record_value,
            field_name,
            write.field.index(),
            &src,
            heap.as_deref_mut(),
            budget,
        )?;
        populate_record_field_cache(
            &record_value,
            field_name,
            write.field,
            heap.as_deref(),
            inline_caches,
            cache_site,
        );
    }
    frame.write(write.record, record_value)
}

pub(crate) fn dispatch_get_enum_field(
    frame: &mut CallFrame,
    heap: Option<&mut HeapExecution<'_>>,
    dst: Register,
    value: Register,
    field: &str,
) -> VmResult<()> {
    let value = get_enum_field_value(&frame.read(value)?, field, heap.as_deref())?;
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
    let value = get_enum_slot_value(&frame.read(value)?, field, slot, heap.as_deref())?;
    frame.write(dst, value)
}

pub(crate) fn dispatch_linked_get_enum_slot(
    frame: &mut CallFrame,
    heap: Option<&mut HeapExecution<'_>>,
    program: &LinkedProgram,
    dst: Register,
    value: Register,
    field: FieldSlot,
    debug_name: DebugNameId,
) -> VmResult<()> {
    dispatch_get_enum_slot(
        frame,
        heap,
        dst,
        value,
        program.debug_name(debug_name),
        field.index(),
    )
}

pub(crate) fn dispatch_enum_tag_equal(
    frame: &mut CallFrame,
    heap: Option<&HeapExecution<'_>>,
    dst: Register,
    value: Register,
    enum_name: &str,
    variant: &str,
) -> VmResult<()> {
    let matches = enum_tag_equal(&frame.read(value)?, enum_name, variant, heap);
    frame.write(dst, Value::Bool(matches))
}

pub(crate) fn dispatch_linked_enum_tag_equal(
    frame: &mut CallFrame,
    heap: Option<&HeapExecution<'_>>,
    program: &LinkedProgram,
    dst: Register,
    value: Register,
    enum_ty: TypeHandle,
    variant: VariantHandle,
) -> VmResult<()> {
    let enum_ty = program.ty(enum_ty).ok_or_else(|| {
        VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
            opcode: "EnumTagEqual",
        })
    })?;
    let variant = program.variant(variant).ok_or_else(|| {
        VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
            opcode: "EnumTagEqual",
        })
    })?;
    let matches = enum_tag_id_equal(&frame.read(value)?, enum_ty.id, variant.id, heap);
    frame.write(dst, Value::Bool(matches))
}

pub(crate) fn get_record_field_value(
    value: &Value,
    field: &str,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    match value {
        Value::HeapRef(reference) => {
            let Some(HeapValue::Record {
                type_name, fields, ..
            }) = heap.and_then(|heap| heap.heap.get(*reference))
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
            let Some(HeapValue::Record {
                type_name, fields, ..
            }) = heap.and_then(|heap| heap.heap.get(*reference))
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

fn get_linked_record_slot_value(
    value: &Value,
    field_name: &str,
    field: FieldSlot,
    heap: Option<&HeapExecution<'_>>,
    inline_caches: Option<&dyn VmInlineCaches>,
    cache_site: Option<CacheSiteId>,
) -> VmResult<Value> {
    if let Some(value) = cached_record_slot_value(value, field, heap, inline_caches, cache_site) {
        return Ok(value);
    }
    let result = get_record_slot_value(value, field_name, field.index(), heap)?;
    populate_record_field_cache(value, field_name, field, heap, inline_caches, cache_site);
    Ok(result)
}

fn set_linked_record_slot_value(
    value: &mut Value,
    field_name: &str,
    src: &Value,
    heap: Option<&mut HeapExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
    inline_caches: Option<&dyn VmInlineCaches>,
    cache_site: Option<CacheSiteId>,
) -> VmResult<bool> {
    let Some(cache_site) = cache_site else {
        return Ok(false);
    };
    let Some(inline_caches) = inline_caches else {
        return Ok(false);
    };
    let Some(entry) = inline_caches.record_field(cache_site) else {
        return Ok(false);
    };
    let Some(heap) = heap else {
        return Ok(false);
    };
    let Value::HeapRef(reference) = value else {
        return Ok(false);
    };
    let Some((type_name, true)) = heap
        .heap
        .get(*reference)
        .map(|heap_value| match heap_value {
            HeapValue::Record {
                type_name,
                identity: Some(identity),
                fields,
            } => (
                type_name.clone(),
                identity.type_id == entry.type_id
                    && identity.shape_id == entry.shape_id
                    && fields.get_slot(entry.field.index(), field_name).is_some(),
            ),
            _ => (String::new(), false),
        })
    else {
        return Ok(false);
    };
    let stored_value = crate::store_runtime_value(src, heap, budget)?;
    let HeapValue::Record { fields, .. } = heap.heap.get_mut(*reference).map_err(|_| {
        VmError::new(VmErrorKind::UnknownRecordField {
            type_name: type_name.clone(),
            field: field_name.to_owned(),
        })
    })?
    else {
        return type_error("record slot assignment");
    };
    fields
        .set_slot_existing(entry.field.index(), field_name, stored_value)
        .map_err(|_| {
            VmError::new(VmErrorKind::UnknownRecordField {
                type_name,
                field: field_name.to_owned(),
            })
        })?;
    Ok(true)
}

fn cached_record_slot_value(
    value: &Value,
    field: FieldSlot,
    heap: Option<&HeapExecution<'_>>,
    inline_caches: Option<&dyn VmInlineCaches>,
    cache_site: Option<CacheSiteId>,
) -> Option<Value> {
    let entry = inline_caches?.record_field(cache_site?)?;
    let Value::HeapRef(reference) = value else {
        return None;
    };
    let HeapValue::Record {
        identity: Some(identity),
        fields,
        ..
    } = heap?.heap.get(*reference)?
    else {
        return None;
    };
    (identity.type_id == entry.type_id
        && identity.shape_id == entry.shape_id
        && field == entry.field)
        .then(|| {
            fields
                .get_slot_at(entry.field.index())
                .map(stored_runtime_value)
        })?
}

fn populate_record_field_cache(
    value: &Value,
    field_name: &str,
    field: FieldSlot,
    heap: Option<&HeapExecution<'_>>,
    inline_caches: Option<&dyn VmInlineCaches>,
    cache_site: Option<CacheSiteId>,
) {
    let Some(cache_site) = cache_site else {
        return;
    };
    let Some(inline_caches) = inline_caches else {
        return;
    };
    let Some(entry) = record_field_cache_entry(value, field_name, field, heap) else {
        return;
    };
    inline_caches.set_record_field(cache_site, entry);
}

fn record_field_cache_entry(
    value: &Value,
    field_name: &str,
    field: FieldSlot,
    heap: Option<&HeapExecution<'_>>,
) -> Option<RecordFieldInlineCacheEntry> {
    let Value::HeapRef(reference) = value else {
        return None;
    };
    let HeapValue::Record {
        identity: Some(identity),
        fields,
        ..
    } = heap?.heap.get(*reference)?
    else {
        return None;
    };
    fields.get_slot(field.index(), field_name)?;
    Some(RecordFieldInlineCacheEntry {
        type_id: identity.type_id,
        shape_id: identity.shape_id,
        field,
    })
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
