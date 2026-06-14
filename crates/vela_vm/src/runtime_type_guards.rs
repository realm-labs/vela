use vela_bytecode::{
    GuardKind, LinkedCodeObject, LinkedProgram, Register, StandardTypeGuard, TypeGuard,
    TypeGuardPlan, TypeGuardPlanId, UnlinkedTypeGuard, UnlinkedTypeGuardPlan,
};
use vela_common::PrimitiveTag;

use crate::heap::HeapValue;
use crate::option_result::{StdEnumKind, StdEnumVariant, std_enum_tag};
use crate::stored_runtime_value;
use crate::{CallFrame, HeapExecution, Value, VmError, VmErrorKind, VmResult};

pub(crate) fn execute_unlinked_guard(
    value: &Value,
    guard: &UnlinkedTypeGuard,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<()> {
    // The interpreter is the generic fallback path for specialization misses.
    if guard.context.kind == GuardKind::Specialization {
        return Ok(());
    }

    match guard.plan {
        UnlinkedTypeGuardPlan::Primitive(expected) => {
            execute_primitive_guard(value, expected, heap, &guard.context.debug_name)
        }
        UnlinkedTypeGuardPlan::Standard(expected) => {
            execute_standard_guard(value, expected, heap, &guard.context.debug_name)
        }
        UnlinkedTypeGuardPlan::Array { ref element } => {
            execute_array_guard(value, element.as_deref(), heap, &guard.context.debug_name)
        }
        UnlinkedTypeGuardPlan::Map {
            ref key,
            value: ref value_plan,
        } => execute_map_guard(
            value,
            key.as_deref(),
            value_plan.as_deref(),
            heap,
            &guard.context.debug_name,
        ),
        UnlinkedTypeGuardPlan::Set { ref element } => {
            execute_set_guard(value, element.as_deref(), heap, &guard.context.debug_name)
        }
        UnlinkedTypeGuardPlan::Iterator { .. } => execute_standard_guard(
            value,
            StandardTypeGuard::Iterator,
            heap,
            &guard.context.debug_name,
        ),
        UnlinkedTypeGuardPlan::Option { ref some } => {
            execute_option_guard(value, some.as_deref(), heap, &guard.context.debug_name)
        }
        UnlinkedTypeGuardPlan::Result { ref ok, ref err } => execute_result_guard(
            value,
            ok.as_deref(),
            err.as_deref(),
            heap,
            &guard.context.debug_name,
        ),
        UnlinkedTypeGuardPlan::Type(ref expected) => {
            execute_unlinked_type_guard(value, expected, heap, &guard.context.debug_name)
        }
        UnlinkedTypeGuardPlan::Variant {
            ref enum_name,
            ref variant,
        } => execute_unlinked_variant_guard(
            value,
            enum_name,
            variant,
            heap,
            &guard.context.debug_name,
        ),
        UnlinkedTypeGuardPlan::Shape {
            ref type_name,
            shape_id,
        } => execute_unlinked_shape_guard(
            value,
            type_name,
            shape_id,
            heap,
            &guard.context.debug_name,
        ),
        UnlinkedTypeGuardPlan::HostType(_) => Ok(()),
    }
}

pub(crate) fn execute_linked_guard(
    value: &Value,
    guard: &TypeGuard,
    program: &LinkedProgram,
    heap: Option<&HeapExecution<'_>>,
    debug_name: &str,
) -> VmResult<()> {
    // The interpreter is the generic fallback path for specialization misses.
    if guard.context.kind == GuardKind::Specialization {
        return Ok(());
    }

    match guard.plan {
        TypeGuardPlan::Primitive(expected) => {
            execute_primitive_guard(value, expected, heap, debug_name)
        }
        TypeGuardPlan::Standard(expected) => {
            execute_standard_guard(value, expected, heap, debug_name)
        }
        TypeGuardPlan::Array { ref element } => {
            execute_linked_array_guard(value, element.as_deref(), program, heap, debug_name)
        }
        TypeGuardPlan::Map {
            ref key,
            value: ref value_plan,
        } => execute_linked_map_guard(
            value,
            key.as_deref(),
            value_plan.as_deref(),
            program,
            heap,
            debug_name,
        ),
        TypeGuardPlan::Set { ref element } => {
            execute_linked_set_guard(value, element.as_deref(), program, heap, debug_name)
        }
        TypeGuardPlan::Iterator { .. } => {
            execute_standard_guard(value, StandardTypeGuard::Iterator, heap, debug_name)
        }
        TypeGuardPlan::Option { ref some } => {
            execute_linked_option_guard(value, some.as_deref(), program, heap, debug_name)
        }
        TypeGuardPlan::Result { ref ok, ref err } => execute_linked_result_guard(
            value,
            ok.as_deref(),
            err.as_deref(),
            program,
            heap,
            debug_name,
        ),
        TypeGuardPlan::Type(expected) => {
            let expected = program.ty(expected).ok_or_else(|| {
                VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
                    opcode: "type_guard",
                })
            })?;
            execute_type_id_guard(
                value,
                expected.id,
                program.debug_name(expected.debug_name),
                heap,
                debug_name,
            )
        }
        TypeGuardPlan::Variant(expected) => {
            let expected = program.variant(expected).ok_or_else(|| {
                VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
                    opcode: "variant_guard",
                })
            })?;
            execute_variant_id_guard(
                value,
                expected.id,
                program.debug_name(expected.debug_name),
                heap,
                debug_name,
            )
        }
        TypeGuardPlan::Shape { ty, shape_id } => {
            let expected = program.ty(ty).ok_or_else(|| {
                VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
                    opcode: "shape_guard",
                })
            })?;
            execute_shape_id_guard(
                value,
                expected.id,
                shape_id,
                program.debug_name(expected.debug_name),
                heap,
                debug_name,
            )
        }
        TypeGuardPlan::HostType(_) => Ok(()),
    }
}

