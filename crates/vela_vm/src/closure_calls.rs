use std::sync::Arc;

use vela_bytecode::Register;
use vela_common::Span;

use crate::heap::HeapValue;
use crate::value::ClosureValue;
use crate::{
    CallFrame, ExecutionBudget, HeapExecution, SmallStorage, Value, VmError, VmErrorKind, VmResult,
    allocate_heap_value,
};

pub(crate) struct LinkedMakeClosure<'a> {
    pub(crate) dst: Register,
    pub(crate) function: vela_bytecode::ScriptFunctionHandle,
    pub(crate) captures: &'a [Register],
    pub(crate) call_site: Option<Span>,
}

pub(crate) fn make_linked_closure(
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    frame: &mut CallFrame,
    closure: LinkedMakeClosure<'_>,
) -> VmResult<()> {
    let captures = captures_from_registers(frame, closure.captures)?;
    let heap = heap.as_deref_mut().ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "closure heap",
        })
        .with_source_span_if_absent(closure.call_site)
    })?;
    let value = allocate_heap_value(
        HeapValue::Closure(ClosureValue {
            owner: Arc::clone(frame.linked_owner().ok_or_else(|| {
                VmError::new(VmErrorKind::TypeMismatch {
                    operation: "linked closure owner",
                })
            })?),
            function: closure.function,
            captures,
        }),
        heap,
        budget.as_deref_mut(),
    )?;
    frame.write(closure.dst, value)
}

fn captures_from_registers(
    frame: &CallFrame,
    captures: &[Register],
) -> VmResult<SmallStorage<Value>> {
    SmallStorage::try_from_slice_map(captures, 4, |register| frame.read(*register))
}

#[cfg(test)]
mod tests {
    use super::captures_from_registers;
    use crate::{CallFrame, SmallStorage, Value};
    use vela_bytecode::Register;

    #[test]
    fn captures_from_registers_uses_inline_storage_for_common_arity() {
        let mut frame = CallFrame::new(4);
        for index in 0..4 {
            frame
                .write(Register(index), Value::i64(i64::from(index)))
                .expect("register write");
        }

        let captures = captures_from_registers(
            &frame,
            &[Register(0), Register(1), Register(2), Register(3)],
        )
        .expect("captures");

        assert!(matches!(captures, SmallStorage::Four(_)));
    }
}
