use vela_common::{HostMethodId, Span};
use vela_host::adapter::ScriptStateAdapter;
use vela_host::path::HostPath;
use vela_host::tx::PatchTx;
use vela_host::value::HostValue;
use vela_vm::HostExecution;
use vela_vm::budget::ExecutionBudget;
use vela_vm::error::VmResult;

use crate::engine::Engine;
use crate::permission::{Capability, CapabilitySet};

pub struct NativeCallContext<'ctx, 'host> {
    engine: &'ctx Engine,
    host: &'ctx mut HostExecution<'host>,
    budget: Option<&'ctx mut ExecutionBudget>,
}

impl<'ctx, 'host> NativeCallContext<'ctx, 'host> {
    pub(crate) fn new(
        engine: &'ctx Engine,
        host: &'ctx mut HostExecution<'host>,
        budget: Option<&'ctx mut ExecutionBudget>,
    ) -> Self {
        Self {
            engine,
            host,
            budget,
        }
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
        self.host.adapter
    }

    pub fn tx(&mut self) -> &mut PatchTx {
        self.host.tx
    }

    pub fn read_path(&mut self, path: &HostPath, source_span: Option<Span>) -> VmResult<HostValue> {
        Ok(self
            .host
            .tx
            .read_path_at(self.host.adapter, path, source_span)?)
    }

    pub fn charge_instructions(&mut self, instructions: u64) -> VmResult<()> {
        if let Some(budget) = self.budget.as_deref_mut() {
            budget.charge_instructions(instructions)?;
        }
        Ok(())
    }

    pub fn charge_memory_bytes(&mut self, bytes: usize) -> VmResult<()> {
        if let Some(budget) = self.budget.as_deref_mut() {
            budget.charge_memory_bytes(bytes)?;
        }
        Ok(())
    }

    pub fn reserve_patch(&self) -> VmResult<()> {
        if let Some(budget) = self.budget.as_deref() {
            budget.reserve_patch(self.host.tx.patches().len())?;
        }
        Ok(())
    }

    pub fn set_path(
        &mut self,
        path: HostPath,
        value: HostValue,
        source_span: Option<Span>,
    ) -> VmResult<()> {
        self.reserve_patch()?;
        self.host
            .tx
            .set_path(self.host.adapter, path, value, source_span)?;
        Ok(())
    }

    pub fn add_path(
        &mut self,
        path: HostPath,
        value: HostValue,
        source_span: Option<Span>,
    ) -> VmResult<()> {
        self.reserve_patch()?;
        self.host
            .tx
            .add_path(self.host.adapter, path, value, source_span)?;
        Ok(())
    }

    pub fn sub_path(
        &mut self,
        path: HostPath,
        value: HostValue,
        source_span: Option<Span>,
    ) -> VmResult<()> {
        self.reserve_patch()?;
        self.host
            .tx
            .sub_path(self.host.adapter, path, value, source_span)?;
        Ok(())
    }

    pub fn mul_path(
        &mut self,
        path: HostPath,
        value: HostValue,
        source_span: Option<Span>,
    ) -> VmResult<()> {
        self.reserve_patch()?;
        self.host
            .tx
            .mul_path(self.host.adapter, path, value, source_span)?;
        Ok(())
    }

    pub fn div_path(
        &mut self,
        path: HostPath,
        value: HostValue,
        source_span: Option<Span>,
    ) -> VmResult<()> {
        self.reserve_patch()?;
        self.host
            .tx
            .div_path(self.host.adapter, path, value, source_span)?;
        Ok(())
    }

    pub fn rem_path(
        &mut self,
        path: HostPath,
        value: HostValue,
        source_span: Option<Span>,
    ) -> VmResult<()> {
        self.reserve_patch()?;
        self.host
            .tx
            .rem_path(self.host.adapter, path, value, source_span)?;
        Ok(())
    }

    pub fn push_path(
        &mut self,
        path: HostPath,
        value: HostValue,
        source_span: Option<Span>,
    ) -> VmResult<()> {
        self.reserve_patch()?;
        self.host
            .tx
            .push_path(self.host.adapter, path, value, source_span)?;
        Ok(())
    }

    pub fn remove_path(&mut self, path: HostPath, source_span: Option<Span>) -> VmResult<()> {
        self.reserve_patch()?;
        self.host
            .tx
            .remove_path(self.host.adapter, path, source_span)?;
        Ok(())
    }

    pub fn call_method(
        &mut self,
        path: HostPath,
        method: HostMethodId,
        args: Vec<HostValue>,
        source_span: Option<Span>,
    ) -> VmResult<HostValue> {
        self.reserve_patch()?;
        Ok(self
            .host
            .tx
            .call_method(self.host.adapter, path, method, args, source_span)?)
    }

    #[must_use]
    pub fn budget(&self) -> Option<&ExecutionBudget> {
        self.budget.as_deref()
    }
}