pub(crate) fn execute_linked_param_guards(
    code: &LinkedCodeObject,
    program: &LinkedProgram,
    frame: &CallFrame,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<()> {
    let param_offset = usize::from(code.capture_count);
    for param_guard in &code.param_guards {
        let register = Register(
            code.capture_count
                .checked_add(param_guard.parameter)
                .ok_or_else(|| {
                    VmError::new(VmErrorKind::RegisterOutOfBounds {
                        register: Register(u16::MAX),
                    })
                })?,
        );
        let value = frame.read(register)?;
        if matches!(value, Value::Missing) {
            continue;
        }
        let guard = code.type_guard(param_guard.guard).ok_or_else(|| {
            VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
                opcode: "param_guard",
            })
        })?;
        execute_linked_guard(
            &value,
            guard,
            program,
            heap,
            program.debug_name(guard.context.debug_name),
        )?;
        debug_assert!(usize::from(param_guard.parameter) < code.params.len());
        debug_assert!(usize::from(register.0) >= param_offset);
    }
    Ok(())
}

pub(crate) fn execute_linked_register_guard(
    code: &LinkedCodeObject,
    program: &LinkedProgram,
    frame: &CallFrame,
    register: Register,
    guard_id: TypeGuardPlanId,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<()> {
    let value = frame.read(register)?;
    let guard = code.type_guard(guard_id).ok_or_else(|| {
        VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
            opcode: "GuardType",
        })
    })?;
    execute_linked_guard(
        &value,
        guard,
        program,
        heap,
        program.debug_name(guard.context.debug_name),
    )
}

pub(crate) fn execute_linked_return_guard(
    code: &LinkedCodeObject,
    program: &LinkedProgram,
    value: Value,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    let Some(guard_id) = code.return_guard else {
        return Ok(value);
    };
    let guard = code.type_guard(guard_id).ok_or_else(|| {
        VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
            opcode: "return_guard",
        })
    })?;
    execute_linked_guard(
        &value,
        guard,
        program,
        heap,
        program.debug_name(guard.context.debug_name),
    )?;
    Ok(value)
}

fn execute_primitive_guard(
    value: &Value,
    expected: PrimitiveTag,
    heap: Option<&HeapExecution<'_>>,
    debug_name: &str,
) -> VmResult<()> {
    if runtime_primitive_tag(value, heap) == Some(expected) {
        return Ok(());
    }
    Err(VmError::new(VmErrorKind::TypeContractViolation {
        expected: primitive_type_name(expected).to_owned(),
        actual: runtime_type_name(value, heap).to_owned(),
        debug_name: debug_name.to_owned(),
    }))
}

