use std::fmt;
use std::path::{Path, PathBuf};

use vela_bytecode::compiler::CompiledProgram;
use vela_bytecode::compiler::error::CompileError;
use vela_bytecode::compiler::{ProgramCompilationRequest, compile_program};
use vela_common::SourceId;
use vela_common::Span;
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

    fn override_link(message: impl Into<String>, source: Span) -> Self {
        Self {
            kind: EngineSourceErrorKind::OverrideLink {
                message: message.into(),
                source,
            },
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
    OverrideLink { message: String, source: Span },
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
            EngineSourceErrorKind::OverrideLink { message, source } => {
                write!(formatter, "{message} at {source:?}")
            }
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
        let mut program = compile_program(ProgramCompilationRequest {
            sources,
            options: &options,
            registry: Some(self.compiler_registry()),
        })
        .map_err(EngineSourceError::backend)?;
        self.resolve_override_targets(&mut program)?;
        Ok(program)
    }

    fn resolve_override_targets(
        &self,
        program: &mut CompiledProgram,
    ) -> Result<(), EngineSourceError> {
        let registry = self.registry();
        let schema = program.binding_schema_mut();
        for callable in schema.callables_mut() {
            let Some(target) = callable.override_target.as_ref() else {
                continue;
            };
            let path = target.public_path().to_owned();
            let slot = self.replaceable_slot(&path).ok_or_else(|| {
                EngineSourceError::override_link(
                    format!(
                        "Vela override `{}` names unknown replaceable target `{path}`",
                        callable.public_path
                    ),
                    callable.source,
                )
            })?;
            crate::dispatch::validate_override_source(slot, callable).map_err(|error| {
                EngineSourceError::override_link(error.to_string(), callable.source)
            })?;
            import_override_contract(registry.as_ref(), slot, callable)
                .map_err(|message| EngineSourceError::override_link(message, callable.source))?;
            callable.override_target = Some(vela_bytecode::RustBindingOverrideTarget::Resolved {
                public_path: path,
                slot: slot.id,
                contract_fingerprint: slot.contract.abi_fingerprint().get(),
            });
        }
        schema.refresh_checksum();
        Ok(())
    }
}

fn import_override_contract(
    registry: &vela_reflect::registry::TypeRegistry,
    slot: &crate::dispatch::ReplaceableSlotDescriptor,
    callable: &mut vela_bytecode::RustBindingCallable,
) -> Result<(), String> {
    let expected = slot
        .contract
        .parameters
        .iter()
        .filter(|parameter| parameter.mode != crate::interop::BoundaryMode::HiddenContext);
    for (actual, expected) in callable.parameters.iter_mut().zip(expected) {
        actual.mode = match expected.mode {
            crate::interop::BoundaryMode::Value
            | crate::interop::BoundaryMode::ReadOnlyValueBorrow => {
                vela_bytecode::RustBindingBoundaryMode::Value
            }
            crate::interop::BoundaryMode::SharedHost => {
                vela_bytecode::RustBindingBoundaryMode::SharedHost
            }
            crate::interop::BoundaryMode::ExclusiveHost => {
                vela_bytecode::RustBindingBoundaryMode::ExclusiveHost
            }
            crate::interop::BoundaryMode::HiddenContext => {
                unreachable!("hidden parameters were filtered")
            }
        };
        actual.ty = binding_type_from_hint(registry, &expected.ty)?;
    }
    callable.returns.ty = binding_type_from_hint(registry, &slot.contract.returns.ty)?;
    callable.returns.mode = match slot.contract.returns.mode {
        crate::interop::ReturnMode::OwnedValue => vela_bytecode::RustBindingReturnMode::OwnedValue,
        crate::interop::ReturnMode::StructuredValue => {
            vela_bytecode::RustBindingReturnMode::StructuredValue
        }
        crate::interop::ReturnMode::ScopedHost {
            origin,
            child_access,
            parent_freeze,
        } => vela_bytecode::RustBindingReturnMode::ScopedHost {
            origin: match origin {
                crate::interop::BorrowedReturnOrigin::Receiver => {
                    vela_bytecode::RustBindingBorrowedReturnOrigin::Receiver
                }
                crate::interop::BorrowedReturnOrigin::Parameter(index) => {
                    vela_bytecode::RustBindingBorrowedReturnOrigin::Parameter(index)
                }
            },
            child_access: binding_scoped_access(child_access),
            parent_freeze: binding_scoped_access(parent_freeze),
        },
    };
    callable.returns.error_mode = match slot.contract.returns.error_mode {
        crate::interop::ErrorMode::Value => vela_bytecode::RustBindingErrorMode::Value,
        crate::interop::ErrorMode::RuntimeResult => {
            vela_bytecode::RustBindingErrorMode::RuntimeResult
        }
    };
    callable.refresh_contract_fingerprint();
    Ok(())
}

