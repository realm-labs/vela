use vela_bytecode::CodeObject;
use vela_host::HostRef;

use crate::{ClosureValue, Value, VmError, VmErrorKind, VmResult};

pub(crate) fn expect_host_ref(value: &Value, operation: &'static str) -> VmResult<HostRef> {
    match value {
        Value::HostRef(value) => Ok(*value),
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
    }
}

pub(crate) fn expect_closure(value: &Value, operation: &'static str) -> VmResult<ClosureValue> {
    match value {
        Value::Closure(closure) => Ok(closure.clone()),
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
    }
}

pub(crate) fn expect_string<'a>(value: &'a Value, operation: &'static str) -> VmResult<&'a str> {
    match value {
        Value::String(value) => Ok(value),
        Value::Null
        | Value::Missing
        | Value::Bool(_)
        | Value::Int(_)
        | Value::Float(_)
        | Value::Array(_)
        | Value::Set(_)
        | Value::Map(_)
        | Value::Record { .. }
        | Value::Enum { .. }
        | Value::Range(_)
        | Value::Closure(_)
        | Value::HeapRef(_)
        | Value::Iterator(_)
        | Value::HostRef(_)
        | Value::PathProxy(_) => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
    }
}

pub(crate) fn expect_int(value: &Value, operation: &'static str) -> VmResult<i64> {
    match value {
        Value::Int(value) => Ok(*value),
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
    }
}

pub(crate) fn expect_arity(name: &str, args: &[Value], expected: usize) -> VmResult<()> {
    if args.len() == expected {
        Ok(())
    } else {
        Err(VmError::new(VmErrorKind::ArityMismatch {
            name: name.to_owned(),
            expected,
            actual: args.len(),
        }))
    }
}

pub(crate) fn is_truthy(value: &Value) -> bool {
    !matches!(value, Value::Missing | Value::Null | Value::Bool(false))
}

pub(crate) fn validate_jump(code: &CodeObject, offset: usize) -> VmResult<()> {
    if offset <= code.instructions.len() {
        Ok(())
    } else {
        Err(VmError::new(VmErrorKind::InstructionOutOfBounds { offset }))
    }
}