fn execute_standard_guard(
    value: &Value,
    expected: StandardTypeGuard,
    heap: Option<&HeapExecution<'_>>,
    debug_name: &str,
) -> VmResult<()> {
    if runtime_standard_type(value, heap) == Some(expected) {
        return Ok(());
    }
    Err(type_contract_error(
        value,
        standard_type_name(expected),
        heap,
        debug_name,
    ))
}

fn execute_option_guard(
    value: &Value,
    some: Option<&UnlinkedTypeGuardPlan>,
    heap: Option<&HeapExecution<'_>>,
    debug_name: &str,
) -> VmResult<()> {
    match std_enum_value(value, heap) {
        Some((StdEnumKind::Option, StdEnumVariant::Some, fields)) => {
            if let Some(some) = some {
                let payload = fields
                    .get_slot(0, "0")
                    .map(stored_runtime_value)
                    .ok_or_else(|| {
                        VmError::new(VmErrorKind::TypeMismatch {
                            operation: "Option payload contract",
                        })
                    })?;
                execute_unlinked_guard_plan(&payload, some, heap, debug_name)?;
            }
            Ok(())
        }
        Some((StdEnumKind::Option, StdEnumVariant::None, _)) => Ok(()),
        _ => Err(type_contract_error(value, "Option", heap, debug_name)),
    }
}

fn execute_array_guard(
    value: &Value,
    element: Option<&UnlinkedTypeGuardPlan>,
    heap: Option<&HeapExecution<'_>>,
    debug_name: &str,
) -> VmResult<()> {
    let Value::HeapRef(reference) = value else {
        return Err(type_contract_error(value, "Array", heap, debug_name));
    };
    let Some(HeapValue::Array(values)) = heap.and_then(|heap| heap.heap.get(*reference)) else {
        return Err(type_contract_error(value, "Array", heap, debug_name));
    };
    if let Some(element) = element {
        for value in values {
            execute_unlinked_guard_plan(value, element, heap, debug_name)?;
        }
    }
    Ok(())
}

fn execute_set_guard(
    value: &Value,
    element: Option<&UnlinkedTypeGuardPlan>,
    heap: Option<&HeapExecution<'_>>,
    debug_name: &str,
) -> VmResult<()> {
    let Value::HeapRef(reference) = value else {
        return Err(type_contract_error(value, "Set", heap, debug_name));
    };
    let Some(HeapValue::Set(values)) = heap.and_then(|heap| heap.heap.get(*reference)) else {
        return Err(type_contract_error(value, "Set", heap, debug_name));
    };
    if let Some(element) = element {
        for value in values {
            execute_unlinked_guard_plan(value, element, heap, debug_name)?;
        }
    }
    Ok(())
}

fn execute_map_guard(
    value: &Value,
    key: Option<&UnlinkedTypeGuardPlan>,
    value_plan: Option<&UnlinkedTypeGuardPlan>,
    heap: Option<&HeapExecution<'_>>,
    debug_name: &str,
) -> VmResult<()> {
    let Value::HeapRef(reference) = value else {
        return Err(type_contract_error(value, "Map", heap, debug_name));
    };
    let Some(HeapValue::Map(values)) = heap.and_then(|heap| heap.heap.get(*reference)) else {
        return Err(type_contract_error(value, "Map", heap, debug_name));
    };
    if !map_key_plan_is_string_or_erased(key) {
        return Err(VmError::new(VmErrorKind::TypeContractViolation {
            expected: "Map<String, _>".to_owned(),
            actual: "Map".to_owned(),
            debug_name: debug_name.to_owned(),
        }));
    }
    if let Some(value_plan) = value_plan {
        for value in values.values() {
            execute_unlinked_guard_plan(value, value_plan, heap, debug_name)?;
        }
    }
    Ok(())
}

