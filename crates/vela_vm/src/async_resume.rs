use crate::budget::ExecutionBudget;
use crate::error::{VmError, VmErrorKind, VmResult};
use crate::execution_session::LinkedExecutionSession;
use crate::heap_execution::HeapExecution;
use crate::native_function_calls;
use crate::{HostExecution, NativeCallFuture, OwnedValue, Vm};

pub struct PreparedAsyncCall {
    pub(crate) native_id: Option<vela_def::FunctionId>,
    pub(crate) method_id: Option<vela_common::HostMethodId>,
    pub(crate) function: native_function_calls::PreparedAsyncNativeFunction,
    pub(crate) args: Vec<OwnedValue>,
    pub(crate) name: String,
}

impl PreparedAsyncCall {
    #[must_use]
    pub const fn native_id(&self) -> Option<vela_def::FunctionId> {
        self.native_id
    }

    #[must_use]
    pub const fn method_id(&self) -> Option<vela_common::HostMethodId> {
        self.method_id
    }

    #[must_use]
    pub fn args(&self) -> &[OwnedValue] {
        &self.args
    }

    #[must_use]
    pub fn invoke(&self) -> NativeCallFuture<'_> {
        match &self.function {
            native_function_calls::PreparedAsyncNativeFunction::Pure(function) => {
                function(&self.args)
            }
            native_function_calls::PreparedAsyncNativeFunction::Host(_) => Box::pin(async {
                Err(VmError::new(VmErrorKind::TypeMismatch {
                    operation: "async host native invocation",
                }))
            }),
            native_function_calls::PreparedAsyncNativeFunction::HostMethod { .. } => {
                Box::pin(async {
                    Err(VmError::new(VmErrorKind::TypeMismatch {
                        operation: "async host method invocation",
                    }))
                })
            }
            native_function_calls::PreparedAsyncNativeFunction::DirectHostMethod { .. } => {
                Box::pin(async {
                    Err(VmError::new(VmErrorKind::TypeMismatch {
                        operation: "async direct host method invocation",
                    }))
                })
            }
            native_function_calls::PreparedAsyncNativeFunction::DirectHostFunction { .. } => {
                Box::pin(async {
                    Err(VmError::new(VmErrorKind::TypeMismatch {
                        operation: "async direct host function invocation",
                    }))
                })
            }
        }
    }

    #[must_use]
    pub fn requires_host(&self) -> bool {
        matches!(
            self.function,
            native_function_calls::PreparedAsyncNativeFunction::Host(_)
                | native_function_calls::PreparedAsyncNativeFunction::HostMethod { .. }
        )
    }

    #[must_use]
    pub fn requires_host_lease(&self) -> bool {
        matches!(
            self.function,
            native_function_calls::PreparedAsyncNativeFunction::DirectHostMethod { .. }
                | native_function_calls::PreparedAsyncNativeFunction::DirectHostFunction { .. }
        )
    }

    #[must_use]
    pub fn requires_host_lease_set(&self) -> bool {
        matches!(
            self.function,
            native_function_calls::PreparedAsyncNativeFunction::DirectHostFunction { .. }
        )
    }

    pub fn invoke_with_host<'call, 'host>(
        &'call self,
        host: &'call mut HostExecution<'host>,
        budget: Option<&'call mut ExecutionBudget>,
    ) -> NativeCallFuture<'call> {
        match &self.function {
            native_function_calls::PreparedAsyncNativeFunction::Pure(function) => {
                function(&self.args)
            }
            native_function_calls::PreparedAsyncNativeFunction::Host(function) => {
                function(&self.args, host, budget)
            }
            native_function_calls::PreparedAsyncNativeFunction::HostMethod {
                function,
                receiver,
            } => function(receiver, &self.args, host, budget),
            native_function_calls::PreparedAsyncNativeFunction::DirectHostMethod { .. } => {
                Box::pin(async {
                    Err(VmError::new(VmErrorKind::TypeMismatch {
                        operation: "async direct host method invocation",
                    }))
                })
            }
            native_function_calls::PreparedAsyncNativeFunction::DirectHostFunction { .. } => {
                Box::pin(async {
                    Err(VmError::new(VmErrorKind::TypeMismatch {
                        operation: "async direct host function invocation",
                    }))
                })
            }
        }
    }

    #[must_use]
    pub fn host_lease_request(
        &self,
    ) -> Option<(vela_host::path::HostRef, vela_host::lease::HostLeaseKind)> {
        let native_function_calls::PreparedAsyncNativeFunction::DirectHostMethod {
            receiver,
            lease_kind,
            ..
        } = &self.function
        else {
            return None;
        };
        receiver
            .segments
            .is_empty()
            .then_some((receiver.root, *lease_kind))
    }

    pub fn invoke_with_host_lease<'host>(
        &self,
        lease: vela_host::lease::ErasedHostLease<'host>,
    ) -> NativeCallFuture<'host> {
        let native_function_calls::PreparedAsyncNativeFunction::DirectHostMethod {
            function,
            receiver,
            ..
        } = &self.function
        else {
            return Box::pin(async {
                Err(VmError::new(VmErrorKind::TypeMismatch {
                    operation: "async host lease invocation",
                }))
            });
        };
        function(receiver.root, lease, self.args.clone())
    }

    #[must_use]
    pub fn host_lease_requests(
        &self,
    ) -> Option<Vec<(vela_host::path::HostRef, vela_host::lease::HostLeaseKind)>> {
        match &self.function {
            native_function_calls::PreparedAsyncNativeFunction::DirectHostMethod {
                receiver,
                lease_kind,
                ..
            } if receiver.segments.is_empty() => Some(vec![(receiver.root, *lease_kind)]),
            native_function_calls::PreparedAsyncNativeFunction::DirectHostFunction {
                requests,
                ..
            } => Some(requests.clone()),
            _ => None,
        }
    }

    pub fn invoke_with_host_leases<'invoke, 'lease>(
        &'invoke self,
        leases: &'invoke mut [vela_host::lease::ErasedHostLease<'lease>],
    ) -> NativeCallFuture<'invoke> {
        match &self.function {
            native_function_calls::PreparedAsyncNativeFunction::DirectHostFunction {
                function,
                ..
            } => function(leases, self.args.clone()),
            _ => Box::pin(async {
                Err(VmError::new(VmErrorKind::TypeMismatch {
                    operation: "async multiple host lease invocation",
                }))
            }),
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl Vm {
    pub fn resume_linked_async_call(
        &self,
        session: &mut LinkedExecutionSession,
        result: VmResult<OwnedValue>,
        heap: Option<&mut HeapExecution<'_>>,
        budget: Option<&mut ExecutionBudget>,
    ) -> VmResult<()> {
        self.resume_linked_native_call(session, result, heap, budget, "async")
    }

    pub fn resume_linked_context_call(
        &self,
        session: &mut LinkedExecutionSession,
        result: VmResult<OwnedValue>,
        heap: Option<&mut HeapExecution<'_>>,
        budget: Option<&mut ExecutionBudget>,
    ) -> VmResult<()> {
        self.resume_linked_native_call(session, result, heap, budget, "context")
    }

    fn resume_linked_native_call(
        &self,
        session: &mut LinkedExecutionSession,
        result: VmResult<OwnedValue>,
        heap: Option<&mut HeapExecution<'_>>,
        budget: Option<&mut ExecutionBudget>,
        boundary: &'static str,
    ) -> VmResult<()> {
        let pending = session.pending_native.pop().ok_or_else(|| {
            VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
                opcode: match boundary {
                    "async" => "async resume without a pending invocation",
                    _ => "context resume without a pending invocation",
                },
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
                .expect("pending async invocation retains an active frame")
                .registers
                .write(destination, value)?;
        }
        Ok(())
    }
}
