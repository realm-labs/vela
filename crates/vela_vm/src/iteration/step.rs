use crate::runtime_checks::{expect_int, validate_jump};
use crate::{CallFrame, Value, VmError, VmErrorKind, VmResult};
use vela_bytecode::{InstructionOffset, LinkedCodeObject, Register, UnlinkedCodeObject};

use super::IterRuntime;

pub(crate) struct RangeNextStep {
    pub(crate) cursor: Register,
    pub(crate) end: Register,
    pub(crate) done: Register,
    pub(crate) inclusive: bool,
    pub(crate) dst: Register,
    pub(crate) jump_if_done: InstructionOffset,
}

pub(crate) fn dispatch_range_next(
    runtime: IterRuntime<'_, '_>,
    code: &UnlinkedCodeObject,
    step: RangeNextStep,
) -> VmResult<Option<usize>> {
    dispatch_range_next_with(runtime.frame, step, |offset| validate_jump(code, offset))
}

pub(crate) fn dispatch_linked_range_next(
    runtime: IterRuntime<'_, '_>,
    code: &LinkedCodeObject,
    step: RangeNextStep,
) -> VmResult<Option<usize>> {
    dispatch_range_next_with(runtime.frame, step, |offset| {
        debug_assert!(offset <= code.instructions.len());
        Ok(())
    })
}

#[inline(always)]
pub(crate) fn dispatch_i64_range_next(
    frame: &mut CallFrame,
    code: &UnlinkedCodeObject,
    step: RangeNextStep,
) -> VmResult<Option<usize>> {
    dispatch_i64_range_next_with(frame, step, |offset| validate_jump(code, offset))
}

#[inline(always)]
pub(crate) fn dispatch_linked_i64_range_next(
    frame: &mut CallFrame,
    code: &LinkedCodeObject,
    step: RangeNextStep,
) -> VmResult<Option<usize>> {
    dispatch_i64_range_next_with(frame, step, |offset| {
        debug_assert!(offset <= code.instructions.len());
        Ok(())
    })
}

#[inline(always)]
fn dispatch_i64_range_next_with(
    frame: &mut CallFrame,
    step: RangeNextStep,
    mut validate: impl FnMut(usize) -> VmResult<()>,
) -> VmResult<Option<usize>> {
    let is_done = frame.read_bool(step.done, "range")?;
    if is_done {
        validate(step.jump_if_done.0)?;
        return Ok(Some(step.jump_if_done.0));
    }

    let current = frame.read_i64(step.cursor, "range")?;
    let end = frame.read_i64(step.end, "range")?;
    let has_next = if step.inclusive {
        current <= end
    } else {
        current < end
    };
    if has_next {
        frame.write_i64(step.dst, current)?;
        if current == i64::MAX {
            frame.write_bool(step.done, true)?;
        } else {
            frame.write_i64(step.cursor, current + 1)?;
        }
        Ok(None)
    } else {
        frame.write_bool(step.done, true)?;
        validate(step.jump_if_done.0)?;
        Ok(Some(step.jump_if_done.0))
    }
}

fn dispatch_range_next_with(
    frame: &mut CallFrame,
    step: RangeNextStep,
    mut validate: impl FnMut(usize) -> VmResult<()>,
) -> VmResult<Option<usize>> {
    let is_done = match frame.read(step.done)? {
        Value::Bool(value) => value,
        _ => {
            return Err(VmError::new(VmErrorKind::TypeMismatch {
                operation: "range",
            }));
        }
    };
    if is_done {
        validate(step.jump_if_done.0)?;
        return Ok(Some(step.jump_if_done.0));
    }

    let current = expect_int(&frame.read(step.cursor)?, "range")?;
    let end = expect_int(&frame.read(step.end)?, "range")?;
    let has_next = if step.inclusive {
        current <= end
    } else {
        current < end
    };
    if has_next {
        frame.write(step.dst, Value::I64(current))?;
        if current == i64::MAX {
            frame.write(step.done, Value::Bool(true))?;
        } else {
            frame.write(step.cursor, Value::I64(current + 1))?;
        }
        Ok(None)
    } else {
        frame.write(step.done, Value::Bool(true))?;
        validate(step.jump_if_done.0)?;
        Ok(Some(step.jump_if_done.0))
    }
}