fn execute_result_guard(
    value: &Value,
    ok: Option<&UnlinkedTypeGuardPlan>,
    err: Option<&UnlinkedTypeGuardPlan>,
    heap: Option<&HeapExecution<'_>>,
    debug_name: &str,
) -> VmResult<()> {
    match std_enum_value(value, heap) {
        Some((StdEnumKind::Result, StdEnumVariant::Ok, fields)) => {
            if let Some(ok) = ok {
                let payload = fields
                    .get_slot(0, "0")
                    .map(stored_runtime_value)
                    .ok_or_else(|| {
                        VmError::new(VmErrorKind::TypeMismatch {
                            operation: "Result Ok payload contract",
                        })
                    })?;
                execute_unlinked_guard_plan(&payload, ok, heap, debug_name)?;
            }
            Ok(())
        }
        Some((StdEnumKind::Result, StdEnumVariant::Err, fields)) => {
            if let Some(err) = err {
                let payload = fields
                    .get_slot(0, "0")
                    .map(stored_runtime_value)
                    .ok_or_else(|| {
                        VmError::new(VmErrorKind::TypeMismatch {
                            operation: "Result Err payload contract",
                        })
                    })?;
                execute_unlinked_guard_plan(&payload, err, heap, debug_name)?;
            }
            Ok(())
        }
        _ => Err(type_contract_error(value, "Result", heap, debug_name)),
    }
}

fn execute_unlinked_guard_plan(
    value: &Value,
    plan: &UnlinkedTypeGuardPlan,
    heap: Option<&HeapExecution<'_>>,
    debug_name: &str,
) -> VmResult<()> {
    match plan {
        UnlinkedTypeGuardPlan::Primitive(expected) => {
            execute_primitive_guard(value, *expected, heap, debug_name)
        }
        UnlinkedTypeGuardPlan::Standard(expected) => {
            execute_standard_guard(value, *expected, heap, debug_name)
        }
        UnlinkedTypeGuardPlan::Array { element } => {
            execute_array_guard(value, element.as_deref(), heap, debug_name)
        }
        UnlinkedTypeGuardPlan::Map { key, value: values } => {
            execute_map_guard(value, key.as_deref(), values.as_deref(), heap, debug_name)
        }
        UnlinkedTypeGuardPlan::Set { element } => {
            execute_set_guard(value, element.as_deref(), heap, debug_name)
        }
        UnlinkedTypeGuardPlan::Iterator { .. } => {
            execute_standard_guard(value, StandardTypeGuard::Iterator, heap, debug_name)
        }
        UnlinkedTypeGuardPlan::Option { some } => {
            execute_option_guard(value, some.as_deref(), heap, debug_name)
        }
        UnlinkedTypeGuardPlan::Result { ok, err } => {
            execute_result_guard(value, ok.as_deref(), err.as_deref(), heap, debug_name)
        }
        UnlinkedTypeGuardPlan::Type(expected) => {
            execute_unlinked_type_guard(value, expected, heap, debug_name)
        }
        UnlinkedTypeGuardPlan::Variant { enum_name, variant } => {
            execute_unlinked_variant_guard(value, enum_name, variant, heap, debug_name)
        }
        UnlinkedTypeGuardPlan::Shape {
            type_name,
            shape_id,
        } => execute_unlinked_shape_guard(value, type_name, *shape_id, heap, debug_name),
        UnlinkedTypeGuardPlan::HostType(_) => Ok(()),
    }
}

fn execute_linked_option_guard(
    value: &Value,
    some: Option<&TypeGuardPlan>,
    program: &LinkedProgram,
    heap: Option<&HeapExecution<'_>>,
    debug_name: &str,
) -> VmResult<()> {
    match std_enum_value(value, heap) {
        Some((StdEnumKind::Option, StdEnumVariant::Some, fields)) => {
            if let Some(some) = some {
                let payload = fields
                    .get_slot(0, "0")
                    .map(stored_runtime_value)
                    .ok_or_else(|| {
                        VmError::new(VmErrorKind::TypeMismatch {
                            operation: "Option payload contract",
                        })
                    })?;
                execute_linked_guard_plan(&payload, some, program, heap, debug_name)?;
            }
            Ok(())
        }
        Some((StdEnumKind::Option, StdEnumVariant::None, _)) => Ok(()),
        _ => Err(type_contract_error(value, "Option", heap, debug_name)),
    }
}

