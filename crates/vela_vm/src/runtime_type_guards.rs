use vela_bytecode::{TypeGuard, TypeGuardPlan, UnlinkedTypeGuard, UnlinkedTypeGuardPlan};
use vela_common::PrimitiveTag;

use crate::heap::HeapValue;
use crate::{HeapExecution, Value, VmError, VmErrorKind, VmResult};

pub(crate) fn execute_unlinked_guard(
    value: &Value,
    guard: &UnlinkedTypeGuard,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<()> {
    match guard.plan {
        UnlinkedTypeGuardPlan::Primitive(expected) => {
            execute_primitive_guard(value, expected, heap, &guard.context.debug_name)
        }
        UnlinkedTypeGuardPlan::Type(_)
        | UnlinkedTypeGuardPlan::Variant { .. }
        | UnlinkedTypeGuardPlan::Shape { .. }
        | UnlinkedTypeGuardPlan::HostType(_) => Ok(()),
    }
}

pub(crate) fn execute_linked_guard(
    value: &Value,
    guard: &TypeGuard,
    heap: Option<&HeapExecution<'_>>,
    debug_name: &str,
) -> VmResult<()> {
    match guard.plan {
        TypeGuardPlan::Primitive(expected) => {
            execute_primitive_guard(value, expected, heap, debug_name)
        }
        TypeGuardPlan::Type(_)
        | TypeGuardPlan::Variant(_)
        | TypeGuardPlan::Shape { .. }
        | TypeGuardPlan::HostType(_) => Ok(()),
    }
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
