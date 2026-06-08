use std::sync::Arc;

use vela_bytecode::Program;
use vela_hot_reload::error::HotReloadResult;
use vela_hot_reload::profile::ProgramProfile;
use vela_hot_reload::report::HotReloadReport;
use vela_hot_reload::runtime::HotReloadRuntime;
use vela_hot_reload::symbol::ProgramVersionId;
use vela_hot_reload::version::{HotUpdate, ProgramVersion};

use crate::engine::Engine;

pub(super) struct RuntimeImage {
    engine: Engine,
    program: Program,
    version_id: Option<ProgramVersionId>,
    layout: RuntimeImageLayout,
    #[allow(dead_code)]
    profile: Option<ProgramProfile>,
    hot_reload: Option<HotReloadRuntime>,
}

pub(super) struct RuntimeImageLayout {
    global_names: Vec<String>,
}

impl RuntimeImage {
    pub(super) fn new(engine: Engine, program: Program) -> Self {
        let layout = RuntimeImageLayout::from_global_names(program.global_names());
        Self {
            engine,
            program,
            version_id: None,
            layout,
            profile: None,
            hot_reload: None,
        }
    }

    pub(super) fn from_hot_reload_version(engine: Engine, version: ProgramVersion) -> Self {
        let version_id = Some(version.id);
        let layout = RuntimeImageLayout::from_global_names(version.global_names());
        let profile = Some(version.profile().clone());
        let program = version.to_program();
        Self {
            engine,
            program,
            version_id,
            layout,
            profile,
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
        self.layout.global_names()
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
        self.version_id
    }

    pub(super) fn apply_hot_update_result_report(
        &mut self,
        update: HotReloadResult<HotUpdate>,
    ) -> Option<HotReloadReport> {
        let hot_reload = self.hot_reload.as_mut()?;
        let report = hot_reload.apply_hot_update_result_report(update);
        if let Some(version) = report.version() {
            self.refresh_from_version(&version);
        }
        Some(report)
    }

    pub(super) fn check_reload(&mut self) -> Option<HotReloadReport> {
        let hot_reload = self.hot_reload.as_mut()?;
        let report = hot_reload.check_reload()?;
        if let Some(version) = report.version() {
            self.refresh_from_version(&version);
        }
        Some(report)
    }

    fn refresh_from_version(&mut self, version: &ProgramVersion) {
        self.program = version.to_program();
        self.version_id = Some(version.id);
        self.layout = RuntimeImageLayout::from_global_names(version.global_names());
        self.profile = Some(version.profile().clone());
    }
}

impl RuntimeImageLayout {
    fn from_global_names(names: &[String]) -> Self {
        Self {
            global_names: names.to_vec(),
        }
    }

    fn global_names(&self) -> &[String] {
        &self.global_names
    }
}
