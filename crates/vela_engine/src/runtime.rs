use std::{fmt, path::Path};

use vela_bytecode::Program;
use vela_common::SourceId;
use vela_host::adapter::ScriptStateAdapter;
use vela_host::error::HostError;
use vela_host::tx::PatchTx;
use vela_hot_reload::error::HotReloadResult;
use vela_hot_reload::report::HotReloadReport;
use vela_hot_reload::runtime::HotReloadRuntime;
use vela_hot_reload::version::{HotUpdate, ProgramVersion};
use vela_vm::HostExecution;
use vela_vm::budget::ExecutionBudget;
use vela_vm::error::VmResult;
use vela_vm::owned_value::OwnedValue;

use crate::engine::Engine;
use crate::error::{EngineError, EngineErrorKind, EngineResult};
use crate::reload::{
    EngineHotReloadSourceError, EngineHotReloadSourceErrorKind, EngineHotReloadSourceResult,
};

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

    pub fn stage_hot_update(&mut self, update: HotUpdate) -> EngineResult<()> {
        self.stage_hot_update_result(Ok(update))
    }

    pub fn stage_hot_update_result(
        &mut self,
        update: HotReloadResult<HotUpdate>,
    ) -> EngineResult<()> {
        let Some(hot_reload) = self.hot_reload.as_mut() else {
            return Err(EngineError::new(
                EngineErrorKind::RuntimeNotHotReloadEnabled,
            ));
        };
        let _replaced = hot_reload.stage_hot_update_result(update);
        Ok(())
    }

    pub fn stage_hot_reload_update(&mut self, source: SourceId, text: &str) -> EngineResult<()> {
        let update = self.compile_hot_reload_update(source, text)?;
        self.stage_hot_update_result(update)
    }

    pub fn has_pending_hot_update(&self) -> EngineResult<bool> {
        let Some(hot_reload) = self.hot_reload.as_ref() else {
            return Err(EngineError::new(
                EngineErrorKind::RuntimeNotHotReloadEnabled,
            ));
        };
        Ok(hot_reload.has_pending_update())
    }

    pub fn check_reload(&mut self) -> EngineResult<Option<HotReloadReport>> {
        let Some(hot_reload) = self.hot_reload.as_mut() else {
            return Err(EngineError::new(
                EngineErrorKind::RuntimeNotHotReloadEnabled,
            ));
        };
        Ok(Self::consume_reload_report(&mut self.program, hot_reload))
    }

    pub fn check_reload_at_tick_boundary(&mut self) -> EngineResult<Option<HotReloadReport>> {
        self.check_reload()
    }

    pub fn apply_patch_tx_at_safe_point(
        &mut self,
        tx: PatchTx,
        adapter: &mut impl ScriptStateAdapter,
    ) -> Result<PatchApplySafePointReport, PatchApplySafePointError> {
        let before_apply_reload = self.check_optional_reload();
        if let Err(host_error) = tx.apply(adapter) {
            return Err(PatchApplySafePointError {
                host_error,
                before_apply_reload,
            });
        }
        let after_apply_reload = self.check_optional_reload();
        Ok(PatchApplySafePointReport {
            before_apply_reload,
            after_apply_reload,
        })
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

    pub fn compile_hot_reload_update(
        &self,
        source: SourceId,
        text: &str,
    ) -> EngineResult<HotReloadResult<HotUpdate>> {
        let previous = self.current_hot_reload_version()?;
        Ok(self
            .engine
            .compile_hot_reload_update(&previous, source, text))
    }

    pub fn compile_hot_reload_update_file(
        &self,
        path: impl AsRef<Path>,
    ) -> EngineResult<EngineHotReloadSourceResult<HotUpdate>> {
        let previous = self.current_hot_reload_version()?;
        Ok(self.engine.compile_hot_reload_update_file(&previous, path))
    }

    pub fn compile_hot_reload_update_dir(
        &self,
        root: impl AsRef<Path>,
    ) -> EngineResult<EngineHotReloadSourceResult<HotUpdate>> {
        let previous = self.current_hot_reload_version()?;
        Ok(self.engine.compile_hot_reload_update_dir(&previous, root))
    }

    pub fn compile_hot_reload_update_changed_file(
        &self,
        root: impl AsRef<Path>,
        changed_file: impl AsRef<Path>,
    ) -> EngineResult<EngineHotReloadSourceResult<HotUpdate>> {
        let previous = self.current_hot_reload_version()?;
        Ok(self
            .engine
            .compile_hot_reload_update_changed_file(&previous, root, changed_file))
    }

    pub fn stage_hot_reload_update_file(
        &mut self,
        path: impl AsRef<Path>,
    ) -> EngineResult<EngineHotReloadSourceResult<()>> {
        let previous = self.current_hot_reload_version()?;
        let update = self.engine.compile_hot_reload_update_file(&previous, path);
        self.stage_hot_reload_source_update_result(update)
    }

    pub fn stage_hot_reload_update_dir(
        &mut self,
        root: impl AsRef<Path>,
    ) -> EngineResult<EngineHotReloadSourceResult<()>> {
        let previous = self.current_hot_reload_version()?;
        let update = self.engine.compile_hot_reload_update_dir(&previous, root);
        self.stage_hot_reload_source_update_result(update)
    }

    pub fn stage_hot_reload_update_changed_file(
        &mut self,
        root: impl AsRef<Path>,
        changed_file: impl AsRef<Path>,
    ) -> EngineResult<EngineHotReloadSourceResult<()>> {
        let previous = self.current_hot_reload_version()?;
        let update =
            self.engine
                .compile_hot_reload_update_changed_file(&previous, root, changed_file);
        self.stage_hot_reload_source_update_result(update)
    }

    fn stage_hot_reload_source_update_result(
        &mut self,
        update: EngineHotReloadSourceResult<HotUpdate>,
    ) -> EngineResult<EngineHotReloadSourceResult<()>> {
        match update {
            Ok(update) => {
                self.stage_hot_update(update)?;
                Ok(Ok(()))
            }
            Err(error) => match error.kind {
                EngineHotReloadSourceErrorKind::Source(error) => {
                    Ok(Err(EngineHotReloadSourceError {
                        kind: EngineHotReloadSourceErrorKind::Source(error),
                    }))
                }
                EngineHotReloadSourceErrorKind::HotReload(error) => {
                    self.stage_hot_update_result(Err(error))?;
                    Ok(Ok(()))
                }
            },
        }
    }

    pub fn call(
        &mut self,
        entry: &str,
        args: &[OwnedValue],
        options: CallOptions,
        adapter: &mut dyn ScriptStateAdapter,
        tx: &mut PatchTx,
    ) -> VmResult<OwnedValue> {
        let mut budget = options.budget();
        let mut host = HostExecution { adapter, tx };
        let vm = if let Some(hot_reload) = &self.hot_reload {
            let current = hot_reload.current();
            self.engine
                .into_vm_for_program_with_abi(&self.program, current.abi())
        } else {
            self.engine.into_vm_for_program(&self.program)
        };
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

    pub fn call_at_event_end_safe_point(
        &mut self,
        entry: &str,
        args: &[OwnedValue],
        options: CallOptions,
        adapter: &mut dyn ScriptStateAdapter,
        tx: &mut PatchTx,
    ) -> VmResult<EventCallSafePointReport> {
        let value = self.call(entry, args, options, adapter, tx)?;
        let reload = self.check_optional_reload();
        Ok(EventCallSafePointReport { value, reload })
    }

    fn current_hot_reload_version(&self) -> EngineResult<std::sync::Arc<ProgramVersion>> {
        self.hot_reload
            .as_ref()
            .map(HotReloadRuntime::current)
            .ok_or_else(|| EngineError::new(EngineErrorKind::RuntimeNotHotReloadEnabled))
    }

    fn check_optional_reload(&mut self) -> Option<HotReloadReport> {
        let hot_reload = self.hot_reload.as_mut()?;
        Self::consume_reload_report(&mut self.program, hot_reload)
    }

    fn consume_reload_report(
        program: &mut Program,
        hot_reload: &mut HotReloadRuntime,
    ) -> Option<HotReloadReport> {
        let report = hot_reload.check_reload()?;
        if let Some(version) = report.version() {
            *program = version.to_program();
        }
        Some(report)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EventCallSafePointReport {
    pub value: OwnedValue,
    pub reload: Option<HotReloadReport>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PatchApplySafePointReport {
    pub before_apply_reload: Option<HotReloadReport>,
    pub after_apply_reload: Option<HotReloadReport>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PatchApplySafePointError {
    pub host_error: HostError,
    pub before_apply_reload: Option<HotReloadReport>,
}

impl fmt::Display for PatchApplySafePointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "host patch apply failed at safe point: {}",
            self.host_error
        )
    }
}

impl std::error::Error for PatchApplySafePointError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.host_error)
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
