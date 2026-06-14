use std::path::Path;

use vela_bytecode::compiler::{
    compile_module_sources_with_options_and_registry,
    compile_program_source_with_options_and_registry,
};
use vela_common::SourceId;
use vela_hot_reload::abi::HotReloadAbi;
use vela_hot_reload::compile::{initial_version_from_linked_program, update_from_linked_program};
use vela_hot_reload::error::{HotReloadError, HotReloadErrorKind, HotReloadResult};
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

    pub fn compile_hot_reload_initial(&self, text: &str) -> HotReloadResult<ProgramVersion> {
        self.compile_hot_reload_initial_with_id(SourceId::new(1), text)
    }

    pub(crate) fn compile_hot_reload_initial_with_id(
        &self,
        source: SourceId,
        text: &str,
    ) -> HotReloadResult<ProgramVersion> {
        let program = compile_program_source_with_options_and_registry(
            source,
            text,
            &self.compiler_options(),
            self.compiler_registry(),
        )
        .map_err(|error| HotReloadError {
            kind: HotReloadErrorKind::Compile(error),
        })?;
        let linked_program = self.link_program(&program).map_err(HotReloadError::from)?;
        Ok(initial_version_from_linked_program(
            program,
            self.hot_reload_abi(),
            linked_program,
        ))
    }

    pub fn compile_hot_reload_update(
        &self,
        previous: &ProgramVersion,
        text: &str,
    ) -> HotReloadResult<HotUpdate> {
        self.compile_hot_reload_update_with_id(previous, SourceId::new(1), text)
    }

    pub(crate) fn compile_hot_reload_update_with_id(
        &self,
        previous: &ProgramVersion,
        source: SourceId,
        text: &str,
    ) -> HotReloadResult<HotUpdate> {
        let program = compile_program_source_with_options_and_registry(
            source,
            text,
            &self.compiler_options(),
            self.compiler_registry(),
        )
        .map_err(|error| HotReloadError {
            kind: HotReloadErrorKind::Compile(error),
        })?;
        let linked_program = self.link_program(&program).map_err(HotReloadError::from)?;
        update_from_linked_program(
            previous,
            program,
            self.hot_reload_abi(),
            self.hot_reload_policy(),
            linked_program,
        )
    }

    pub fn compile_hot_reload_initial_file(
        &self,
        path: impl AsRef<Path>,
    ) -> EngineHotReloadSourceResult<ProgramVersion> {
        let text = read_source_text(path.as_ref()).map_err(EngineHotReloadSourceError::source)?;
        self.compile_hot_reload_initial(&text)
            .map_err(EngineHotReloadSourceError::hot_reload)
    }

    pub fn compile_hot_reload_update_file(
        &self,
        previous: &ProgramVersion,
        path: impl AsRef<Path>,
    ) -> EngineHotReloadSourceResult<HotUpdate> {
        let text = read_source_text(path.as_ref()).map_err(EngineHotReloadSourceError::source)?;
        self.compile_hot_reload_update(previous, &text)
            .map_err(EngineHotReloadSourceError::hot_reload)
    }

    pub fn compile_hot_reload_initial_dir(
        &self,
        root: impl AsRef<Path>,
    ) -> EngineHotReloadSourceResult<ProgramVersion> {
        let sources =
            load_module_sources(root.as_ref()).map_err(EngineHotReloadSourceError::source)?;
        let program = compile_module_sources_with_options_and_registry(
            &sources,
            &self.compiler_options(),
            self.compiler_registry(),
        )
        .map_err(|error| {
            EngineHotReloadSourceError::hot_reload(HotReloadError {
                kind: HotReloadErrorKind::Compile(error),
            })
        })?;
        let linked_program = self
            .link_program(&program)
            .map_err(HotReloadError::from)
            .map_err(EngineHotReloadSourceError::hot_reload)?;
        Ok(initial_version_from_linked_program(
            program,
            self.hot_reload_abi(),
            linked_program,
        ))
    }

    pub fn compile_hot_reload_update_dir(
        &self,
        previous: &ProgramVersion,
        root: impl AsRef<Path>,
    ) -> EngineHotReloadSourceResult<HotUpdate> {
        let sources =
            load_module_sources(root.as_ref()).map_err(EngineHotReloadSourceError::source)?;
        let program = compile_module_sources_with_options_and_registry(
            &sources,
            &self.compiler_options(),
            self.compiler_registry(),
        )
        .map_err(|error| {
            EngineHotReloadSourceError::hot_reload(HotReloadError {
                kind: HotReloadErrorKind::Compile(error),
            })
        })?;
        let linked_program = self
            .link_program(&program)
            .map_err(HotReloadError::from)
            .map_err(EngineHotReloadSourceError::hot_reload)?;
        update_from_linked_program(
            previous,
            program,
            self.hot_reload_abi(),
            self.hot_reload_policy(),
            linked_program,
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
        let program = compile_module_sources_with_options_and_registry(
            &sources,
            &self.compiler_options(),
            self.compiler_registry(),
        )
        .map_err(|error| {
            EngineHotReloadSourceError::hot_reload(HotReloadError {
                kind: HotReloadErrorKind::Compile(error),
            })
        })?;
        let linked_program = self
            .link_program(&program)
            .map_err(HotReloadError::from)
            .map_err(EngineHotReloadSourceError::hot_reload)?;
        update_from_linked_program(
            previous,
            program,
            self.hot_reload_abi(),
            self.hot_reload_policy(),
            linked_program,
        )
        .map_err(EngineHotReloadSourceError::hot_reload)
    }
}
