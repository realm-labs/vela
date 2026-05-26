use std::fmt;
use std::path::Path;

use vela_bytecode::Program;
use vela_bytecode::compiler::{
    CompileError, compile_module_sources_with_options, compile_program_source_with_options,
};
use vela_common::SourceId;

use crate::Engine;

mod loader;

pub(crate) use loader::{load_module_sources, read_source_text};

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
    pub fn compile_file(&self, path: impl AsRef<Path>) -> Result<Program, EngineSourceError> {
        let path = path.as_ref();
        let text = read_source_text(path)?;
        compile_program_source_with_options(SourceId::new(1), &text, &self.compiler_options())
            .map_err(EngineSourceError::compile)
    }

    pub fn compile_dir(&self, root: impl AsRef<Path>) -> Result<Program, EngineSourceError> {
        let root = root.as_ref();
        let sources = load_module_sources(root)?;
        compile_module_sources_with_options(&sources, &self.compiler_options())
            .map_err(EngineSourceError::compile)
    }
}
