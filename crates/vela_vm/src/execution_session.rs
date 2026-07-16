use std::sync::Arc;

use vela_bytecode::{InstructionOffset, LinkedArtifact, Register, ScriptFunctionHandle};
use vela_common::Span;

use crate::array_methods;
use crate::budget::ExecutionBudget;
use crate::equality::ResumableComparison;
use crate::error::{VmResult, VmStackFrame};
use crate::frame::CallFrame;
use crate::heap_execution::HeapExecution;
use crate::iteration;
use crate::resumable_callbacks::ResumableCallbackMethod;
use crate::value::Value;
use crate::{Vm, VmBytecodeProfiler, VmInlineCaches};

pub struct LinkedExecutionSession {
    pub(crate) frames: Vec<ExecutionFrame>,
    pub(crate) pending_native: Vec<PendingNativeResume>,
    pub(crate) root_call_depth_charged: bool,
    pub(crate) root_generation: vela_bytecode::ExecutableGenerationId,
    pub(crate) context_native_boundaries: bool,
}

impl LinkedExecutionSession {
    pub fn enable_context_native_boundaries(&mut self) {
        self.context_native_boundaries = true;
    }

    pub(crate) fn top_reentry_frame(&self) -> Option<usize> {
        self.frames.iter().rposition(|frame| {
            frame
                .return_to
                .is_some_and(|return_to| matches!(return_to.target, PendingReturnTarget::Reentry))
        })
    }
}

#[derive(Clone, Copy)]
pub(crate) struct PendingNativeResume {
    pub(crate) destination: Option<Register>,
    pub(crate) source_span: Option<Span>,
}

pub(crate) struct ExecutionFrame {
    pub(crate) owner: Arc<LinkedArtifact>,
    pub(crate) function: ScriptFunctionHandle,
    pub(crate) ip: InstructionOffset,
    pub(crate) registers: CallFrame,
    pub(crate) return_to: Option<ReturnContinuation>,
    pub(crate) pending_operation: Option<PendingFrameOperation>,
    pub(crate) call_site: Option<Span>,
    pub(crate) call_site_offset: Option<InstructionOffset>,
}

impl ExecutionFrame {
    pub(crate) fn stack_frame(&self) -> VmStackFrame {
        let program = self.owner.program();
        let Some(code) = program.function(self.function) else {
            return VmStackFrame::new("<missing linked function>", self.call_site)
                .with_bytecode_offset(self.call_site_offset);
        };
        VmStackFrame::new(program.debug_name(code.debug_name), self.call_site)
            .with_bytecode_offset(self.call_site_offset)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ReturnContinuation {
    pub(crate) target: PendingReturnTarget,
    pub(crate) protected_root_len: Option<usize>,
}

#[derive(Clone, Copy)]
pub(crate) enum PendingReturnTarget {
    Register(Register),
    Operation,
    Reentry,
}

pub(crate) enum PendingFrameOperation {
    Comparison {
        comparison: ResumableComparison,
        destination: Register,
        returned: Option<Value>,
        source_span: Option<Span>,
    },
    ArrayOrdering {
        ordering: array_methods::ResumableArrayOrdering,
        destination: Register,
        returned: Option<Value>,
        source_span: Option<Span>,
    },
    CallbackMethod {
        callback: ResumableCallbackMethod,
        destination: Register,
        returned: Option<Value>,
        source_span: Option<Span>,
    },
    IteratorNext {
        next: iteration::ResumableIteratorNext,
        destination: Register,
        jump_if_done: InstructionOffset,
        returned: Option<Value>,
        source_span: Option<Span>,
    },
}

pub(crate) struct PendingLinkedCall {
    pub(crate) owner: Arc<LinkedArtifact>,
    pub(crate) function: ScriptFunctionHandle,
    pub(crate) captures: Vec<Value>,
    pub(crate) args: Vec<Value>,
    pub(crate) check_param_guards: bool,
    pub(crate) call_site: Option<Span>,
    pub(crate) call_site_offset: Option<InstructionOffset>,
    pub(crate) return_target: PendingReturnTarget,
}

impl PendingLinkedCall {
    pub(crate) fn stack_frame(&self) -> VmStackFrame {
        let program = self.owner.program();
        let Some(code) = program.function(self.function) else {
            return VmStackFrame::new("<missing linked function>", self.call_site)
                .with_bytecode_offset(self.call_site_offset);
        };
        VmStackFrame::new(program.debug_name(code.debug_name), self.call_site)
            .with_bytecode_offset(self.call_site_offset)
    }
}

pub struct LinkedExecutionStart<'artifact, 'args, 'caches> {
    pub artifact: &'artifact Arc<LinkedArtifact>,
    pub function: ScriptFunctionHandle,
    pub args: &'args [Value],
    pub roots: &'args [Value],
    pub inline_caches: Option<&'caches dyn VmInlineCaches>,
    pub bytecode_profiler: Option<&'caches dyn VmBytecodeProfiler>,
}

impl Vm {
    pub fn start_linked_execution(
        &self,
        start: LinkedExecutionStart<'_, '_, '_>,
        heap: &mut HeapExecution<'_>,
        budget: &mut ExecutionBudget,
    ) -> VmResult<LinkedExecutionSession> {
        heap.protect_values(start.roots);
        heap.protect_values(start.args);
        let stack_frame = start
            .artifact
            .program()
            .function(start.function)
            .map_or_else(
                || VmStackFrame::new("<missing linked function>", None),
                |code| {
                    VmStackFrame::new(start.artifact.program().debug_name(code.debug_name), None)
                },
            );
        let limits_call_depth = budget.limits_call_depth();
        if limits_call_depth {
            budget
                .enter_call()
                .map_err(|error| error.with_call_frame(stack_frame.clone()))?;
        }
        let entry = self.prepare_execution_frame(
            Arc::clone(start.artifact),
            start.function,
            &[],
            start.args,
            true,
            None,
            None,
            start.inline_caches,
            start.bytecode_profiler,
            None,
            Some(heap),
            Some(budget),
        );
        match entry {
            Ok(entry) => Ok(LinkedExecutionSession {
                root_generation: entry.owner.generation(),
                context_native_boundaries: false,
                frames: vec![entry],
                pending_native: Vec::new(),
                root_call_depth_charged: limits_call_depth,
            }),
            Err(error) => {
                if limits_call_depth {
                    budget.exit_call();
                }
                Err(error.with_call_frame(stack_frame))
            }
        }
    }
}
