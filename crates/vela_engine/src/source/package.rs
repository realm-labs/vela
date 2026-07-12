use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use vela_bytecode::compiler::error::CompileError;
use vela_bytecode::compiler::{PackageProgramCompilationRequest, compile_package_program};
use vela_bytecode::{LinkError, LinkedArtifact, PackageArtifactMetadata, PackageCompilationInput};
use vela_common::{CapabilitySet, SourceId};
use vela_hir::module_graph::ModuleSource;
use vela_hir::source_ingestion::{HirSourceBuildError, HirSourceSet, build_package_source_set};
use vela_hot_reload::compile::{initial_version_from_linked_artifact, update_from_linked_artifact};
use vela_hot_reload::error::HotReloadError;
use vela_hot_reload::version::{HotUpdate, ProgramVersion};
use vela_package::{PackageGraph, PackageGraphError, PackageId, load_package_graph};

use crate::engine::Engine;

static NEXT_PACKAGE_SNAPSHOT: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PackageCompilationSnapshotId(u64);

#[derive(Clone, Debug)]
pub struct PackageCompilationSnapshot {
    id: PackageCompilationSnapshotId,
    package_graph: Arc<PackageGraph>,
    sources: Arc<HirSourceSet>,
    module_sources: Arc<[ModuleSource]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageCompileRequest {
    snapshot: PackageCompilationSnapshotId,
    roots: BTreeSet<PackageId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnginePackageError {
    pub kind: EnginePackageErrorKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EnginePackageErrorKind {
    Graph(PackageGraphError),
    Frontend(HirSourceBuildError),
    Backend(CompileError),
    Link(LinkError),
    HotReload(HotReloadError),
    EmptyRoots,
    TooManySources {
        count: usize,
    },
    UnknownPackage {
        package: PackageId,
    },
    SnapshotMismatch {
        expected: PackageCompilationSnapshotId,
        actual: PackageCompilationSnapshotId,
    },
    UndeclaredCapabilities {
        package: PackageId,
        missing: CapabilitySet,
    },
    MissingHostGrants {
        package: PackageId,
        missing: CapabilitySet,
    },
    RequestFingerprintMismatch,
}

impl PackageCompilationSnapshotId {
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl PackageCompilationSnapshot {
    #[must_use]
    pub const fn id(&self) -> PackageCompilationSnapshotId {
        self.id
    }

    #[must_use]
    pub fn package_graph(&self) -> &Arc<PackageGraph> {
        &self.package_graph
    }

    #[must_use]
    pub fn sources(&self) -> &Arc<HirSourceSet> {
        &self.sources
    }
}

impl PackageCompileRequest {
    #[must_use]
    pub fn for_root(snapshot: &PackageCompilationSnapshot, root: &PackageId) -> Self {
        Self {
            snapshot: snapshot.id,
            roots: BTreeSet::from([root.clone()]),
        }
    }

    #[must_use]
    pub fn for_roots(
        snapshot: &PackageCompilationSnapshot,
        roots: impl IntoIterator<Item = PackageId>,
    ) -> Self {
        Self {
            snapshot: snapshot.id,
            roots: roots.into_iter().collect(),
        }
    }

    #[must_use]
    pub const fn snapshot(&self) -> PackageCompilationSnapshotId {
        self.snapshot
    }

    #[must_use]
    pub const fn roots(&self) -> &BTreeSet<PackageId> {
        &self.roots
    }
}

impl fmt::Display for EnginePackageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            EnginePackageErrorKind::Graph(error) => error.fmt(formatter),
            EnginePackageErrorKind::Frontend(error) => write!(formatter, "{error:?}"),
            EnginePackageErrorKind::Backend(error) => write!(formatter, "{error:?}"),
            EnginePackageErrorKind::Link(error) => error.fmt(formatter),
            EnginePackageErrorKind::HotReload(error) => error.fmt(formatter),
            EnginePackageErrorKind::EmptyRoots => {
                formatter.write_str("package compilation requires at least one root")
            }
            EnginePackageErrorKind::TooManySources { count } => {
                write!(formatter, "too many package source files: {count}")
            }
            EnginePackageErrorKind::UnknownPackage { package } => {
                write!(formatter, "unknown package `{package}`")
            }
            EnginePackageErrorKind::SnapshotMismatch { expected, actual } => write!(
                formatter,
                "package request belongs to snapshot {} but snapshot {} was supplied",
                actual.get(),
                expected.get()
            ),
            EnginePackageErrorKind::UndeclaredCapabilities { package, missing } => write!(
                formatter,
                "package `{package}` uses undeclared capabilities: {}",
                capability_names(*missing)
            ),
            EnginePackageErrorKind::MissingHostGrants { package, missing } => write!(
                formatter,
                "package `{package}` requires capabilities not granted by the host: {}",
                capability_names(*missing)
            ),
            EnginePackageErrorKind::RequestFingerprintMismatch => formatter
                .write_str("package hot reload request does not match the active root package set"),
        }
    }
}

impl std::error::Error for EnginePackageError {}

impl Engine {
    pub fn load_package_workspace(
        &self,
        manifest: impl AsRef<Path>,
    ) -> Result<PackageCompilationSnapshot, EnginePackageError> {
        let manifest = manifest.as_ref();
        let authorized = manifest
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .to_owned();
        self.load_package_workspace_with_authorized_roots(manifest, &[authorized])
    }

