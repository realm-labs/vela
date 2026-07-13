use std::fmt;
use std::path::{Path, PathBuf};

use vela_bytecode::compiler::CompiledProgram;
use vela_bytecode::compiler::error::CompileError;
use vela_bytecode::compiler::{ProgramCompilationRequest, compile_program};
use vela_common::SourceId;
use vela_hir::module_graph::ModuleSource;
use vela_hir::source_ingestion::{
    HirSourceBuildError, HirSourceSet, build_module_source_set, build_single_source,
};
use vela_package::{PackageGraph, PackageGraphError, load_package_graph};

use crate::engine::Engine;

mod loader;
mod package;

pub use package::{
    EnginePackageError, EnginePackageErrorKind, PackageCompilationSnapshot,
    PackageCompilationSnapshotId, PackageCompileRequest, ProviderCatalog, ProviderCatalogError,
    ProviderCompileRequest, ProviderDescriptor, ProviderMethodDescriptor, ProviderSelection,
    ProviderSourceLocation,
};

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
    pub fn load_package_graph(
        &self,
        manifest: impl AsRef<Path>,
        authorized_roots: &[PathBuf],
    ) -> Result<PackageGraph, PackageGraphError> {
        load_package_graph(manifest, authorized_roots)
    }

    pub fn compile_source(&self, text: &str) -> Result<CompiledProgram, EngineSourceError> {
        self.compile_source_with_id(SourceId::new(1), text)
    }

    pub(crate) fn compile_source_with_id(
        &self,
        source: SourceId,
        text: &str,
    ) -> Result<CompiledProgram, EngineSourceError> {
        let sources = build_single_source(source, text).map_err(EngineSourceError::frontend)?;
        self.compile_source_set(&sources)
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
        self.compile_module_sources(&sources)
    }

    pub(crate) fn compile_module_sources(
        &self,
        sources: &[ModuleSource],
    ) -> Result<CompiledProgram, EngineSourceError> {
        let sources = build_module_source_set(sources).map_err(EngineSourceError::frontend)?;
        self.compile_source_set(&sources)
    }

    fn compile_source_set(
        &self,
        sources: &HirSourceSet,
    ) -> Result<CompiledProgram, EngineSourceError> {
        let options = self.compiler_options();
        compile_program(ProgramCompilationRequest {
            sources,
            options: &options,
            registry: Some(self.compiler_registry()),
        })
        .map_err(EngineSourceError::backend)
    }
}

#[cfg(test)]
mod package_tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn engine_and_language_service_assemble_the_same_package_graph() {
        let root = std::env::temp_dir().join(format!(
            "vela_shared_package_graph_{}_{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("src")).expect("create package source root");
        fs::write(
            root.join("vela.toml"),
            "[package]\nid=\"dev.vela.shared\"\nname=\"shared\"\nversion=\"0.1.0\"\n",
        )
        .expect("write manifest");
        fs::write(root.join("src/main.vela"), "fn main() { return 1 }\n").expect("write source");

        let engine = Engine::builder().build().expect("build engine");
        let engine_graph = engine
            .load_package_graph(root.join("vela.toml"), std::slice::from_ref(&root))
            .expect("Engine graph");
        let service_graph = vela_language_service::load_package_project(
            root.join("vela.toml"),
            std::slice::from_ref(&root),
        )
        .expect("language-service graph");

        assert_eq!(engine_graph, service_graph);
        fs::remove_dir_all(root).expect("remove package fixture");
    }
}
