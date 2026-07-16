use std::sync::Arc;

use vela_bytecode::{LinkedArtifact, ScriptFunctionHandle};

use crate::budget::ExecutionBudget;
use crate::error::{VmError, VmErrorKind, VmResult, VmStackFrame};
use crate::execution_session::{
    LinkedExecutionSession, PendingFrameOperation, PendingReturnTarget, ReturnContinuation,
};
use crate::heap_execution::HeapExecution;
use crate::value::Value;
use crate::{Vm, VmBytecodeProfiler, VmInlineCaches};

pub struct LinkedExecutionReentry<'artifact, 'args, 'caches> {
    pub artifact: &'artifact Arc<LinkedArtifact>,
    pub function: ScriptFunctionHandle,
    pub args: &'args [Value],
    pub inline_caches: Option<&'caches dyn VmInlineCaches>,
    pub bytecode_profiler: Option<&'caches dyn VmBytecodeProfiler>,
}

impl Vm {
    pub fn push_linked_reentry(
        &self,
        session: &mut LinkedExecutionSession,
        reentry: LinkedExecutionReentry<'_, '_, '_>,
        heap: &mut HeapExecution<'_>,
        budget: &mut ExecutionBudget,
    ) -> VmResult<()> {
        if session.pending_native.is_empty() {
            return Err(VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
                opcode: "reentry without a suspended native invocation",
            }));
        }
        if reentry.artifact.generation() != session.root_generation {
            return Err(VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
                opcode: "reentry artifact generation mismatch",
            }));
        }

        let stack_frame = reentry
            .artifact
            .program()
            .function(reentry.function)
            .map_or_else(
                || VmStackFrame::new("<missing linked reentry function>", None),
                |code| {
                    VmStackFrame::new(reentry.artifact.program().debug_name(code.debug_name), None)
                },
            );
        let limits_call_depth = budget.limits_call_depth();
        if limits_call_depth {
            budget
                .enter_call()
                .map_err(|error| error.with_call_frame(stack_frame.clone()))?;
        }

        let protected_root_len = session.frames.last().map(|caller| {
            let protected_root_len = heap.push_frame_roots(&caller.registers);
            if let Some(operation) = caller.pending_operation.as_ref() {
                match operation {
                    PendingFrameOperation::CallbackMethod { callback, .. } => {
                        callback.protect_roots(heap);
                    }
                    PendingFrameOperation::IteratorNext { next, .. } => {
                        next.protect_roots(heap);
                    }
                    PendingFrameOperation::Comparison { .. }
                    | PendingFrameOperation::ArrayOrdering { .. } => {}
                }
            }
            protected_root_len
        });
        let entry = self.prepare_execution_frame(
            Arc::clone(reentry.artifact),
            reentry.function,
            &[],
            reentry.args,
            true,
            None,
            None,
            reentry.inline_caches,
            reentry.bytecode_profiler,
            Some(ReturnContinuation {
                target: PendingReturnTarget::Reentry,
                protected_root_len,
            }),
            Some(heap),
            Some(budget),
        );
        match entry {
            Ok(entry) => {
                session.frames.push(entry);
                Ok(())
            }
            Err(error) => {
                if let Some(protected_root_len) = protected_root_len {
                    heap.truncate_protected_roots(protected_root_len);
                }
                if limits_call_depth {
                    budget.exit_call();
                }
                Err(error.with_call_frame(stack_frame))
            }
        }
    }

    pub fn abort_linked_reentry(
        &self,
        session: &mut LinkedExecutionSession,
        heap: &mut HeapExecution<'_>,
        budget: &mut ExecutionBudget,
    ) -> VmResult<()> {
        let Some(reentry_index) = session.top_reentry_frame() else {
            return Err(VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
                opcode: "abort without an active reentry",
            }));
        };
        let protected_root_len = session.frames[reentry_index]
            .return_to
            .and_then(|return_to| return_to.protected_root_len);
        let removed = session.frames.len().saturating_sub(reentry_index);
        session.frames.truncate(reentry_index);
        if let Some(protected_root_len) = protected_root_len {
            heap.truncate_protected_roots(protected_root_len);
        }
        if budget.limits_call_depth() {
            for _ in 0..removed {
                budget.exit_call();
            }
        }
        Ok(())
    }
}
