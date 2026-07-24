use smallvec::SmallVec;
use vela_common::{HostMethodId, Span};
use vela_host::access::HostAccess;
use vela_host::adapter::ScriptStateAdapter;
use vela_host::lease::{ErasedHostLease, HostLeaseKind};
use vela_host::object::ScriptHostObject;
use vela_host::path::HostPath;
use vela_host::path::HostRef;
use vela_host::value::HostValue;
use vela_vm::HostExecution;
use vela_vm::budget::ExecutionBudget;
use vela_vm::error::VmResult;
use vela_vm::owned_value::OwnedValue;

use crate::engine::Engine;
use crate::native::EffectSet;
use crate::permission::{Capability, CapabilitySet};
use crate::runtime::handles::{RuntimeCallTargetKind, RuntimeMethodSelectorKind};
use crate::runtime::{
    CallArgs, RuntimeCallFuture, RuntimeCallTarget, RuntimeMethodSelector, VelaMethodTarget,
    VelaValue,
};

pub(crate) trait NativeReentry: Send {
    fn binding_schema(&self) -> &vela_bytecode::RustBindingSchema;

    fn value_to_owned(&mut self, value: &VelaValue) -> VmResult<OwnedValue>;

    fn adapter(&mut self) -> &mut dyn ScriptStateAdapter;

    fn access(&mut self) -> &mut HostAccess;

    fn host_execution(&mut self) -> HostExecution<'_>;

    fn budget(&self) -> Option<&ExecutionBudget>;

    fn budget_mut(&mut self) -> Option<&mut ExecutionBudget>;

    fn with_host_leases(
        &mut self,
        requests: &[(HostRef, HostLeaseKind)],
        effect_ceiling: CapabilitySet,
        invoke: &mut NativeContextLeaseInvoker<'_>,
    ) -> VmResult<()>;

    fn call<'args>(
        &mut self,
        target: RuntimeCallTargetKind,
        args: CallArgs<'args>,
    ) -> VmResult<VelaValue>;

    fn call_async<'call, 'args>(
        &'call mut self,
        target: RuntimeCallTargetKind,
        args: CallArgs<'args>,
    ) -> RuntimeCallFuture<'call>
    where
        'args: 'call;

    fn bind_method(
        &mut self,
        receiver: &VelaValue,
        method: RuntimeMethodSelectorKind,
    ) -> VmResult<VelaMethodTarget>;
}

pub(crate) type NativeContextLeaseInvoker<'invoke> =
    dyn for<'lease, 'nested_ctx, 'nested_host> FnMut(
            &mut [ErasedHostLease<'lease>],
            &mut NativeCallContext<'nested_ctx, 'nested_host>,
        ) -> VmResult<()>
        + 'invoke;

pub struct NativeCallContext<'ctx, 'host> {
    engine: &'ctx Engine,
    host: Option<&'ctx mut HostExecution<'host>>,
    budget: Option<&'ctx mut ExecutionBudget>,
    reentry: Option<&'ctx mut dyn NativeReentry>,
    host_provenance: ActiveHostProvenanceSet,
    effect_ceiling: CapabilitySet,
    service_dispatcher: Option<&'ctx dyn crate::service::ServiceCallDispatcher>,
}

#[derive(Clone, Copy)]
struct ActiveHostProvenance {
    root: HostRef,
    mode: HostLeaseKind,
    object_address: usize,
}

type ActiveHostProvenanceSet = SmallVec<[ActiveHostProvenance; 8]>;

impl<'ctx, 'host> NativeCallContext<'ctx, 'host> {
    pub(crate) fn new(
        engine: &'ctx Engine,
        host: &'ctx mut HostExecution<'host>,
        budget: Option<&'ctx mut ExecutionBudget>,
        reentry: Option<&'ctx mut dyn NativeReentry>,
        effect_ceiling: CapabilitySet,
    ) -> Self {
        Self {
            engine,
            host: Some(host),
            budget,
            reentry,
            host_provenance: ActiveHostProvenanceSet::new(),
            effect_ceiling,
            service_dispatcher: None,
        }
    }

