use vela_bytecode::{
    GuardKind, LinkedCodeObject, LinkedProgram, Register, TypeGuard, TypeGuardPlan,
    TypeGuardPlanId, UnlinkedTypeGuard, UnlinkedTypeGuardPlan,
};
use vela_common::PrimitiveTag;

use crate::heap::HeapValue;
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
        expected: expected.name().to_owned(),
        actual: runtime_type_name(value, heap).to_owned(),
        debug_name: debug_name.to_owned(),
    }))
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

fn runtime_primitive_tag(value: &Value, heap: Option<&HeapExecution<'_>>) -> Option<PrimitiveTag> {
    match value {
        Value::Null => Some(PrimitiveTag::Null),
        Value::Bool(_) => Some(PrimitiveTag::Bool),
        Value::Scalar(value) => Some(value.primitive_tag()),
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::String(_)) => Some(PrimitiveTag::String),
            Some(HeapValue::Bytes(_)) => Some(PrimitiveTag::Bytes),
            _ => None,
        },
        Value::Missing | Value::Range(_) | Value::HostRef(_) => None,
    }
}

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

fn runtime_type_name<'a>(value: &Value, heap: Option<&'a HeapExecution<'_>>) -> &'a str {
    match value {
        Value::Missing => "missing",
        Value::Null => PrimitiveTag::Null.name(),
        Value::Bool(_) => PrimitiveTag::Bool.name(),
        Value::Scalar(value) => value.type_name(),
        Value::Range(_) => "range",
        Value::HostRef(_) => "host",
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::String(_)) => PrimitiveTag::String.name(),
            Some(HeapValue::Bytes(_)) => PrimitiveTag::Bytes.name(),
            Some(HeapValue::Array(_)) => "array",
            Some(HeapValue::Map(_)) => "map",
            Some(HeapValue::Set(_)) => "set",
            Some(HeapValue::Record { .. }) => "record",
            Some(HeapValue::Enum { .. }) => "enum",
            Some(HeapValue::Closure(_)) => "closure",
            Some(HeapValue::PathProxy(_)) => "host_path",
            Some(HeapValue::Iterator(_)) => "iterator",
            None => "heap",
        },
    }
}
