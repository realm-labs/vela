use crate::heap::{EnumIdentity, HeapValue, RecordIdentity};
use crate::option_result::std_enum_identity_for_names;
use crate::script_object::ScriptFields;
use crate::{
    CallFrame, ExecutionBudget, HeapExecution, Value, VmError, VmErrorKind, VmResult,
    allocate_heap_value, enum_variant_owner, store_runtime_value,
};
use vela_bytecode::{
    DebugNameId, FieldSlot, LinkedProgram, LinkedType, LinkedVariant, Register, TypeHandle,
    VariantHandle,
};

pub(crate) struct EnumConstruction<'a> {
    pub(crate) enum_name: &'a str,
    pub(crate) variant: &'a str,
    pub(crate) identity: Option<EnumIdentity>,
    pub(crate) fields: &'a [(String, Register)],
}

pub(crate) struct RecordConstruction<'a> {
    pub(crate) type_name: &'a str,
    pub(crate) type_id: Option<vela_def::TypeId>,
    pub(crate) fields: &'a [(String, Register)],
}

pub(crate) struct LinkedEnumConstruction<'a> {
    pub(crate) enum_ty: TypeHandle,
    pub(crate) variant: VariantHandle,
    pub(crate) fields: &'a [(FieldSlot, DebugNameId, Register)],
}

pub(crate) fn make_record_with_identity(
    frame: &mut CallFrame,
    heap: Option<&mut HeapExecution<'_>>,
    mut budget: Option<&mut ExecutionBudget>,
    dst: Register,
    construction: RecordConstruction<'_>,
) -> VmResult<()> {
    let RecordConstruction {
        type_name,
        type_id,
        fields,
    } = construction;
    let Some(heap) = heap else {
        return Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "record heap",
        }));
    };
    let slots =
        runtime_fields_from_registers(type_name, frame, fields, heap, budget_ref(&mut budget))?;
    let identity = type_id.map(|type_id| RecordIdentity::new(type_id, slots.shape_id()));
    let value = allocate_heap_value(
        HeapValue::Record {
            identity,
            fields: slots,
        },
        heap,
        budget_ref(&mut budget),
    )?;
    frame.write(dst, value)
}

pub(crate) fn make_linked_record(
    frame: &mut CallFrame,
    heap: Option<&mut HeapExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
    dst: Register,
    program: &LinkedProgram,
    ty: TypeHandle,
    fields: &[(FieldSlot, DebugNameId, Register)],
) -> VmResult<()> {
    let linked_ty = linked_type(program, ty, "MakeRecord")?;
    let type_name = program.debug_name(linked_ty.debug_name);
    let fields = linked_object_fields(program, fields);
    make_record_with_identity(
        frame,
        heap,
        budget,
        dst,
        RecordConstruction {
            type_name,
            type_id: Some(linked_ty.id),
            fields: &fields,
        },
    )
}

pub(crate) fn make_enum_with_identity(
    frame: &mut CallFrame,
    heap: Option<&mut HeapExecution<'_>>,
    mut budget: Option<&mut ExecutionBudget>,
    dst: Register,
    construction: EnumConstruction<'_>,
) -> VmResult<()> {
    let EnumConstruction {
        enum_name,
        variant,
        identity,
        fields,
    } = construction;
    let owner = enum_variant_owner(enum_name, variant);
    let Some(heap) = heap else {
        return Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "enum heap",
        }));
    };
    let slots =
        runtime_fields_from_registers(&owner, frame, fields, heap, budget_ref(&mut budget))?;
    let value = allocate_heap_value(
        HeapValue::Enum {
            enum_name: enum_name.to_owned(),
            variant: variant.to_owned(),
            identity,
            fields: slots,
        },
        heap,
        budget_ref(&mut budget),
    )?;
    frame.write(dst, value)
}

