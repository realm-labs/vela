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

pub(crate) fn make_record(
    frame: &mut CallFrame,
    heap: Option<&mut HeapExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
    dst: Register,
    type_name: &str,
    fields: &[(String, Register)],
) -> VmResult<()> {
    make_record_with_identity(
        frame,
        heap,
        budget,
        dst,
        RecordConstruction {
            type_name,
            type_id: None,
            fields,
        },
    )
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
            type_name: type_name.to_owned(),
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

pub(crate) fn make_enum(
    frame: &mut CallFrame,
    heap: Option<&mut HeapExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
    dst: Register,
    enum_name: &str,
    variant: &str,
    fields: &[(String, Register)],
) -> VmResult<()> {
    make_enum_with_identity(
        frame,
        heap,
        budget,
        dst,
        EnumConstruction {
            enum_name,
            variant,
            identity: std_enum_identity_for_names(enum_name, variant),
            fields,
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
    match fields {
        [] => Ok(ScriptFields::empty(owner)),
        [(name, register)] => {
            let value = store_runtime_value(&frame.read(*register)?, heap, budget.as_deref_mut())?;
            Ok(ScriptFields::single(owner, name.clone(), value))
        }
        [(first_name, first_register), (second_name, second_register)] => {
            let first_value =
                store_runtime_value(&frame.read(*first_register)?, heap, budget.as_deref_mut())?;
            let second_value =
                store_runtime_value(&frame.read(*second_register)?, heap, budget.as_deref_mut())?;
            Ok(ScriptFields::two(
                owner,
                first_name.clone(),
                first_value,
                second_name.clone(),
                second_value,
            ))
        }
        [
            (first_name, first_register),
            (second_name, second_register),
            (third_name, third_register),
        ] => {
            let first_value =
                store_runtime_value(&frame.read(*first_register)?, heap, budget.as_deref_mut())?;
            let second_value =
                store_runtime_value(&frame.read(*second_register)?, heap, budget.as_deref_mut())?;
            let third_value =
                store_runtime_value(&frame.read(*third_register)?, heap, budget.as_deref_mut())?;
            Ok(ScriptFields::three(
                owner,
                first_name.clone(),
                first_value,
                second_name.clone(),
                second_value,
                third_name.clone(),
                third_value,
            ))
        }
        [
            (first_name, first_register),
            (second_name, second_register),
            (third_name, third_register),
            (fourth_name, fourth_register),
        ] => {
            let first_value =
                store_runtime_value(&frame.read(*first_register)?, heap, budget.as_deref_mut())?;
            let second_value =
                store_runtime_value(&frame.read(*second_register)?, heap, budget.as_deref_mut())?;
            let third_value =
                store_runtime_value(&frame.read(*third_register)?, heap, budget.as_deref_mut())?;
            let fourth_value =
                store_runtime_value(&frame.read(*fourth_register)?, heap, budget.as_deref_mut())?;
            Ok(ScriptFields::four(
                owner,
                [
                    (first_name.clone(), first_value),
                    (second_name.clone(), second_value),
                    (third_name.clone(), third_value),
                    (fourth_name.clone(), fourth_value),
                ],
            ))
        }
        [
            (first_name, first_register),
            (second_name, second_register),
            (third_name, third_register),
            (fourth_name, fourth_register),
            (fifth_name, fifth_register),
        ] => {
            let first_value =
                store_runtime_value(&frame.read(*first_register)?, heap, budget.as_deref_mut())?;
            let second_value =
                store_runtime_value(&frame.read(*second_register)?, heap, budget.as_deref_mut())?;
            let third_value =
                store_runtime_value(&frame.read(*third_register)?, heap, budget.as_deref_mut())?;
            let fourth_value =
                store_runtime_value(&frame.read(*fourth_register)?, heap, budget.as_deref_mut())?;
            let fifth_value =
                store_runtime_value(&frame.read(*fifth_register)?, heap, budget.as_deref_mut())?;
            Ok(ScriptFields::five(
                owner,
                [
                    (first_name.clone(), first_value),
                    (second_name.clone(), second_value),
                    (third_name.clone(), third_value),
                    (fourth_name.clone(), fourth_value),
                    (fifth_name.clone(), fifth_value),
                ],
            ))
        }
        [
            (first_name, first_register),
            (second_name, second_register),
            (third_name, third_register),
            (fourth_name, fourth_register),
            (fifth_name, fifth_register),
            (sixth_name, sixth_register),
        ] => {
            let first_value =
                store_runtime_value(&frame.read(*first_register)?, heap, budget.as_deref_mut())?;
            let second_value =
                store_runtime_value(&frame.read(*second_register)?, heap, budget.as_deref_mut())?;
            let third_value =
                store_runtime_value(&frame.read(*third_register)?, heap, budget.as_deref_mut())?;
            let fourth_value =
                store_runtime_value(&frame.read(*fourth_register)?, heap, budget.as_deref_mut())?;
            let fifth_value =
                store_runtime_value(&frame.read(*fifth_register)?, heap, budget.as_deref_mut())?;
            let sixth_value =
                store_runtime_value(&frame.read(*sixth_register)?, heap, budget.as_deref_mut())?;
            Ok(ScriptFields::six(
                owner,
                [
                    (first_name.clone(), first_value),
                    (second_name.clone(), second_value),
                    (third_name.clone(), third_value),
                    (fourth_name.clone(), fourth_value),
                    (fifth_name.clone(), fifth_value),
                    (sixth_name.clone(), sixth_value),
                ],
            ))
        }
        _ => fields
            .iter()
            .map(|(name, register)| {
                Ok((
                    name.clone(),
                    store_runtime_value(&frame.read(*register)?, heap, budget.as_deref_mut())?,
                ))
            })
            .collect::<VmResult<Vec<_>>>()
            .map(|fields| ScriptFields::from_pairs(owner, fields)),
    }
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
