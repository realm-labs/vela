use vela_bytecode::{InstructionOffset, LinkedCodeObject, Register};
use vela_common::ScalarValue;

use crate::heap::HeapValue;
use crate::iteration::{IterRuntime, RangeNextStep};
use crate::runtime_checks::expect_int;
use crate::{CallFrame, Value, VmError, VmErrorKind, VmResult};

pub(crate) fn validate_jump(code: &LinkedCodeObject, offset: usize) -> VmResult<()> {
    if offset <= code.instructions.len() {
        Ok(())
    } else {
        Err(VmError::new(VmErrorKind::InstructionOutOfBounds { offset }))
    }
}

pub(crate) fn iter_next(
    mut runtime: IterRuntime<'_, '_>,
    code: &LinkedCodeObject,
    iterator: Register,
    dst: Register,
    jump_if_done: InstructionOffset,
) -> VmResult<Option<usize>> {
    let value = *runtime.frame.read(iterator)?;
    let next = match value {
        Value::HeapRef(reference) => {
            let Some(HeapValue::Iterator(iterator_state)) = runtime
                .heap
                .as_deref_mut()
                .and_then(|heap| heap.heap.get_mut(reference).ok())
            else {
                return Err(VmError::new(VmErrorKind::TypeMismatch {
                    operation: "iterator",
                }));
            };
            iterator_state.next()
        }
        _ => {
            return Err(VmError::new(VmErrorKind::TypeMismatch {
                operation: "iterator",
            }));
        }
    };
    match next {
        Some(value) => {
            runtime.frame.write(dst, value)?;
            Ok(None)
        }
        None => {
            validate_jump(code, jump_if_done.0)?;
            Ok(Some(jump_if_done.0))
        }
    }
}

pub(crate) fn range_next(
    frame: &mut CallFrame,
    code: &LinkedCodeObject,
    step: RangeNextStep,
) -> VmResult<Option<usize>> {
    let is_done = match frame.read(step.done)? {
        Value::Bool(value) => *value,
        _ => {
            return Err(VmError::new(VmErrorKind::TypeMismatch {
                operation: "range",
            }));
        }
    };
    if is_done {
        validate_jump(code, step.jump_if_done.0)?;
        return Ok(Some(step.jump_if_done.0));
    }

    let current = expect_int(frame.read(step.cursor)?, "range")?;
    let end = expect_int(frame.read(step.end)?, "range")?;
    let has_next = if step.inclusive {
        current <= end
    } else {
        current < end
    };
    if has_next {
        frame.write(step.dst, Value::Scalar(ScalarValue::I64(current)))?;
        if current == i64::MAX {
            frame.write(step.done, Value::Bool(true))?;
        } else {
            frame.write(step.cursor, Value::Scalar(ScalarValue::I64(current + 1)))?;
        }
        Ok(None)
    } else {
        frame.write(step.done, Value::Bool(true))?;
        validate_jump(code, step.jump_if_done.0)?;
        Ok(Some(step.jump_if_done.0))
    }
}