fn execute_linked_array_guard(
    value: &Value,
    element: Option<&TypeGuardPlan>,
    program: &LinkedProgram,
    heap: Option<&HeapExecution<'_>>,
    debug_name: &str,
) -> VmResult<()> {
    let Value::HeapRef(reference) = value else {
        return Err(type_contract_error(value, "Array", heap, debug_name));
    };
    let Some(HeapValue::Array(values)) = heap.and_then(|heap| heap.heap.get(*reference)) else {
        return Err(type_contract_error(value, "Array", heap, debug_name));
    };
    if let Some(element) = element {
        for value in values {
            execute_linked_guard_plan(value, element, program, heap, debug_name)?;
        }
    }
    Ok(())
}

fn execute_linked_set_guard(
    value: &Value,
    element: Option<&TypeGuardPlan>,
    program: &LinkedProgram,
    heap: Option<&HeapExecution<'_>>,
    debug_name: &str,
) -> VmResult<()> {
    let Value::HeapRef(reference) = value else {
        return Err(type_contract_error(value, "Set", heap, debug_name));
    };
    let Some(HeapValue::Set(values)) = heap.and_then(|heap| heap.heap.get(*reference)) else {
        return Err(type_contract_error(value, "Set", heap, debug_name));
    };
    if let Some(element) = element {
        for value in values {
            execute_linked_guard_plan(value, element, program, heap, debug_name)?;
        }
    }
    Ok(())
}

fn execute_linked_map_guard(
    value: &Value,
    key: Option<&TypeGuardPlan>,
    value_plan: Option<&TypeGuardPlan>,
    program: &LinkedProgram,
    heap: Option<&HeapExecution<'_>>,
    debug_name: &str,
) -> VmResult<()> {
    let Value::HeapRef(reference) = value else {
        return Err(type_contract_error(value, "Map", heap, debug_name));
    };
    let Some(HeapValue::Map(values)) = heap.and_then(|heap| heap.heap.get(*reference)) else {
        return Err(type_contract_error(value, "Map", heap, debug_name));
    };
    if !linked_map_key_plan_is_string_or_erased(key) {
        return Err(VmError::new(VmErrorKind::TypeContractViolation {
            expected: "Map<String, _>".to_owned(),
            actual: "Map".to_owned(),
            debug_name: debug_name.to_owned(),
        }));
    }
    if let Some(value_plan) = value_plan {
        for value in values.values() {
            execute_linked_guard_plan(value, value_plan, program, heap, debug_name)?;
        }
    }
    Ok(())
}

fn execute_linked_result_guard(
    value: &Value,
    ok: Option<&TypeGuardPlan>,
    err: Option<&TypeGuardPlan>,
    program: &LinkedProgram,
    heap: Option<&HeapExecution<'_>>,
    debug_name: &str,
) -> VmResult<()> {
    match std_enum_value(value, heap) {
        Some((StdEnumKind::Result, StdEnumVariant::Ok, fields)) => {
            if let Some(ok) = ok {
                let payload = fields
                    .get_slot(0, "0")
                    .map(stored_runtime_value)
                    .ok_or_else(|| {
                        VmError::new(VmErrorKind::TypeMismatch {
                            operation: "Result Ok payload contract",
                        })
                    })?;
                execute_linked_guard_plan(&payload, ok, program, heap, debug_name)?;
            }
            Ok(())
        }
        Some((StdEnumKind::Result, StdEnumVariant::Err, fields)) => {
            if let Some(err) = err {
                let payload = fields
                    .get_slot(0, "0")
                    .map(stored_runtime_value)
                    .ok_or_else(|| {
                        VmError::new(VmErrorKind::TypeMismatch {
                            operation: "Result Err payload contract",
                        })
                    })?;
                execute_linked_guard_plan(&payload, err, program, heap, debug_name)?;
            }
            Ok(())
        }
        _ => Err(type_contract_error(value, "Result", heap, debug_name)),
    }
}

