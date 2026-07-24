use std::sync::Arc;

use vela_bytecode::{LinkedArtifact, ScriptFunctionHandle};

use crate::heap_execution::HeapExecution;
use crate::runtime_checks::expect_closure_ref;
use crate::{ExecutionBudget, Value, VmError, VmErrorKind, VmResult};

use super::methods::{restore_iterator_to_heap, take_iterator_from_heap};
use super::state::IteratorPollStep;

pub(crate) struct ResumableIteratorNext {
    receiver: Value,
    operation: &'static str,
    charge_step: bool,
    started: bool,
}

pub(crate) enum ResumableIteratorStep {
    Complete(Option<Value>),
    Call {
        owner: Arc<LinkedArtifact>,
        function: ScriptFunctionHandle,
        captures: Vec<Value>,
        args: Vec<Value>,
    },
}

impl ResumableIteratorNext {
    pub(crate) fn new(receiver: Value, operation: &'static str, charge_step: bool) -> Self {
        Self {
            receiver,
            operation,
            charge_step,
            started: false,
        }
    }

    pub(crate) fn step(
        &mut self,
        program_owner: &Arc<LinkedArtifact>,
        host: &mut Option<&mut dyn crate::method_runtime::HostIteratorAccess>,
        heap: &mut Option<&mut HeapExecution<'_>>,
        budget: &mut Option<&mut ExecutionBudget>,
        returned: Option<Value>,
    ) -> VmResult<ResumableIteratorStep> {
        let mut iterator = take_iterator_from_heap(&self.receiver, heap, self.operation)?;
        let charge_step = self.charge_step && !self.started;
        self.started = true;
        let result = {
            let mut runtime = crate::method_runtime::MethodRuntime {
                program: program_owner.program(),
                heap: heap.as_deref_mut(),
                budget: budget.as_deref_mut(),
                host: host.as_deref_mut(),
            };
            iterator.poll_next(&mut runtime, self.operation, charge_step, returned)
        };
        restore_iterator_to_heap(self.receiver, heap, iterator, self.operation)?;
        match result? {
            IteratorPollStep::Complete(value) => Ok(ResumableIteratorStep::Complete(value)),
            IteratorPollStep::Call { callback, args } => {
                if let Some(budget) = budget.as_deref_mut() {
                    budget.charge_execution_units(1)?;
                }
                let closure = expect_closure_ref(&callback, heap.as_deref(), self.operation)?;
                let code = closure.owner.function(closure.function).ok_or_else(|| {
                    VmError::new(VmErrorKind::UnknownFunction {
                        name: format!("<linked closure#{}>", closure.function.index()),
                    })
                })?;
                if code.asyncness.is_async() {
                    return Err(VmError::new(VmErrorKind::AsyncCallRequiresAwait {
                        name: closure
                            .owner
                            .program()
                            .debug_name(code.debug_name)
                            .to_owned(),
                    }));
                }
                Ok(ResumableIteratorStep::Call {
                    owner: Arc::clone(&closure.owner),
                    function: closure.function,
                    captures: closure.captures.as_slice().to_vec(),
                    args,
                })
            }
        }
    }

    pub(crate) fn protect_roots(&self, heap: &mut HeapExecution<'_>) {
        heap.protect_values(&[self.receiver]);
    }
}