pub(crate) fn make_linked_enum(
    frame: &mut CallFrame,
    heap: Option<&mut HeapExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
    dst: Register,
    program: &LinkedProgram,
    construction: LinkedEnumConstruction<'_>,
) -> VmResult<()> {
    let LinkedEnumConstruction {
        enum_ty,
        variant,
        fields,
    } = construction;
    let enum_ty = linked_type(program, enum_ty, "MakeEnum")?;
    let variant = linked_variant(program, variant, "MakeEnum")?;
    let enum_name = program.debug_name(enum_ty.debug_name);
    let variant_name = linked_variant_short_name(program, variant);
    let identity = std_enum_identity_for_names(enum_name, variant_name)
        .unwrap_or_else(|| linked_enum_identity(enum_ty, variant));
    let fields = linked_object_fields(program, fields);
    make_enum_with_identity(
        frame,
        heap,
        budget,
        dst,
        EnumConstruction {
            enum_name,
            variant: variant_name,
            identity: Some(identity),
            fields: &fields,
        },
    )
}

#[inline]
fn budget_ref<'a>(budget: &'a mut Option<&mut ExecutionBudget>) -> Option<&'a mut ExecutionBudget> {
    match budget {
        Some(budget) => Some(&mut **budget),
        None => None,
    }
}

fn runtime_fields_from_registers(
    owner: &str,
    frame: &CallFrame,
    fields: &[(String, Register)],
    heap: &mut HeapExecution<'_>,
    mut budget: Option<&mut ExecutionBudget>,
) -> VmResult<ScriptFields<Value>> {
    // Sort name/register pairs into storage order first, then read the values
    // and intern the shape, so a repeat construction of a known shape clones
    // no field names and allocates only its value vector.
    let mut ordered: Vec<(&str, Register)> = fields
        .iter()
        .map(|(name, register)| (name.as_str(), *register))
        .collect();
    ordered.sort_by(|left, right| left.0.cmp(right.0));
    let duplicate = ordered.windows(2).any(|pair| pair[0].0 == pair[1].0);
    if duplicate {
        let pairs = fields
            .iter()
            .map(|(name, register)| {
                Ok((
                    name.clone(),
                    store_runtime_value(&frame.read(*register)?, heap, budget.as_deref_mut())?,
                ))
            })
            .collect::<VmResult<Vec<_>>>()?;
        return Ok(ScriptFields::from_pairs(owner, pairs));
    }
    let mut values = Vec::with_capacity(ordered.len());
    for (_, register) in &ordered {
        values.push(store_runtime_value(
            &frame.read(*register)?,
            heap,
            budget.as_deref_mut(),
        )?);
    }
    let names: Vec<&str> = ordered.iter().map(|(name, _)| *name).collect();
    let shape = heap.heap.shapes_mut().intern(owner, &names);
    Ok(ScriptFields::from_shape(shape, values))
}
fn linked_type<'program>(
    program: &'program LinkedProgram,
    ty: TypeHandle,
    opcode: &'static str,
) -> VmResult<&'program LinkedType> {
    program
        .ty(ty)
        .ok_or_else(|| VmError::new(VmErrorKind::UnsupportedLinkedInstruction { opcode }))
}

fn linked_variant<'program>(
    program: &'program LinkedProgram,
    variant: VariantHandle,
    opcode: &'static str,
) -> VmResult<&'program LinkedVariant> {
    program
        .variant(variant)
        .ok_or_else(|| VmError::new(VmErrorKind::UnsupportedLinkedInstruction { opcode }))
}

fn linked_variant_short_name<'program>(
    program: &'program LinkedProgram,
    variant: &LinkedVariant,
) -> &'program str {
    program
        .debug_name(variant.debug_name)
        .rsplit_once("::")
        .map_or_else(|| program.debug_name(variant.debug_name), |(_, name)| name)
}

fn linked_enum_identity(enum_ty: &LinkedType, variant: &LinkedVariant) -> EnumIdentity {
    EnumIdentity::new(enum_ty.id, variant.id, None)
}

fn linked_object_fields(
    program: &LinkedProgram,
    fields: &[(FieldSlot, DebugNameId, Register)],
) -> Vec<(String, Register)> {
    fields
        .iter()
        .map(|(_, debug_name, register)| (program.debug_name(*debug_name).to_owned(), *register))
        .collect()
}