fn execute_linked_guard_plan(
    value: &Value,
    plan: &TypeGuardPlan,
    program: &LinkedProgram,
    heap: Option<&HeapExecution<'_>>,
    debug_name: &str,
) -> VmResult<()> {
    match plan {
        TypeGuardPlan::Primitive(expected) => {
            execute_primitive_guard(value, *expected, heap, debug_name)
        }
        TypeGuardPlan::Standard(expected) => {
            execute_standard_guard(value, *expected, heap, debug_name)
        }
        TypeGuardPlan::Array { element } => {
            execute_linked_array_guard(value, element.as_deref(), program, heap, debug_name)
        }
        TypeGuardPlan::Map { key, value: values } => execute_linked_map_guard(
            value,
            key.as_deref(),
            values.as_deref(),
            program,
            heap,
            debug_name,
        ),
        TypeGuardPlan::Set { element } => {
            execute_linked_set_guard(value, element.as_deref(), program, heap, debug_name)
        }
        TypeGuardPlan::Iterator { .. } => {
            execute_standard_guard(value, StandardTypeGuard::Iterator, heap, debug_name)
        }
        TypeGuardPlan::Option { some } => {
            execute_linked_option_guard(value, some.as_deref(), program, heap, debug_name)
        }
        TypeGuardPlan::Result { ok, err } => execute_linked_result_guard(
            value,
            ok.as_deref(),
            err.as_deref(),
            program,
            heap,
            debug_name,
        ),
        TypeGuardPlan::Type(expected) => {
            let expected = program.ty(*expected).ok_or_else(|| {
                VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
                    opcode: "type_guard",
                })
            })?;
            execute_type_id_guard(
                value,
                expected.id,
                program.debug_name(expected.debug_name),
                heap,
                debug_name,
            )
        }
        TypeGuardPlan::Variant(expected) => {
            let expected = program.variant(*expected).ok_or_else(|| {
                VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
                    opcode: "variant_guard",
                })
            })?;
            execute_variant_id_guard(
                value,
                expected.id,
                program.debug_name(expected.debug_name),
                heap,
                debug_name,
            )
        }
        TypeGuardPlan::Shape { ty, shape_id } => {
            let expected = program.ty(*ty).ok_or_else(|| {
                VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
                    opcode: "shape_guard",
                })
            })?;
            execute_shape_id_guard(
                value,
                expected.id,
                *shape_id,
                program.debug_name(expected.debug_name),
                heap,
                debug_name,
            )
        }
        TypeGuardPlan::HostType(_) => Ok(()),
    }
}

fn execute_type_id_guard(
    value: &Value,
    expected: vela_def::TypeId,
    expected_name: &str,
    heap: Option<&HeapExecution<'_>>,
    debug_name: &str,
) -> VmResult<()> {
    if runtime_type_id(value, heap) == Some(expected) {
        return Ok(());
    }
    Err(type_contract_error(value, expected_name, heap, debug_name))
}

fn execute_variant_id_guard(
    value: &Value,
    expected: vela_def::VariantId,
    expected_name: &str,
    heap: Option<&HeapExecution<'_>>,
    debug_name: &str,
) -> VmResult<()> {
    if runtime_variant_id(value, heap) == Some(expected) {
        return Ok(());
    }
    Err(type_contract_error(value, expected_name, heap, debug_name))
}

fn execute_shape_id_guard(
    value: &Value,
    expected_type: vela_def::TypeId,
    expected_shape: vela_common::ShapeId,
    expected_name: &str,
    heap: Option<&HeapExecution<'_>>,
    debug_name: &str,
) -> VmResult<()> {
    if runtime_record_shape(value, heap) == Some((expected_type, expected_shape)) {
        return Ok(());
    }
    if runtime_record_debug_shape(value, heap) == Some((expected_name, expected_shape)) {
        return Ok(());
    }
    Err(type_contract_error(value, expected_name, heap, debug_name))
}

fn execute_unlinked_type_guard(
    value: &Value,
    expected: &str,
    heap: Option<&HeapExecution<'_>>,
    debug_name: &str,
) -> VmResult<()> {
    if runtime_type_debug_name(value, heap) == Some(expected) {
        return Ok(());
    }
    Err(type_contract_error(value, expected, heap, debug_name))
}

fn execute_unlinked_variant_guard(
    value: &Value,
    expected_enum: &str,
    expected_variant: &str,
    heap: Option<&HeapExecution<'_>>,
    debug_name: &str,
) -> VmResult<()> {
    let expected = format!("{expected_enum}::{expected_variant}");
    if runtime_variant_debug_name(value, heap) == Some((expected_enum, expected_variant)) {
        return Ok(());
    }
    Err(type_contract_error(value, &expected, heap, debug_name))
}

