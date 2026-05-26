use std::path::Path;

use vela_common::SourceId;
use vela_hot_reload::{
    HotReloadAbi, HotReloadResult, HotUpdate, ProgramVersion,
    compile_initial_modules_with_abi_and_options, compile_initial_with_abi_and_options,
    compile_update_modules_with_abi_and_options_and_policy,
    compile_update_with_abi_and_options_and_policy,
};

pub use source_error::{
    EngineHotReloadSourceError, EngineHotReloadSourceErrorKind, EngineHotReloadSourceResult,
};

use crate::Engine;
use crate::source::{load_module_sources, read_source_text};

mod source_error;

impl Engine {
    #[must_use]
    pub fn hot_reload_abi(&self) -> HotReloadAbi {
        HotReloadAbi::from_registry(&self.registry())
    }

    pub fn compile_hot_reload_initial(
        &self,
        source: SourceId,
        text: &str,
    ) -> HotReloadResult<ProgramVersion> {
        compile_initial_with_abi_and_options(
            source,
            text,
            self.hot_reload_abi(),
            &self.compiler_options(),
        )
    }

    pub fn compile_hot_reload_update(
        &self,
        previous: &ProgramVersion,
        source: SourceId,
        text: &str,
    ) -> HotReloadResult<HotUpdate> {
        compile_update_with_abi_and_options_and_policy(
            previous,
            source,
            text,
            self.hot_reload_abi(),
            &self.compiler_options(),
            self.hot_reload_policy(),
        )
    }

    pub fn compile_hot_reload_initial_file(
        &self,
        path: impl AsRef<Path>,
    ) -> EngineHotReloadSourceResult<ProgramVersion> {
        let text = read_source_text(path.as_ref()).map_err(EngineHotReloadSourceError::source)?;
        self.compile_hot_reload_initial(SourceId::new(1), &text)
            .map_err(EngineHotReloadSourceError::hot_reload)
    }

    pub fn compile_hot_reload_update_file(
        &self,
        previous: &ProgramVersion,
        path: impl AsRef<Path>,
    ) -> EngineHotReloadSourceResult<HotUpdate> {
        let text = read_source_text(path.as_ref()).map_err(EngineHotReloadSourceError::source)?;
        self.compile_hot_reload_update(previous, SourceId::new(1), &text)
            .map_err(EngineHotReloadSourceError::hot_reload)
    }

    pub fn compile_hot_reload_initial_dir(
        &self,
        root: impl AsRef<Path>,
    ) -> EngineHotReloadSourceResult<ProgramVersion> {
        let sources =
            load_module_sources(root.as_ref()).map_err(EngineHotReloadSourceError::source)?;
        compile_initial_modules_with_abi_and_options(
            &sources,
            self.hot_reload_abi(),
            &self.compiler_options(),
        )
        .map_err(EngineHotReloadSourceError::hot_reload)
    }

    pub fn compile_hot_reload_update_dir(
        &self,
        previous: &ProgramVersion,
        root: impl AsRef<Path>,
    ) -> EngineHotReloadSourceResult<HotUpdate> {
        let sources =
            load_module_sources(root.as_ref()).map_err(EngineHotReloadSourceError::source)?;
        compile_update_modules_with_abi_and_options_and_policy(
            previous,
            &sources,
            self.hot_reload_abi(),
            &self.compiler_options(),
            self.hot_reload_policy(),
        )
        .map_err(EngineHotReloadSourceError::hot_reload)
    }
}