fn binding_scoped_access(
    access: crate::interop::ScopedHostAccess,
) -> vela_bytecode::RustBindingScopedHostAccess {
    match access {
        crate::interop::ScopedHostAccess::Shared => {
            vela_bytecode::RustBindingScopedHostAccess::Shared
        }
        crate::interop::ScopedHostAccess::Exclusive => {
            vela_bytecode::RustBindingScopedHostAccess::Exclusive
        }
    }
}

fn binding_type_from_hint(
    registry: &vela_reflect::registry::TypeRegistry,
    hint: &crate::native::TypeHint,
) -> Result<vela_bytecode::RustBindingType, String> {
    use crate::native::TypeHint;
    use vela_bytecode::RustBindingType;

    let path = |name: &str, arguments: Vec<RustBindingType>| RustBindingType::Path {
        segments: vec![name.to_owned()].into_boxed_slice(),
        arguments: arguments.into_boxed_slice(),
    };
    Ok(match hint {
        TypeHint::Any => RustBindingType::Any,
        TypeHint::Primitive(tag) => path(tag.name(), Vec::new()),
        TypeHint::Array => path("Array", Vec::new()),
        TypeHint::ArrayOf(element) => {
            path("Array", vec![binding_type_from_hint(registry, element)?])
        }
        TypeHint::Map => path("Map", Vec::new()),
        TypeHint::MapOf { key, value } => path(
            "Map",
            vec![
                binding_type_from_hint(registry, key)?,
                binding_type_from_hint(registry, value)?,
            ],
        ),
        TypeHint::Set => path("Set", Vec::new()),
        TypeHint::SetOf(element) => path("Set", vec![binding_type_from_hint(registry, element)?]),
        TypeHint::TupleOf(elements) => path(
            "Tuple",
            elements
                .iter()
                .map(|element| binding_type_from_hint(registry, element))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        TypeHint::Iterator => path("Iterator", Vec::new()),
        TypeHint::IteratorOf(item) => {
            path("Iterator", vec![binding_type_from_hint(registry, item)?])
        }
        TypeHint::OptionOf(payload) => {
            path("Option", vec![binding_type_from_hint(registry, payload)?])
        }
        TypeHint::ResultOf { ok, err } => path(
            "Result",
            vec![
                binding_type_from_hint(registry, ok)?,
                binding_type_from_hint(registry, err)?,
            ],
        ),
        TypeHint::PathProxy => path("PathProxy", Vec::new()),
        TypeHint::Record(key) | TypeHint::Enum(key) => RustBindingType::Definition {
            type_id: key.id,
            public_path: key.name.clone(),
        },
        TypeHint::Host(key) => {
            let descriptor = registry
                .types()
                .find(|descriptor| descriptor.key.id == key.id)
                .ok_or_else(|| {
                    format!(
                        "override contract names unregistered host type `{}`",
                        key.name
                    )
                })?;
            let runtime_type_id = descriptor.host_type_id.ok_or_else(|| {
                format!(
                    "override contract host type `{}` has no runtime identity",
                    key.name
                )
            })?;
            RustBindingType::Host {
                semantic_type_id: key.id,
                runtime_type_id,
                public_path: key.name.clone(),
            }
        }
        TypeHint::Trait(name) => path(name, Vec::new()),
        TypeHint::Function => path("Function", Vec::new()),
    })
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
