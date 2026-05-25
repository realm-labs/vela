use vela_host::{PatchTx, ScriptStateAdapter};
use vela_vm::{ExecutionBudget, HostExecution, VmResult};

use crate::{Engine, PermissionSet};

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
    pub fn permissions(&self) -> &PermissionSet {
        self.engine.permissions()
    }

    #[must_use]
    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions().contains(permission)
    }

    pub fn adapter(&mut self) -> &mut dyn ScriptStateAdapter {
        self.host.adapter
    }

    pub fn tx(&mut self) -> &mut PatchTx {
        self.host.tx
    }

    pub fn charge_instructions(&mut self, instructions: u64) -> VmResult<()> {
        if let Some(budget) = self.budget.as_deref_mut() {
            budget.charge_instructions(instructions)?;
        }
        Ok(())
    }

    #[must_use]
    pub fn budget(&self) -> Option<&ExecutionBudget> {
        self.budget.as_deref()
    }
}
