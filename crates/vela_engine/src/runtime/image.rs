use std::sync::Arc;

use vela_bytecode::Program;
use vela_hot_reload::error::HotReloadResult;
use vela_hot_reload::report::HotReloadReport;
use vela_hot_reload::runtime::HotReloadRuntime;
use vela_hot_reload::symbol::ProgramVersionId;
use vela_hot_reload::version::{HotUpdate, ProgramVersion};

use crate::engine::Engine;

pub(super) struct RuntimeImage {
    engine: Engine,
    program: Program,
    hot_reload: Option<HotReloadRuntime>,
}

impl RuntimeImage {
    pub(super) fn new(engine: Engine, program: Program) -> Self {
        Self {
            engine,
            program,
            hot_reload: None,
        }
    }

    pub(super) fn from_hot_reload_version(engine: Engine, version: ProgramVersion) -> Self {
        let program = version.to_program();
        Self {
            engine,
            program,
            hot_reload: Some(HotReloadRuntime::new(version)),
        }
    }

    pub(super) const fn engine(&self) -> &Engine {
        &self.engine
    }

    pub(super) const fn program(&self) -> &Program {
        &self.program
    }

    pub(super) fn global_names(&self) -> &[String] {
        self.program.global_names()
    }

    pub(super) fn hot_reload(&self) -> Option<&HotReloadRuntime> {
        self.hot_reload.as_ref()
    }

    pub(super) fn hot_reload_mut(&mut self) -> Option<&mut HotReloadRuntime> {
        self.hot_reload.as_mut()
    }

    pub(super) fn hot_reload_version(&self) -> Option<Arc<ProgramVersion>> {
        self.hot_reload.as_ref().map(HotReloadRuntime::current)
    }

    pub(super) fn current_program_version_id(&self) -> Option<ProgramVersionId> {
        self.hot_reload.as_ref().map(|runtime| runtime.current().id)
    }

    pub(super) fn apply_hot_update_result_report(
        &mut self,
        update: HotReloadResult<HotUpdate>,
    ) -> Option<HotReloadReport> {
        let hot_reload = self.hot_reload.as_mut()?;
        let report = hot_reload.apply_hot_update_result_report(update);
        if let Some(version) = report.version() {
            self.program = version.to_program();
        }
        Some(report)
    }

    pub(super) fn check_reload(&mut self) -> Option<HotReloadReport> {
        let hot_reload = self.hot_reload.as_mut()?;
        let report = hot_reload.check_reload()?;
        if let Some(version) = report.version() {
            self.program = version.to_program();
        }
        Some(report)
    }
}
