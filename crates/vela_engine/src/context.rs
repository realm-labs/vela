use vela_vm::budget::ExecutionBudget;
use vela_vm::error::VmResult;
use vela_vm::owned_value::OwnedValue;

use crate::engine::Engine;
use crate::permission::{Capability, CapabilitySet};
use crate::runtime::handles::{RuntimeCallTargetKind, RuntimeMethodSelectorKind};
use crate::runtime::{
    CallArgs, RuntimeCallTarget, RuntimeMethodSelector, VelaMethodTarget, VelaValue,
};

pub(crate) trait NativeReentry: Send {
    fn binding_schema(&self) -> &vela_bytecode::RustBindingSchema;

    fn value_to_owned(&mut self, value: &VelaValue) -> VmResult<OwnedValue>;
    fn budget(&self) -> Option<&ExecutionBudget>;

    fn budget_mut(&mut self) -> Option<&mut ExecutionBudget>;

    fn call(&mut self, target: RuntimeCallTargetKind, args: CallArgs) -> VmResult<VelaValue>;

    fn bind_method(
        &mut self,
        receiver: &VelaValue,
        method: RuntimeMethodSelectorKind,
    ) -> VmResult<VelaMethodTarget>;
}
pub struct NativeCallContext<'ctx> {
    engine: &'ctx Engine,
    budget: Option<&'ctx mut ExecutionBudget>,
    reentry: Option<&'ctx mut dyn NativeReentry>,
    effect_ceiling: CapabilitySet,
}
impl<'ctx> NativeCallContext<'ctx> {
    pub(crate) fn new(
        engine: &'ctx Engine,
        budget: Option<&'ctx mut ExecutionBudget>,
        reentry: Option<&'ctx mut dyn NativeReentry>,
        effect_ceiling: CapabilitySet,
    ) -> Self {
        Self {
            engine,
            budget,
            reentry,
            effect_ceiling,
        }
    }

    pub(crate) fn new_reentry(
        engine: &'ctx Engine,
        reentry: &'ctx mut dyn NativeReentry,
        effect_ceiling: CapabilitySet,
    ) -> Self {
        Self {
            engine,
            budget: None,
            reentry: Some(reentry),
            effect_ceiling,
        }
    }

    pub fn call<T>(&mut self, target: T, args: CallArgs) -> VmResult<VelaValue>
    where
        T: RuntimeCallTarget,
    {
        let target = crate::runtime::handles::call_target_sealed::Sealed::into_call_target(target);
        self.reentry
            .as_deref_mut()
            .ok_or_else(reentry_unavailable)?
            .call(target, args)
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
}

fn reentry_unavailable() -> vela_vm::error::VmError {
    vela_vm::error::VmError::new(vela_vm::error::VmErrorKind::TypeMismatch {
        operation: "native call context reentry outside an active execution",
    })
}
