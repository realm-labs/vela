use crate::budget::ExecutionBudget;
use crate::error::{VmError, VmErrorKind, VmResult};
use crate::execution_session::LinkedExecutionSession;
use crate::heap_execution::HeapExecution;
use crate::{OwnedValue, Vm};

impl Vm {
    pub fn resume_linked_context_call(
        &self,
        session: &mut LinkedExecutionSession,
        result: VmResult<OwnedValue>,
        _host: Option<&mut crate::HostExecution<'_>>,
        heap: Option<&mut HeapExecution<'_>>,
        budget: Option<&mut ExecutionBudget>,
    ) -> VmResult<()> {
        let pending = session.pending_native.pop().ok_or_else(|| {
            VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
                opcode: "context resume without a pending invocation",
            })
        })?;
        let value = result.map_err(|mut error| {
            error = error.with_source_span_if_absent(pending.source_span);
            for frame in session.frames.iter().skip(1).rev() {
                error = error.with_call_frame(frame.stack_frame());
            }
            if let Some(root) = session.frames.first() {
                error = error.with_call_frame(root.stack_frame());
            }
            error
        })?;
        let heap = heap.ok_or_else(|| {
            VmError::new(VmErrorKind::TypeMismatch {
                operation: "native boundary heap",
            })
            .with_source_span_if_absent(pending.source_span)
        })?;
        let owner = session
            .frames
            .last()
            .map(|frame| std::sync::Arc::clone(&frame.owner))
            .ok_or_else(|| {
                VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
                    opcode: "native resume without an active frame",
                })
            })?;
        let value =
            crate::heap_values::owned_to_linked_value(value, owner.program(), heap, budget)?;
        if let Some(destination) = pending.destination {
            session
                .frames
                .last_mut()
                .expect("pending context invocation retains an active frame")
                .registers
                .write(destination, value)?;
        }
        Ok(())
    }
}