fn execute_unlinked_shape_guard(
    value: &Value,
    expected_type: &str,
    expected_shape: vela_common::ShapeId,
    heap: Option<&HeapExecution<'_>>,
    debug_name: &str,
) -> VmResult<()> {
    if runtime_record_debug_shape(value, heap) == Some((expected_type, expected_shape)) {
        return Ok(());
    }
    Err(type_contract_error(value, expected_type, heap, debug_name))
}

fn std_enum_value<'a>(
    value: &Value,
    heap: Option<&'a HeapExecution<'_>>,
) -> Option<(
    StdEnumKind,
    StdEnumVariant,
    &'a crate::script_object::ScriptFields<Value>,
)> {
    let Value::HeapRef(reference) = value else {
        return None;
    };
    let HeapValue::Enum {
        identity: Some(identity),
        fields,
        ..
    } = heap?.heap.get(*reference)?
    else {
        return None;
    };
    let (kind, variant) = std_enum_tag(*identity)?;
    Some((kind, variant, fields))
}

fn runtime_standard_type(
    value: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> Option<StandardTypeGuard> {
    match value {
        Value::Range(_) => Some(StandardTypeGuard::Range),
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::Array(_)) => Some(StandardTypeGuard::Array),
            Some(HeapValue::Map(_)) => Some(StandardTypeGuard::Map),
            Some(HeapValue::Set(_)) => Some(StandardTypeGuard::Set),
            Some(HeapValue::Closure(_)) => Some(StandardTypeGuard::Closure),
            Some(HeapValue::Iterator(_)) => Some(StandardTypeGuard::Iterator),
            Some(HeapValue::Enum {
                identity: Some(identity),
                ..
            }) => match std_enum_tag(*identity) {
                Some((StdEnumKind::Option, _)) => Some(StandardTypeGuard::Option),
                Some((StdEnumKind::Result, _)) => Some(StandardTypeGuard::Result),
                None => None,
            },
            _ => None,
        },
        _ => None,
    }
}

fn standard_type_name(guard: StandardTypeGuard) -> &'static str {
    match guard {
        StandardTypeGuard::Array => "Array",
        StandardTypeGuard::Map => "Map",
        StandardTypeGuard::Set => "Set",
        StandardTypeGuard::Range => "Range",
        StandardTypeGuard::Function => "Function",
        StandardTypeGuard::Closure => "Closure",
        StandardTypeGuard::Iterator => "Iterator",
        StandardTypeGuard::Option => "Option",
        StandardTypeGuard::Result => "Result",
    }
}

fn map_key_plan_is_string_or_erased(plan: Option<&UnlinkedTypeGuardPlan>) -> bool {
    matches!(
        plan,
        None | Some(UnlinkedTypeGuardPlan::Primitive(PrimitiveTag::String))
    )
}

fn linked_map_key_plan_is_string_or_erased(plan: Option<&TypeGuardPlan>) -> bool {
    matches!(
        plan,
        None | Some(TypeGuardPlan::Primitive(PrimitiveTag::String))
    )
}

const fn primitive_type_name(tag: PrimitiveTag) -> &'static str {
    match tag {
        PrimitiveTag::String => "String",
        PrimitiveTag::Bytes => "Bytes",
        _ => tag.name(),
    }
}

fn type_contract_error(
    value: &Value,
    expected: &str,
    heap: Option<&HeapExecution<'_>>,
    debug_name: &str,
) -> VmError {
    VmError::new(VmErrorKind::TypeContractViolation {
        expected: expected.to_owned(),
        actual: runtime_type_name(value, heap).to_owned(),
        debug_name: debug_name.to_owned(),
    })
}

