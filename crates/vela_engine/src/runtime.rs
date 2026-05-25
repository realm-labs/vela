use vela_bytecode::Program;
use vela_host::{PatchTx, ScriptStateAdapter};
use vela_hot_reload::{
    HotReloadReport, HotReloadResult, HotReloadRuntime, HotUpdate, ProgramVersion,
};
use vela_vm::{ExecutionBudget, HostExecution, Value, VmResult};

use crate::{Engine, EngineError, EngineErrorKind, EngineResult};

#[derive(Clone)]
pub struct Runtime {
    engine: Engine,
    program: Program,
    hot_reload: Option<HotReloadRuntime>,
}

impl Runtime {
    #[must_use]
    pub fn new(engine: Engine, program: Program) -> Self {
        Self {
            engine,
            program,
            hot_reload: None,
        }
    }

    #[must_use]
    pub fn from_hot_reload_version(engine: Engine, version: ProgramVersion) -> Self {
        Self {
            engine,
            program: version.to_program(),
            hot_reload: Some(HotReloadRuntime::new(version)),
        }
    }

    #[must_use]
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    #[must_use]
    pub fn program(&self) -> &Program {
        &self.program
    }

    #[must_use]
    pub fn hot_reload_version(&self) -> Option<std::sync::Arc<ProgramVersion>> {
        self.hot_reload.as_ref().map(HotReloadRuntime::current)
    }

    pub fn apply_hot_update(&mut self, update: HotUpdate) -> EngineResult<HotReloadReport> {
        self.apply_hot_update_result_report(Ok(update))
    }

    pub fn apply_hot_update_result_report(
        &mut self,
        update: HotReloadResult<HotUpdate>,
    ) -> EngineResult<HotReloadReport> {
        let Some(hot_reload) = self.hot_reload.as_mut() else {
            return Err(EngineError::new(
                EngineErrorKind::RuntimeNotHotReloadEnabled,
            ));
        };
        let report = hot_reload.apply_hot_update_result_report(update);
        if let Some(version) = report.version() {
            self.program = version.to_program();
        }
        Ok(report)
    }

    pub fn call(
        &mut self,
        entry: &str,
        args: &[Value],
        options: CallOptions,
        adapter: &mut dyn ScriptStateAdapter,
        tx: &mut PatchTx,
    ) -> VmResult<Value> {
        let mut budget = options.budget();
        let mut host = HostExecution { adapter, tx };
        let vm = self.engine.into_vm();
        if options.managed_heap {
            vm.run_program_with_host_managed_heap_and_budget(
                &self.program,
                entry,
                args,
                &mut host,
                &mut budget,
            )
        } else {
            vm.run_program_with_host_and_budget(&self.program, entry, args, &mut host, &mut budget)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallOptions {
    pub instruction_budget: u64,
    pub memory_budget: usize,
    pub call_depth: usize,
    pub patch_budget: usize,
    pub managed_heap: bool,
}

impl CallOptions {
    #[must_use]
    pub const fn new(
        instruction_budget: u64,
        memory_budget: usize,
        call_depth: usize,
        patch_budget: usize,
    ) -> Self {
        Self {
            instruction_budget,
            memory_budget,
            call_depth,
            patch_budget,
            managed_heap: true,
        }
    }

    #[must_use]
    pub const fn gameplay() -> Self {
        Self::new(50_000, 4 * 1024 * 1024, 64, 1024)
    }

    #[must_use]
    pub const fn unbounded() -> Self {
        Self::new(u64::MAX, usize::MAX, usize::MAX, usize::MAX)
    }

    #[must_use]
    pub const fn with_managed_heap(mut self, managed_heap: bool) -> Self {
        self.managed_heap = managed_heap;
        self
    }

    fn budget(&self) -> ExecutionBudget {
        ExecutionBudget::new(
            self.instruction_budget,
            self.memory_budget,
            self.call_depth,
            self.patch_budget,
        )
    }
}

impl Default for CallOptions {
    fn default() -> Self {
        Self::gameplay()
    }
}
