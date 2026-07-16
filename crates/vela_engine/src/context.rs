use vela_common::{HostMethodId, Span};
use vela_host::access::HostAccess;
use vela_host::adapter::ScriptStateAdapter;
use vela_host::lease::{ErasedHostLease, HostLeaseKind};
use vela_host::path::HostPath;
use vela_host::path::HostRef;
use vela_host::value::HostValue;
use vela_vm::HostExecution;
use vela_vm::budget::ExecutionBudget;
use vela_vm::error::VmResult;

use crate::engine::Engine;
use crate::permission::{Capability, CapabilitySet};
use crate::runtime::handles::{RuntimeCallTargetKind, RuntimeMethodSelectorKind};
use crate::runtime::{
    CallArgs, RuntimeCallFuture, RuntimeCallTarget, RuntimeMethodSelector, VelaMethodTarget,
    VelaValue,
};

pub(crate) trait NativeReentry: Send {
    fn adapter(&mut self) -> &mut dyn ScriptStateAdapter;

    fn access(&mut self) -> &mut HostAccess;

    fn host_execution(&mut self) -> HostExecution<'_>;

    fn budget(&self) -> Option<&ExecutionBudget>;

    fn budget_mut(&mut self) -> Option<&mut ExecutionBudget>;

    fn call<'call>(
        &'call mut self,
        target: RuntimeCallTargetKind,
        args: CallArgs<'call>,
    ) -> VmResult<VelaValue>;

    fn call_async<'call>(
        &'call mut self,
        target: RuntimeCallTargetKind,
        args: CallArgs<'call>,
    ) -> RuntimeCallFuture<'call>;

    fn bind_method(
        &mut self,
        receiver: &VelaValue,
        method: RuntimeMethodSelectorKind,
    ) -> VmResult<VelaMethodTarget>;
}

pub struct NativeCallContext<'ctx, 'host> {
    engine: &'ctx Engine,
    host: Option<&'ctx mut HostExecution<'host>>,
    budget: Option<&'ctx mut ExecutionBudget>,
    reentry: Option<&'ctx mut dyn NativeReentry>,
}

impl<'ctx, 'host> NativeCallContext<'ctx, 'host> {
    pub(crate) fn new(
        engine: &'ctx Engine,
        host: &'ctx mut HostExecution<'host>,
        budget: Option<&'ctx mut ExecutionBudget>,
        reentry: Option<&'ctx mut dyn NativeReentry>,
    ) -> Self {
        Self {
            engine,
            host: Some(host),
            budget,
            reentry,
        }
    }

    pub(crate) fn new_reentry(engine: &'ctx Engine, reentry: &'ctx mut dyn NativeReentry) -> Self {
        Self {
            engine,
            host: None,
            budget: None,
            reentry: Some(reentry),
        }
    }

    pub fn call<'call, T>(&'call mut self, target: T, args: CallArgs<'call>) -> VmResult<VelaValue>
    where
        T: RuntimeCallTarget,
    {
        let target = crate::runtime::handles::call_target_sealed::Sealed::into_call_target(target);
        self.reentry
            .as_deref_mut()
            .ok_or_else(reentry_unavailable)?
            .call(target, args)
    }