    pub fn load_package_workspace_with_authorized_roots(
        &self,
        manifest: impl AsRef<Path>,
        authorized_roots: &[PathBuf],
    ) -> Result<PackageCompilationSnapshot, EnginePackageError> {
        let package_graph = Arc::new(
            load_package_graph(manifest, authorized_roots)
                .map_err(|error| package_error(EnginePackageErrorKind::Graph(error)))?,
        );
        let module_sources = package_module_sources(&package_graph)?;
        let sources = Arc::new(
            build_package_source_set(&module_sources, package_graph.dependency_map().clone())
                .map_err(|error| package_error(EnginePackageErrorKind::Frontend(error)))?,
        );
        Ok(PackageCompilationSnapshot {
            id: PackageCompilationSnapshotId(NEXT_PACKAGE_SNAPSHOT.fetch_add(1, Ordering::Relaxed)),
            package_graph,
            sources,
            module_sources: module_sources.into(),
        })
    }

    pub fn compile_package(
        &self,
        snapshot: &PackageCompilationSnapshot,
        root: &PackageId,
    ) -> Result<Arc<LinkedArtifact>, EnginePackageError> {
        self.compile_packages(snapshot, &PackageCompileRequest::for_root(snapshot, root))
    }

    pub fn compile_packages(
        &self,
        snapshot: &PackageCompilationSnapshot,
        request: &PackageCompileRequest,
    ) -> Result<Arc<LinkedArtifact>, EnginePackageError> {
        if request.snapshot != snapshot.id {
            return Err(package_error(EnginePackageErrorKind::SnapshotMismatch {
                expected: snapshot.id,
                actual: request.snapshot,
            }));
        }
        let closure = package_closure(&snapshot.package_graph, &request.roots)?;
        validate_host_grants(self, &snapshot.package_graph, &closure)?;
        let selected = snapshot
            .module_sources
            .iter()
            .filter(|source| closure.contains(&source.package))
            .cloned()
            .collect::<Vec<_>>();
        let selected_dependencies = closure
            .iter()
            .filter_map(|package| {
                snapshot
                    .package_graph
                    .dependencies(package)
                    .cloned()
                    .map(|dependencies| (package.clone(), dependencies))
            })
            .collect();
        let sources = build_package_source_set(&selected, selected_dependencies)
            .map_err(|error| package_error(EnginePackageErrorKind::Frontend(error)))?;
        let packages = package_inputs(&snapshot.package_graph, &closure);
        let options = self.compiler_options();
        let program = compile_package_program(PackageProgramCompilationRequest {
            sources: &sources,
            options: &options,
            registry: Some(self.compiler_registry()),
            roots: &request.roots,
            packages: &packages,
        })
        .map_err(|error| package_error(EnginePackageErrorKind::Backend(error)))?;
        validate_observed_capabilities(program.package_metadata())?;
        self.link_compiled_program(program)
            .map_err(|error| package_error(EnginePackageErrorKind::Link(error)))
    }

    pub fn compile_package_hot_reload_initial(
        &self,
        snapshot: &PackageCompilationSnapshot,
        request: &PackageCompileRequest,
    ) -> Result<ProgramVersion, EnginePackageError> {
        let artifact = self.compile_packages(snapshot, request)?;
        initial_version_from_linked_artifact(self.hot_reload_abi(), artifact)
            .map_err(|error| package_error(EnginePackageErrorKind::HotReload(error)))
    }

