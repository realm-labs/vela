use vela_bytecode::UnlinkedCodeObject;
use vela_host::path::HostRef;

use crate::heap::HeapValue;
use crate::owned_value::OwnedValue;
use crate::value::ClosureValue;
use crate::{HeapExecution, Value, VmError, VmErrorKind, VmResult};

pub(crate) fn expect_host_ref(value: &Value, operation: &'static str) -> VmResult<HostRef> {
    match value {
        Value::HostRef(value) => Ok(*value),
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
    }
}

pub(crate) fn expect_closure_ref<'heap>(
    value: &Value,
    heap: Option<&'heap HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<&'heap ClosureValue> {
    match value {
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::Closure(closure)) => Ok(closure),
            _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
        },
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
    }
}

pub(crate) trait StringArgument {
    fn as_string_arg(&self) -> Option<&str>;
}

impl StringArgument for Value {
    fn as_string_arg(&self) -> Option<&str> {
        None
    }
}

impl StringArgument for OwnedValue {
    fn as_string_arg(&self) -> Option<&str> {
        match self {
            OwnedValue::String(value) => Some(value),
            _ => None,
        }
    }
}

pub(crate) fn expect_string<'a, T: StringArgument + ?Sized>(
    value: &'a T,
    operation: &'static str,
) -> VmResult<&'a str> {
    value
        .as_string_arg()
        .ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation }))
}

#[inline]
pub(crate) fn expect_int(value: &Value, operation: &'static str) -> VmResult<i64> {
    match value {
        Value::Int(value) => Ok(*value),
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
    }
}

#[inline]
pub(crate) fn expect_arity<T>(name: &str, args: &[T], expected: usize) -> VmResult<()> {
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

#[inline]
pub(crate) fn is_truthy(value: &Value) -> bool {
    !matches!(value, Value::Missing | Value::Null | Value::Bool(false))
}

pub(crate) fn validate_jump(code: &UnlinkedCodeObject, offset: usize) -> VmResult<()> {
    if offset <= code.instructions.len() {
        Ok(())
    } else {
        Err(VmError::new(VmErrorKind::InstructionOutOfBounds { offset }))
    }
}
