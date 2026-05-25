use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use vela_bytecode::Program;
use vela_bytecode::compiler::{
    CompileError, compile_module_sources_with_options, compile_program_source_with_options,
};
use vela_common::SourceId;
use vela_hir::{ModulePath, ModuleSource};

use crate::Engine;

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
        let text = fs::read_to_string(path).map_err(|error| EngineSourceError::io(path, error))?;
        compile_program_source_with_options(SourceId::new(1), &text, &self.compiler_options())
            .map_err(EngineSourceError::compile)
    }

    pub fn compile_dir(&self, root: impl AsRef<Path>) -> Result<Program, EngineSourceError> {
        let root = root.as_ref();
        let mut files = Vec::new();
        collect_lang_files(root, &mut files)?;
        files.sort();

        let mut sources = Vec::with_capacity(files.len());
        for (index, path) in files.iter().enumerate() {
            let source_id = source_id(index, files.len())?;
            let text =
                fs::read_to_string(path).map_err(|error| EngineSourceError::io(path, error))?;
            let module_path = module_path(root, path)?;
            sources.push(ModuleSource::new(source_id, module_path, text));
        }

        compile_module_sources_with_options(&sources, &self.compiler_options())
            .map_err(EngineSourceError::compile)
    }
}

fn collect_lang_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), EngineSourceError> {
    let entries = fs::read_dir(root).map_err(|error| EngineSourceError::io(root, error))?;
    for entry in entries {
        let entry = entry.map_err(|error| EngineSourceError::io(root, error))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| EngineSourceError::io(&path, error))?;
        if file_type.is_dir() {
            collect_lang_files(&path, files)?;
        } else if file_type.is_file()
            && path.extension().and_then(|ext| ext.to_str()) == Some("lang")
        {
            files.push(path);
        }
    }
    Ok(())
}

fn source_id(index: usize, count: usize) -> Result<SourceId, EngineSourceError> {
    let raw = index
        .checked_add(1)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| EngineSourceError::too_many_sources(count))?;
    Ok(SourceId::new(raw))
}

fn module_path(root: &Path, path: &Path) -> Result<ModulePath, EngineSourceError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| EngineSourceError::invalid_path(path))?;
    let components = relative.components().collect::<Vec<_>>();
    let mut segments = Vec::new();
    for (index, component) in components.iter().enumerate() {
        let component_path = component.as_os_str();
        let segment = if index + 1 == components.len() {
            Path::new(component_path)
                .file_stem()
                .and_then(|stem| stem.to_str())
        } else {
            component_path.to_str()
        };
        let Some(segment) = segment else {
            return Err(EngineSourceError::invalid_path(path));
        };
        if !segment.is_empty() {
            segments.push(segment.to_owned());
        }
    }
    if segments.is_empty() {
        return Err(EngineSourceError::invalid_path(path));
    }
    Ok(ModulePath::new(segments))
}