    pub fn compile_package_hot_reload_update(
        &self,
        previous: &ProgramVersion,
        snapshot: &PackageCompilationSnapshot,
        request: &PackageCompileRequest,
    ) -> Result<HotUpdate, EnginePackageError> {
        let artifact = self.compile_packages(snapshot, request)?;
        let previous_request = previous
            .linked_artifact()
            .package_metadata()
            .map(PackageArtifactMetadata::request);
        let next_request = artifact
            .package_metadata()
            .map(PackageArtifactMetadata::request);
        if previous_request != next_request {
            return Err(package_error(
                EnginePackageErrorKind::RequestFingerprintMismatch,
            ));
        }
        update_from_linked_artifact(
            previous,
            self.hot_reload_abi(),
            self.hot_reload_policy(),
            artifact,
        )
        .map_err(|error| package_error(EnginePackageErrorKind::HotReload(error)))
    }
}

fn package_module_sources(graph: &PackageGraph) -> Result<Vec<ModuleSource>, EnginePackageError> {
    graph
        .sources()
        .sources()
        .iter()
        .enumerate()
        .map(|(index, source)| {
            let raw = u32::try_from(index + 1).map_err(|_| {
                package_error(EnginePackageErrorKind::TooManySources { count: index + 1 })
            })?;
            Ok(ModuleSource::new(
                SourceId::new(raw),
                source.package.clone(),
                source.module.clone(),
                source.text.clone(),
            ))
        })
        .collect()
}

fn package_closure(
    graph: &PackageGraph,
    roots: &BTreeSet<PackageId>,
) -> Result<BTreeSet<PackageId>, EnginePackageError> {
    if roots.is_empty() {
        return Err(package_error(EnginePackageErrorKind::EmptyRoots));
    }
    let mut closure = BTreeSet::new();
    let mut pending = roots.iter().cloned().collect::<Vec<_>>();
    while let Some(package) = pending.pop() {
        if !graph.packages().contains_key(&package) {
            return Err(package_error(EnginePackageErrorKind::UnknownPackage {
                package,
            }));
        }
        if !closure.insert(package.clone()) {
            continue;
        }
        if let Some(dependencies) = graph.dependencies(&package) {
            pending.extend(dependencies.values().cloned());
        }
    }
    Ok(closure)
}

fn package_inputs(
    graph: &PackageGraph,
    packages: &BTreeSet<PackageId>,
) -> Vec<PackageCompilationInput> {
    packages
        .iter()
        .filter_map(|id| graph.packages().get(id))
        .map(|package| PackageCompilationInput {
            id: package.id.clone(),
            version: package.version.clone(),
            declared_capabilities: package.required_capabilities,
        })
        .collect()
}

fn validate_host_grants(
    engine: &Engine,
    graph: &PackageGraph,
    packages: &BTreeSet<PackageId>,
) -> Result<(), EnginePackageError> {
    for package in packages {
        let required = graph
            .packages()
            .get(package)
            .expect("package closure contains known packages")
            .required_capabilities;
        let missing = required.difference(engine.capabilities());
        if !missing.is_empty() {
            return Err(package_error(EnginePackageErrorKind::MissingHostGrants {
                package: package.clone(),
                missing,
            }));
        }
    }
    Ok(())
}

fn validate_observed_capabilities(
    metadata: Option<&PackageArtifactMetadata>,
) -> Result<(), EnginePackageError> {
    let metadata = metadata.expect("package compilation always produces package metadata");
    for package in metadata.packages() {
        let missing = package
            .observed_capabilities()
            .difference(package.declared_capabilities());
        if !missing.is_empty() {
            return Err(package_error(
                EnginePackageErrorKind::UndeclaredCapabilities {
                    package: package.id().clone(),
                    missing,
                },
            ));
        }
    }
    Ok(())
}

fn capability_names(capabilities: CapabilitySet) -> String {
    capabilities
        .iter()
        .map(|capability| capability.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn package_error(kind: EnginePackageErrorKind) -> EnginePackageError {
    EnginePackageError { kind }
}

#[cfg(test)]
mod tests;