    pub(crate) fn new_reentry(
        engine: &'ctx Engine,
        reentry: &'ctx mut dyn NativeReentry,
        effect_ceiling: CapabilitySet,
        service_dispatcher: Option<&'ctx dyn crate::service::ServiceCallDispatcher>,
    ) -> Self {
        Self {
            engine,
            host: None,
            budget: None,
            reentry: Some(reentry),
            host_provenance: ActiveHostProvenanceSet::new(),
            effect_ceiling,
            service_dispatcher,
        }
    }

    pub(crate) fn dispatch_service(
        &mut self,
        target: crate::service::ServiceCallTarget,
        args: &[OwnedValue],
    ) -> VmResult<OwnedValue> {
        let dispatcher = self.service_dispatcher.ok_or_else(|| {
            vela_vm::error::VmError::new(vela_vm::error::VmErrorKind::TypeMismatch {
                operation: "service call outside a pinned service generation",
            })
        })?;
        dispatcher.dispatch(target, args, self)
    }

    pub fn call<'args, T>(&mut self, target: T, args: CallArgs<'args>) -> VmResult<VelaValue>
    where
        T: RuntimeCallTarget,
    {
        let target = crate::runtime::handles::call_target_sealed::Sealed::into_call_target(target);
        self.reentry
            .as_deref_mut()
            .ok_or_else(reentry_unavailable)?
            .call(target, args)
    }

    pub fn call_async<'call, 'args, T>(
        &'call mut self,
        target: T,
        args: CallArgs<'args>,
    ) -> RuntimeCallFuture<'call>
    where
        T: RuntimeCallTarget + Send + 'call,
        'args: 'call,
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

    pub(crate) fn binding_schema(&self) -> Option<&vela_bytecode::RustBindingSchema> {
        self.reentry.as_deref().map(NativeReentry::binding_schema)
    }

    pub(crate) fn value_to_owned(&mut self, value: &VelaValue) -> VmResult<OwnedValue> {
        self.reentry
            .as_deref_mut()
            .ok_or_else(reentry_unavailable)?
            .value_to_owned(value)
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

    /// Checks a capability-scoped native operation against both the Runtime
    /// grant and the active Rust callable's declared effect ceiling.
    pub fn require_capability(&self, capability: Capability) -> VmResult<()> {
        if !self.capabilities().contains(capability) {
            return Err(vela_vm::error::VmError::new(
                vela_vm::error::VmErrorKind::PermissionDenied {
                    native: "NativeCallContext operation".to_owned(),
                    capability: capability.as_str().to_owned(),
                },
            ));
        }
        self.require_capabilities(
            "NativeCallContext operation",
            CapabilitySet::new().with(capability),
        )
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
        let engine = self.engine;
        let effect_ceiling = self.effect_ceiling;
        if let Some(host) = self.host.take() {
            let mut budget = self.budget.take();
            let result = invoke_context_with_host_leases(
                engine,
                host,
                &mut budget,
                requests,
                effect_ceiling,
                &mut invoke,
            );
            self.host = Some(host);
            self.budget = budget;
            return result;
        }
        let reentry = self
            .reentry
            .as_deref_mut()
            .ok_or_else(reentry_unavailable)?;
        let mut result = None;
        reentry.with_host_leases(requests, effect_ceiling, &mut |leases, context| {
            result = Some(invoke(leases, context));
            Ok(())
        })?;
        result.ok_or_else(|| {
            vela_vm::error::VmError::new(vela_vm::error::VmErrorKind::TypeMismatch {
                operation: "reentry host lease callback did not run",
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

    pub(crate) fn resolve_host_reborrow<T>(
        &self,
        value: &T,
        requested: HostLeaseKind,
    ) -> VmResult<HostRef>
    where
        T: ScriptHostObject,
    {
        let address = (value as *const T).cast::<()>() as usize;
        let actual_type = value.host_type_id();
        self.host_provenance
            .iter()
            .find(|provenance| {
                provenance.object_address == address
                    && provenance.root.type_id == actual_type
                    && (requested == HostLeaseKind::Shared
                        || provenance.mode == HostLeaseKind::Exclusive)
            })
            .map(|provenance| provenance.root)
            .ok_or_else(|| {
                vela_vm::error::VmError::new(vela_vm::error::VmErrorKind::TypeMismatch {
                    operation: "generated active host argument lacks live lease provenance",
                })
            })
    }

    pub(crate) fn set_host_provenance(
        &mut self,
        requests: &[(HostRef, HostLeaseKind)],
        leases: &[ErasedHostLease<'_>],
    ) {
        self.host_provenance = requests
            .iter()
            .zip(leases)
            .filter_map(|((root, mode), lease)| {
                lease
                    .object()
                    .lease_any()
                    .map(|object| ActiveHostProvenance {
                        root: *root,
                        mode: *mode,
                        object_address: (object as *const dyn std::any::Any).cast::<()>() as usize,
                    })
            })
            .collect();
    }

    pub fn read_path(&mut self, path: &HostPath, source_span: Option<Span>) -> VmResult<HostValue> {
        self.require_effects("NativeCallContext::read_path", EffectSet::host_read())?;
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
        self.require_effects("NativeCallContext::set_path", EffectSet::host_write())?;
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
        self.require_effects("NativeCallContext::add_path", EffectSet::host_write())?;
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
        self.require_effects("NativeCallContext::sub_path", EffectSet::host_write())?;
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
        self.require_effects("NativeCallContext::mul_path", EffectSet::host_write())?;
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
        self.require_effects("NativeCallContext::div_path", EffectSet::host_write())?;
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
        self.require_effects("NativeCallContext::rem_path", EffectSet::host_write())?;
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
        self.require_effects("NativeCallContext::push_path", EffectSet::host_write())?;
        let host = self.host_execution();
        host.access
            .push_diagnostic_path(host.adapter, path, value, source_span)?;
        Ok(())
    }

    pub fn remove_path(&mut self, path: HostPath, source_span: Option<Span>) -> VmResult<()> {
        self.require_effects("NativeCallContext::remove_path", EffectSet::host_write())?;
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
        self.require_effects("NativeCallContext::call_method", EffectSet::host_write())?;
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

    pub(crate) fn require_capabilities(
        &self,
        operation: &str,
        required: CapabilitySet,
    ) -> VmResult<()> {
        let available = if self.effect_ceiling.contains(Capability::HostWrite) {
            self.effect_ceiling.with(Capability::HostRead)
        } else {
            self.effect_ceiling
        };
        if available.contains_all(required) {
            return Ok(());
        }
        let capability = required
            .difference(available)
            .iter()
            .next()
            .expect("a capability is missing from the effect ceiling");
        Err(vela_vm::error::VmError::new(
            vela_vm::error::VmErrorKind::PermissionDenied {
                native: operation.to_owned(),
                capability: capability.as_str().to_owned(),
            },
        ))
    }

    fn require_effects(&self, operation: &str, effects: EffectSet) -> VmResult<()> {
        self.require_capabilities(operation, effects.required_capability_set())
    }
}

fn invoke_context_with_host_leases<R>(
    engine: &Engine,
    host: &mut HostExecution<'_>,
    budget: &mut Option<&mut ExecutionBudget>,
    requests: &[(HostRef, HostLeaseKind)],
    effect_ceiling: CapabilitySet,
    invoke: &mut impl for<'lease, 'nested_ctx, 'nested_host> FnMut(
        &mut [ErasedHostLease<'lease>],
        &mut NativeCallContext<'nested_ctx, 'nested_host>,
    ) -> VmResult<R>,
) -> VmResult<R> {
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
                    effect_ceiling,
                );
                nested.set_host_provenance(requests, leases);
                result = Some(invoke(leases, &mut nested));
                Ok(())
            })
    };
    lease_result?;
    result.ok_or_else(|| {
        vela_vm::error::VmError::new(vela_vm::error::VmErrorKind::TypeMismatch {
            operation: "host lease callback did not run",
        })
    })?
}

fn reentry_unavailable() -> vela_vm::error::VmError {
    vela_vm::error::VmError::new(vela_vm::error::VmErrorKind::TypeMismatch {
        operation: "native call context reentry outside an active async execution",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_arity_host_provenance_stays_inline() {
        let provenance = (0_u32..8)
            .map(|slot| ActiveHostProvenance {
                root: HostRef::new(
                    vela_common::HostTypeId::new(1),
                    vela_common::HostObjectId::new(u64::from(slot) + 1),
                    1,
                ),
                mode: HostLeaseKind::Shared,
                object_address: slot as usize,
            })
            .collect::<ActiveHostProvenanceSet>();

        assert_eq!(provenance.len(), 8);
        assert!(!provenance.spilled());
    }
}