macro_rules! define_runtime_type_helpers {
    ($($value_variant:ident => $primitive_tag:ident),* $(,)?) => {
        fn runtime_primitive_tag(
            value: &Value,
            heap: Option<&HeapExecution<'_>>,
        ) -> Option<PrimitiveTag> {
            match value {
                Value::Null => Some(PrimitiveTag::Null),
                Value::Bool(_) => Some(PrimitiveTag::Bool),
                Value::Char(_) => Some(PrimitiveTag::Char),
                $(
                    Value::$value_variant(_) => Some(PrimitiveTag::$primitive_tag),
                )*
                Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
                    Some(HeapValue::String(_)) => Some(PrimitiveTag::String),
                    Some(HeapValue::Bytes(_)) => Some(PrimitiveTag::Bytes),
                    _ => None,
                },
                Value::Missing | Value::Range(_) | Value::HostRef(_) => None,
            }
        }

        fn runtime_type_name<'a>(
            value: &Value,
            heap: Option<&'a HeapExecution<'_>>,
        ) -> &'a str {
            match value {
                Value::Missing => "missing",
                Value::Null => primitive_type_name(PrimitiveTag::Null),
                Value::Bool(_) => primitive_type_name(PrimitiveTag::Bool),
                Value::Char(_) => primitive_type_name(PrimitiveTag::Char),
                $(
                    Value::$value_variant(_) => primitive_type_name(PrimitiveTag::$primitive_tag),
                )*
                Value::Range(_) => "Range",
                Value::HostRef(_) => "host",
                Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
                    Some(HeapValue::String(_)) => primitive_type_name(PrimitiveTag::String),
                    Some(HeapValue::Bytes(_)) => primitive_type_name(PrimitiveTag::Bytes),
                    Some(HeapValue::Array(_)) => "Array",
                    Some(HeapValue::Map(_)) => "Map",
                    Some(HeapValue::Set(_)) => "Set",
                    Some(HeapValue::Record { .. }) => "record",
                    Some(HeapValue::Enum { .. }) => "enum",
                    Some(HeapValue::Closure(_)) => "Closure",
                    Some(HeapValue::PathProxy(_)) => "host_path",
                    Some(HeapValue::Iterator(_)) => "Iterator",
                    None => "heap",
                },
            }
        }
    };
}

define_runtime_type_helpers!(
    I8 => I8,
    I16 => I16,
    I32 => I32,
    I64 => I64,
    U8 => U8,
    U16 => U16,
    U32 => U32,
    U64 => U64,
    F32 => F32,
    F64 => F64,
);

fn runtime_type_id(value: &Value, heap: Option<&HeapExecution<'_>>) -> Option<vela_def::TypeId> {
    match value {
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::Record {
                identity: Some(identity),
                ..
            }) => Some(identity.type_id),
            Some(HeapValue::Enum {
                identity: Some(identity),
                ..
            }) => Some(identity.type_id),
            _ => None,
        },
        _ => None,
    }
}

fn runtime_variant_id(
    value: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> Option<vela_def::VariantId> {
    match value {
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::Enum {
                identity: Some(identity),
                ..
            }) => Some(identity.variant_id),
            _ => None,
        },
        _ => None,
    }
}

fn runtime_record_shape(
    value: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> Option<(vela_def::TypeId, vela_common::ShapeId)> {
    match value {
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::Record {
                identity: Some(identity),
                ..
            }) => Some((identity.type_id, identity.shape_id)),
            _ => None,
        },
        _ => None,
    }
}

fn runtime_type_debug_name<'a>(
    value: &Value,
    heap: Option<&'a HeapExecution<'_>>,
) -> Option<&'a str> {
    match value {
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::Record { type_name, .. }) => Some(type_name),
            Some(HeapValue::Enum { enum_name, .. }) => Some(enum_name),
            _ => None,
        },
        _ => None,
    }
}

fn runtime_variant_debug_name<'a>(
    value: &Value,
    heap: Option<&'a HeapExecution<'_>>,
) -> Option<(&'a str, &'a str)> {
    match value {
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::Enum {
                enum_name, variant, ..
            }) => Some((enum_name, variant)),
            _ => None,
        },
        _ => None,
    }
}

fn runtime_record_debug_shape<'a>(
    value: &Value,
    heap: Option<&'a HeapExecution<'_>>,
) -> Option<(&'a str, vela_common::ShapeId)> {
    match value {
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::Record {
                type_name,
                identity: Some(identity),
                ..
            }) => Some((type_name, identity.shape_id)),
            Some(HeapValue::Record {
                type_name, fields, ..
            }) => Some((type_name, fields.shape_id())),
            _ => None,
        },
        _ => None,
    }
}
