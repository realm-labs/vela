use std::path::Path;
use std::sync::Arc;

use vela_bytecode::LinkedArtifact;
use vela_common::SourceId;
use vela_hir::module_graph::{ModulePath, ModuleSource};
use vela_hot_reload::abi::HotReloadAbi;
use vela_hot_reload::compile::{initial_version_from_linked_artifact, update_from_linked_artifact};
use vela_hot_reload::version::{HotUpdate, ProgramVersion};

pub use source_error::{
    EngineHotReloadSourceError, EngineHotReloadSourceErrorKind, EngineHotReloadSourceResult,
};

use crate::engine::Engine;
use crate::source::{load_module_sources, load_module_sources_for_changed_file, read_source_text};

mod source_error;

impl Engine {
    #[must_use]
    pub fn hot_reload_abi(&self) -> HotReloadAbi {
        HotReloadAbi::from_registry(&self.registry())
    }

    pub fn compile_hot_reload_initial(
        &self,
        text: &str,
    ) -> EngineHotReloadSourceResult<ProgramVersion> {
        self.compile_hot_reload_initial_with_id(SourceId::new(1), text)
    }

    pub(crate) fn compile_hot_reload_initial_with_id(
        &self,
        source: SourceId,
        text: &str,
    ) -> EngineHotReloadSourceResult<ProgramVersion> {
        let sources = [ModuleSource::new(source, ModulePath::root(), text)];
        let artifact = self.compile_and_link_sources(&sources, true)?;
        initial_version_from_linked_artifact(self.hot_reload_abi(), artifact)
            .map_err(EngineHotReloadSourceError::hot_reload)
    }

    pub fn compile_hot_reload_update(
        &self,
        previous: &ProgramVersion,
        text: &str,
    ) -> EngineHotReloadSourceResult<HotUpdate> {
        self.compile_hot_reload_update_with_id(previous, SourceId::new(1), text)
    }

    pub(crate) fn compile_hot_reload_update_with_id(
        &self,
        previous: &ProgramVersion,
        source: SourceId,
        text: &str,
    ) -> EngineHotReloadSourceResult<HotUpdate> {
        let sources = [ModuleSource::new(source, ModulePath::root(), text)];
        let artifact = self.compile_and_link_sources(&sources, true)?;
        update_from_linked_artifact(
            previous,
            self.hot_reload_abi(),
            self.hot_reload_policy(),
            artifact,
        )
        .map_err(EngineHotReloadSourceError::hot_reload)
    }

    pub fn compile_hot_reload_initial_file(
        &self,
        path: impl AsRef<Path>,
    ) -> EngineHotReloadSourceResult<ProgramVersion> {
        let text = read_source_text(path.as_ref()).map_err(EngineHotReloadSourceError::source)?;
        self.compile_hot_reload_initial(&text)
    }

    pub fn compile_hot_reload_update_file(
        &self,
        previous: &ProgramVersion,
        path: impl AsRef<Path>,
    ) -> EngineHotReloadSourceResult<HotUpdate> {
        let text = read_source_text(path.as_ref()).map_err(EngineHotReloadSourceError::source)?;
        self.compile_hot_reload_update(previous, &text)
    }

    pub fn compile_hot_reload_initial_dir(
        &self,
        root: impl AsRef<Path>,
    ) -> EngineHotReloadSourceResult<ProgramVersion> {
        let sources =
            load_module_sources(root.as_ref()).map_err(EngineHotReloadSourceError::source)?;
        let artifact = self.compile_and_link_sources(&sources, false)?;
        initial_version_from_linked_artifact(self.hot_reload_abi(), artifact)
            .map_err(EngineHotReloadSourceError::hot_reload)
    }

    pub fn compile_hot_reload_update_dir(
        &self,
        previous: &ProgramVersion,
        root: impl AsRef<Path>,
    ) -> EngineHotReloadSourceResult<HotUpdate> {
        let sources =
            load_module_sources(root.as_ref()).map_err(EngineHotReloadSourceError::source)?;
        let artifact = self.compile_and_link_sources(&sources, false)?;
        update_from_linked_artifact(
            previous,
            self.hot_reload_abi(),
            self.hot_reload_policy(),
            artifact,
        )
        .map_err(EngineHotReloadSourceError::hot_reload)
    }

    pub fn compile_hot_reload_update_changed_file(
        &self,
        previous: &ProgramVersion,
        root: impl AsRef<Path>,
        changed_file: impl AsRef<Path>,
    ) -> EngineHotReloadSourceResult<HotUpdate> {
        let sources = load_module_sources_for_changed_file(root.as_ref(), changed_file.as_ref())
            .map_err(EngineHotReloadSourceError::source)?;
        let artifact = self.compile_and_link_sources(&sources, false)?;
        update_from_linked_artifact(
            previous,
            self.hot_reload_abi(),
            self.hot_reload_policy(),
            artifact,
        )
        .map_err(EngineHotReloadSourceError::hot_reload)
    }

    fn compile_and_link_sources(
        &self,
        sources: &[ModuleSource],
        single_source: bool,
    ) -> EngineHotReloadSourceResult<Arc<LinkedArtifact>> {
        let program = self
            .compile_sources(sources, single_source)
            .map_err(EngineHotReloadSourceError::source)?;
        self.link_compiled_program(program)
            .map_err(EngineHotReloadSourceError::link)
    }
}
