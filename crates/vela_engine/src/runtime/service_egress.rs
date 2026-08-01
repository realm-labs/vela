use std::sync::Arc;

use vela_vm::error::{VmError, VmErrorKind, VmResult};

use super::image::RuntimeImageStorage;
use super::{
    CallArgs, CallOptions, DirectHostIdentity, RuntimeCallFuture, RuntimeImpl, RuntimeServiceCall,
    ServiceScopedReturn, ServiceScopedReturnEnvelope, VelaValue, handles,
};

#[doc(hidden)]
#[derive(Clone)]
pub struct ServiceScopedReturnEgress {
    pub(super) identity: DirectHostIdentity,
    pub(super) envelope: ServiceScopedReturnEnvelope,
}

impl ServiceScopedReturnEgress {
    #[must_use]
    pub fn new(
        identity: &DirectHostIdentity,
        envelope: ServiceScopedReturnEnvelope,
    ) -> ServiceScopedReturnEgress {
        Self {
            identity: identity.clone(),
            envelope,
        }
    }
}

impl<I> RuntimeImpl<I>
where
    I: RuntimeImageStorage,
{
    pub(crate) fn call_service_stable_function<'host>(
        &mut self,
        function: vela_def::FunctionId,
        diagnostic_name: impl Into<String>,
        args: CallArgs<'host>,
        options: CallOptions,
        execution: crate::service::PinnedServiceExecution,
    ) -> VmResult<VelaValue> {
        self.call_impl_with_service_egress(
            handles::StableVelaFunction {
                function,
                diagnostic_name: diagnostic_name.into(),
            },
            args,
            options,
            false,
            RuntimeServiceCall {
                dispatcher: Some(Arc::clone(execution.dispatcher())),
                pinned: Some(execution),
                scoped_return: None,
            },
        )
    }

    #[doc(hidden)]
    pub fn call_service_stable_scoped_function<'host>(
        &mut self,
        function: vela_def::FunctionId,
        diagnostic_name: impl Into<String>,
        args: CallArgs<'host>,
        options: CallOptions,
        execution: crate::service::PinnedServiceExecution,
        egress: ServiceScopedReturnEgress,
    ) -> VmResult<ServiceScopedReturn> {
        egress.identity.prepare_scoped_return();
        let identity = egress.identity.clone();
        self.call_impl_with_service_egress(
            handles::StableVelaFunction {
                function,
                diagnostic_name: diagnostic_name.into(),
            },
            args,
            options,
            false,
            RuntimeServiceCall {
                dispatcher: Some(Arc::clone(execution.dispatcher())),
                pinned: Some(execution),
                scoped_return: Some(egress),
            },
        )?;
        identity.take_scoped_return().ok_or_else(|| {
            VmError::new(VmErrorKind::TypeMismatch {
                operation: "service scoped return handoff",
            })
        })
    }

    pub(crate) fn call_service_stable_function_async<'call, 'args>(
        &'call mut self,
        function: vela_def::FunctionId,
        diagnostic_name: impl Into<String>,
        args: CallArgs<'args>,
        options: CallOptions,
        execution: crate::service::PinnedServiceExecution,
    ) -> RuntimeCallFuture<'call>
    where
        'args: 'call,
    {
        let diagnostic_name = diagnostic_name.into();
        let policy = options.call_policy();
        RuntimeCallFuture::new_controlled(
            async move {
                self.call_impl_async(
                    handles::StableVelaFunction {
                        function,
                        diagnostic_name,
                    },
                    args,
                    options,
                    Some(execution),
                )
                .await
            },
            policy,
        )
    }
}
