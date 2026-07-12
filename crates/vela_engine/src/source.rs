use std::fmt;
use std::path::Path;

use vela_bytecode::compiler::CompiledProgram;
use vela_bytecode::compiler::error::CompileError;
use vela_bytecode::compiler::{ProgramCompilationMode, ProgramCompilationRequest, compile_program};
use vela_common::SourceId;
use vela_hir::module_graph::{ModulePath, ModuleSource};
use vela_hir::source_ingestion::{HirSourceBuildError, build_source_set};

use crate::engine::Engine;

mod loader;

pub(crate) use loader::{
    load_module_sources, load_module_sources_for_changed_file, read_source_text,
};

#[derive(Clone, Debug, PartialEq)]
pub struct EngineSourceError {
    pub kind: EngineSourceErrorKind,
}

impl EngineSourceError {
    fn io(path: &Path, error: std::io::Error) -> Self {
        Self {
            kind: EngineSourceErrorKind::Io {
                path: path.display().to_string(),
                message: error.to_string(),
            },
        }
    }

    fn invalid_path(path: &Path) -> Self {
        Self {
            kind: EngineSourceErrorKind::InvalidSourcePath {
                path: path.display().to_string(),
            },
        }
    }

    fn too_many_sources(count: usize) -> Self {
        Self {
            kind: EngineSourceErrorKind::TooManySources { count },
        }
    }

    fn frontend(error: HirSourceBuildError) -> Self {
        Self {
            kind: EngineSourceErrorKind::Frontend(error),
        }
    }

    fn backend(error: CompileError) -> Self {
        Self {
            kind: EngineSourceErrorKind::Backend(error),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum EngineSourceErrorKind {
    Io { path: String, message: String },
    InvalidSourcePath { path: String },
    TooManySources { count: usize },
    Frontend(HirSourceBuildError),
    Backend(CompileError),
}

impl fmt::Display for EngineSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            EngineSourceErrorKind::Io { path, message } => {
                write!(formatter, "failed to read source {path}: {message}")
            }
            EngineSourceErrorKind::InvalidSourcePath { path } => {
                write!(formatter, "invalid source path {path}")
            }
            EngineSourceErrorKind::TooManySources { count } => {
                write!(formatter, "too many source files: {count}")
            }
            EngineSourceErrorKind::Frontend(error) => write!(formatter, "{error:?}"),
            EngineSourceErrorKind::Backend(error) => write!(formatter, "{error:?}"),
        }
    }
}

impl std::error::Error for EngineSourceError {}

impl Engine {
    pub fn compile_source(&self, text: &str) -> Result<CompiledProgram, EngineSourceError> {
        self.compile_source_with_id(SourceId::new(1), text)
    }

    pub(crate) fn compile_source_with_id(
        &self,
        source: SourceId,
        text: &str,
    ) -> Result<CompiledProgram, EngineSourceError> {
        let sources = [ModuleSource::new(
            source,
            ModulePath::new(Vec::<String>::new()),
            text,
        )];
        self.compile_sources(&sources, true)
    }

    pub fn compile_file(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<CompiledProgram, EngineSourceError> {
        let path = path.as_ref();
        let text = read_source_text(path)?;
        self.compile_source(&text)
    }

    pub fn compile_dir(
        &self,
        root: impl AsRef<Path>,
    ) -> Result<CompiledProgram, EngineSourceError> {
        let root = root.as_ref();
        let sources = load_module_sources(root)?;
        self.compile_sources(&sources, false)
    }

    pub(crate) fn compile_sources(
        &self,
        sources: &[ModuleSource],
        single_source: bool,
    ) -> Result<CompiledProgram, EngineSourceError> {
        let built = build_source_set(sources).map_err(EngineSourceError::frontend)?;
        let mode = if single_source {
            ProgramCompilationMode::SingleSource {
                root: built.modules()[0],
            }
        } else {
            ProgramCompilationMode::ModuleGraph {
                modules: built.modules().into(),
            }
        };
        let options = self.compiler_options();
        compile_program(ProgramCompilationRequest {
            graph: built.graph(),
            mode: &mode,
            options: &options,
            registry: Some(self.compiler_registry()),
        })
        .map_err(EngineSourceError::backend)
    }
}
