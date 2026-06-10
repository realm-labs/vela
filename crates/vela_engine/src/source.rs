use std::fmt;
use std::path::Path;

use vela_bytecode::UnlinkedProgram;
use vela_bytecode::compiler::error::CompileError;
use vela_bytecode::compiler::{
    compile_module_sources_with_options_and_registry,
    compile_program_source_with_options_and_registry,
};
use vela_common::SourceId;

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

    fn compile(error: CompileError) -> Self {
        Self {
            kind: EngineSourceErrorKind::Compile(error),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum EngineSourceErrorKind {
    Io { path: String, message: String },
    InvalidSourcePath { path: String },
    TooManySources { count: usize },
    Compile(CompileError),
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
            EngineSourceErrorKind::Compile(error) => write!(formatter, "{error:?}"),
        }
    }
}

impl std::error::Error for EngineSourceError {}

impl Engine {
    pub fn compile_source(
        &self,
        source: SourceId,
        text: &str,
    ) -> Result<UnlinkedProgram, EngineSourceError> {
        compile_program_source_with_options_and_registry(
            source,
            text,
            &self.compiler_options(),
            self.compiler_registry(),
        )
        .map_err(EngineSourceError::compile)
    }

    pub fn compile_file(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<UnlinkedProgram, EngineSourceError> {
        let path = path.as_ref();
        let text = read_source_text(path)?;
        self.compile_source(SourceId::new(1), &text)
    }

    pub fn compile_dir(
        &self,
        root: impl AsRef<Path>,
    ) -> Result<UnlinkedProgram, EngineSourceError> {
        let root = root.as_ref();
        let sources = load_module_sources(root)?;
        compile_module_sources_with_options_and_registry(
            &sources,
            &self.compiler_options(),
            self.compiler_registry(),
        )
        .map_err(EngineSourceError::compile)
    }
}