    pub fn call_async<'call, T>(
        &'call mut self,
        target: T,
        args: CallArgs<'call>,
    ) -> RuntimeCallFuture<'call>
    where
        T: RuntimeCallTarget + Send + 'call,
    {
        let target = crate::runtime::handles::call_target_sealed::Sealed::into_call_target(target);
        match self.reentry.as_deref_mut() {
            Some(reentry) => reentry.call_async(target, args),
            None => RuntimeCallFuture::new(async { Err(reentry_unavailable()) }),
        }
    }

    pub fn bind_method<T>(&mut self, receiver: &VelaValue, method: T) -> VmResult<VelaMethodTarget>
    where
        T: RuntimeMethodSelector,
    {
        let method =
            crate::runtime::handles::method_selector_sealed::Sealed::into_method_selector(method);
        self.reentry
            .as_deref_mut()
            .ok_or_else(reentry_unavailable)?
            .bind_method(receiver, method)
    }

    #[must_use]
    pub fn engine(&self) -> &Engine {
        self.engine
    }

    #[must_use]
    pub fn capabilities(&self) -> CapabilitySet {
        self.engine.capabilities()
    }

    #[must_use]
    pub fn has_capability(&self, capability: Capability) -> bool {
        self.capabilities().contains(capability)
    }

    pub fn adapter(&mut self) -> &mut dyn ScriptStateAdapter {
        match self.host.as_deref_mut() {
            Some(host) => host.adapter,
            None => self
                .reentry
                .as_deref_mut()
                .expect("reentrant native context has execution state")
                .adapter(),
        }
    }

    /// Executes a generated ordinary Rust context export while retaining its
    /// complete exact-object lease set. The nested context reuses the same
    /// engine, access gates, VM state, and budget; only the adapter reborrow is
    /// narrowed to the lease callback.
    #[doc(hidden)]
    pub fn with_host_leases<R>(
        &mut self,
        requests: &[(HostRef, HostLeaseKind)],
        mut invoke: impl for<'lease, 'nested_ctx, 'nested_host> FnMut(
            &mut [ErasedHostLease<'lease>],
            &mut NativeCallContext<'nested_ctx, 'nested_host>,
        ) -> VmResult<R>,
    ) -> VmResult<R> {
        let Some(host) = self.host.take() else {
            return Err(reentry_unavailable());
        };
        let engine = self.engine;
        let mut budget = self.budget.take();
        let mut result = None;
        let lease_result = {
            let access = &mut *host.access;
            let mut state_values = host.state_values.as_deref_mut();
            host.adapter
                .with_host_leases(requests, &mut |leases, leased_adapter| {
                    let mut leased_host = HostExecution {
                        adapter: leased_adapter,
                        access: &mut *access,
                        state_values: state_values.as_deref_mut(),
                    };
                    let mut nested = NativeCallContext::new(
                        engine,
                        &mut leased_host,
                        budget.as_deref_mut(),
                        None,
                    );
                    result = Some(invoke(leases, &mut nested));
                    Ok(())
                })
        };
        self.host = Some(host);
        self.budget = budget;
        lease_result?;
        result.ok_or_else(|| {
            vela_vm::error::VmError::new(vela_vm::error::VmErrorKind::TypeMismatch {
                operation: "host lease callback did not run",
            })
        })?
    }

    pub fn access(&mut self) -> &mut HostAccess {
        match self.host.as_deref_mut() {
            Some(host) => host.access,
            None => self
                .reentry
                .as_deref_mut()
                .expect("reentrant native context has execution state")
                .access(),
        }
    }

    pub fn read_path(&mut self, path: &HostPath, source_span: Option<Span>) -> VmResult<HostValue> {
        let host = self.host_execution();
        Ok(host
            .access
            .read_diagnostic_path_at(host.adapter, path, source_span)?)
    }

    pub fn charge_execution_units(&mut self, units: u64) -> VmResult<()> {
        if let Some(budget) = self.budget_mut() {
            budget.charge_execution_units(units)?;
        }
        Ok(())
    }

    pub fn charge_memory_bytes(&mut self, bytes: usize) -> VmResult<()> {
        if let Some(budget) = self.budget_mut() {
            budget.charge_memory_bytes(bytes)?;
        }
        Ok(())
    }

    pub fn set_path(
        &mut self,
        path: HostPath,
        value: HostValue,
        source_span: Option<Span>,
    ) -> VmResult<()> {
        let host = self.host_execution();
        host.access
            .write_diagnostic_path(host.adapter, path, value, source_span)?;
        Ok(())
    }

    pub fn add_path(
        &mut self,
        path: HostPath,
        value: HostValue,
        source_span: Option<Span>,
    ) -> VmResult<()> {
        let host = self.host_execution();
        host.access
            .add_diagnostic_path(host.adapter, path, value, source_span)?;
        Ok(())
    }

    pub fn sub_path(
        &mut self,
        path: HostPath,
        value: HostValue,
        source_span: Option<Span>,
    ) -> VmResult<()> {
        let host = self.host_execution();
        host.access
            .sub_diagnostic_path(host.adapter, path, value, source_span)?;
        Ok(())
    }

    pub fn mul_path(
        &mut self,
        path: HostPath,
        value: HostValue,
        source_span: Option<Span>,
    ) -> VmResult<()> {
        let host = self.host_execution();
        host.access
            .mul_diagnostic_path(host.adapter, path, value, source_span)?;
        Ok(())
    }

    pub fn div_path(
        &mut self,
        path: HostPath,
        value: HostValue,
        source_span: Option<Span>,
    ) -> VmResult<()> {
        let host = self.host_execution();
        host.access
            .div_diagnostic_path(host.adapter, path, value, source_span)?;
        Ok(())
    }

    pub fn rem_path(
        &mut self,
        path: HostPath,
        value: HostValue,
        source_span: Option<Span>,
    ) -> VmResult<()> {
        let host = self.host_execution();
        host.access
            .rem_diagnostic_path(host.adapter, path, value, source_span)?;
        Ok(())
    }

    pub fn push_path(
        &mut self,
        path: HostPath,
        value: HostValue,
        source_span: Option<Span>,
    ) -> VmResult<()> {
        let host = self.host_execution();
        host.access
            .push_diagnostic_path(host.adapter, path, value, source_span)?;
        Ok(())
    }

    pub fn remove_path(&mut self, path: HostPath, source_span: Option<Span>) -> VmResult<()> {
        let host = self.host_execution();
        host.access
            .remove_diagnostic_path(host.adapter, path, source_span)?;
        Ok(())
    }

    pub fn call_method(
        &mut self,
        path: HostPath,
        method: HostMethodId,
        args: Vec<HostValue>,
        source_span: Option<Span>,
    ) -> VmResult<HostValue> {
        let host = self.host_execution();
        Ok(host.access.call_diagnostic_path_method(
            host.adapter,
            path,
            method,
            args,
            source_span,
        )?)
    }

    #[must_use]
    pub fn budget(&self) -> Option<&ExecutionBudget> {
        self.budget
            .as_deref()
            .or_else(|| self.reentry.as_deref().and_then(NativeReentry::budget))
    }

    fn budget_mut(&mut self) -> Option<&mut ExecutionBudget> {
        match self.budget.as_deref_mut() {
            Some(budget) => Some(budget),
            None => self
                .reentry
                .as_deref_mut()
                .and_then(NativeReentry::budget_mut),
        }
    }

    fn host_execution(&mut self) -> HostExecution<'_> {
        match self.host.as_deref_mut() {
            Some(host) => HostExecution {
                adapter: &mut *host.adapter,
                access: &mut *host.access,
                state_values: host.state_values.as_deref_mut(),
            },
            None => self
                .reentry
                .as_deref_mut()
                .expect("reentrant native context has execution state")
                .host_execution(),
        }
    }
}

fn reentry_unavailable() -> vela_vm::error::VmError {
    vela_vm::error::VmError::new(vela_vm::error::VmErrorKind::TypeMismatch {
        operation: "native call context reentry outside an active async execution",
    })
}
