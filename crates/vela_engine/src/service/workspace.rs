//! Immutable virtual source workspaces for service patches.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Component, Path};
use std::sync::Arc;

use parking_lot::Mutex;
use vela_common::{ServiceGenerationId, SourceId};
use vela_hir::module_graph::ModuleSource;
use vela_package::{ModulePath, PackageId};

/// Content checksum for one complete service patch source workspace.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PatchRevisionChecksum([u8; 32]);

impl PatchRevisionChecksum {
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for PatchRevisionChecksum {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// One complete virtual multi-file Vela source workspace.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PatchSources {
    files: BTreeMap<String, String>,
}

impl PatchSources {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_files<P, S>(
        files: impl IntoIterator<Item = (P, S)>,
    ) -> Result<Self, ServicePatchWorkspaceError>
    where
        P: Into<String>,
        S: Into<String>,
    {
        let mut sources = Self::new();
        for (path, source) in files {
            sources.put(path, source)?;
        }
        Ok(sources)
    }

    pub fn put(
        &mut self,
        path: impl Into<String>,
        source: impl Into<String>,
    ) -> Result<Option<String>, ServicePatchWorkspaceError> {
        let path = path.into();
        validate_source_path(&path)?;
        Ok(self.files.insert(path, source.into()))
    }

    pub fn remove(&mut self, path: &str) -> Result<String, ServicePatchWorkspaceError> {
        validate_source_path(path)?;
        self.files
            .remove(path)
            .ok_or_else(|| ServicePatchWorkspaceError::MissingSource {
                path: path.to_owned(),
            })
    }

    #[must_use]
    pub fn get(&self, path: &str) -> Option<&str> {
        self.files.get(path).map(String::as_str)
    }

    #[must_use]
    pub fn contains(&self, path: &str) -> bool {
        self.files.contains_key(path)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.files.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&str, &str)> {
        self.files
            .iter()
            .map(|(path, source)| (path.as_str(), source.as_str()))
    }

    pub(crate) fn module_sources(&self) -> Result<Vec<ModuleSource>, ServicePatchWorkspaceError> {
        let mut modules = Vec::with_capacity(self.files.len());
        let mut module_paths = BTreeMap::new();
        for (index, (path, source)) in self.files.iter().enumerate() {
            let source_id = index
                .checked_add(1)
                .and_then(|value| u32::try_from(value).ok())
                .map(SourceId::new)
                .ok_or(ServicePatchWorkspaceError::TooManySources {
                    count: self.files.len(),
                })?;
            let module_path = source_module_path(path)?;
            if let Some(previous) = module_paths.insert(module_path.clone(), path.clone()) {
                return Err(ServicePatchWorkspaceError::DuplicateModulePath {
                    first: previous,
                    second: path.clone(),
                    module: module_path.join(),
                });
            }
            modules.push(ModuleSource::new(
                source_id,
                PackageId::anonymous(),
                module_path,
                source,
            ));
        }
        Ok(modules)
    }

    fn checksum(&self) -> PatchRevisionChecksum {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"vela-service-patch-sources-v1");
        for (path, source) in &self.files {
            hasher.update(&(path.len() as u64).to_le_bytes());
            hasher.update(path.as_bytes());
            hasher.update(&(source.len() as u64).to_le_bytes());
            hasher.update(source.as_bytes());
        }
        PatchRevisionChecksum(*hasher.finalize().as_bytes())
    }
}

/// One incremental edit to the current service patch workspace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PatchEdit {
    Put { path: String, source: String },
    Remove { path: String },
}

impl PatchEdit {
    #[must_use]
    pub fn put(path: impl Into<String>, source: impl Into<String>) -> Self {
        Self::Put {
            path: path.into(),
            source: source.into(),
        }
    }

    #[must_use]
    pub fn remove(path: impl Into<String>) -> Self {
        Self::Remove { path: path.into() }
    }
}

/// One immutable source revision associated with an exact service generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatchRevision {
    service_generation_id: Option<ServiceGenerationId>,
    checksum: PatchRevisionChecksum,
    sources: PatchSources,
}

impl PatchRevision {
    /// Creates an empty detached workspace for an offline patch compiler.
    #[must_use]
    pub fn empty() -> Self {
        Self::from_sources(PatchSources::new())
    }

    /// Creates a detached revision from one complete validated workspace.
    #[must_use]
    pub fn from_sources(sources: PatchSources) -> Self {
        let checksum = sources.checksum();
        Self {
            service_generation_id: None,
            checksum,
            sources,
        }
    }

    #[must_use]
    pub const fn service_generation_id(&self) -> Option<ServiceGenerationId> {
        self.service_generation_id
    }

    #[must_use]
    pub const fn checksum(&self) -> PatchRevisionChecksum {
        self.checksum
    }

    #[must_use]
    pub const fn sources(&self) -> &PatchSources {
        &self.sources
    }

    /// Applies a current or exact-base patch without requiring a live service application.
    pub fn apply(
        &self,
        patch: impl Into<ServicePatch>,
    ) -> Result<Self, ServicePatchWorkspaceError> {
        let sources = apply_patch(Some(self), patch.into())?;
        Ok(Self::from_sources(sources))
    }

    pub(crate) fn with_generation(mut self, generation: ServiceGenerationId) -> Self {
        self.service_generation_id = Some(generation);
        self
    }
}

/// A complete requested change to a service patch workspace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServicePatch {
    Edit {
        expected: Option<PatchRevisionChecksum>,
        expected_generation: Option<ServiceGenerationId>,
        edits: Vec<PatchEdit>,
    },
    Replace {
        sources: PatchSources,
        edits: Vec<PatchEdit>,
    },
}

impl ServicePatch {
    /// Starts an exact-base edit against a revision previously read from the application.
    #[must_use]
    pub fn against(revision: &PatchRevision) -> Self {
        Self::Edit {
            expected: Some(revision.checksum()),
            expected_generation: revision.service_generation_id(),
            edits: Vec::new(),
        }
    }

    /// Replaces the complete workspace, including after a detached bundle activation.
    #[must_use]
    pub fn replace(sources: PatchSources) -> Self {
        Self::Replace {
            sources,
            edits: Vec::new(),
        }
    }

    /// Adds an edit to this request.
    #[must_use]
    pub fn edit(mut self, edit: PatchEdit) -> Self {
        match &mut self {
            Self::Edit { edits, .. } => edits.push(edit),
            Self::Replace { edits, .. } => edits.push(edit),
        }
        self
    }

    #[must_use]
    pub fn put(self, path: impl Into<String>, source: impl Into<String>) -> Self {
        self.edit(PatchEdit::put(path, source))
    }

    #[must_use]
    pub fn remove(self, path: impl Into<String>) -> Self {
        self.edit(PatchEdit::remove(path))
    }

    fn current(edit: PatchEdit) -> Self {
        Self::Edit {
            expected: None,
            expected_generation: None,
            edits: vec![edit],
        }
    }
}

impl From<PatchEdit> for ServicePatch {
    fn from(edit: PatchEdit) -> Self {
        Self::current(edit)
    }
}

#[derive(Clone, Debug)]
struct PublishedPatchRevision {
    service_generation_id: ServiceGenerationId,
    revision: Option<Arc<PatchRevision>>,
}

/// Source-workspace publication state paired with a generated service controller.
///
/// This type is public only so generated service-domain code can keep the
/// source revision synchronized with service generation activation.
#[doc(hidden)]
pub struct ServicePatchState {
    active: Mutex<PublishedPatchRevision>,
}

impl ServicePatchState {
    #[must_use]
    pub fn new(initial_generation: ServiceGenerationId) -> Self {
        Self {
            active: Mutex::new(PublishedPatchRevision {
                service_generation_id: initial_generation,
                revision: Some(Arc::new(
                    PatchRevision::empty().with_generation(initial_generation),
                )),
            }),
        }
    }

    pub fn revision(
        &self,
        active_generation: ServiceGenerationId,
    ) -> Result<Arc<PatchRevision>, ServicePatchWorkspaceError> {
        let active = self.active.lock();
        validate_generation(&active, active_generation)?;
        active
            .revision
            .clone()
            .ok_or(ServicePatchWorkspaceError::SourcesUnavailable {
                generation: active_generation,
            })
    }

    pub fn prepare(
        &self,
        active_generation: ServiceGenerationId,
        patch: ServicePatch,
    ) -> Result<PatchRevision, ServicePatchWorkspaceError> {
        let active = self.active.lock();
        validate_generation(&active, active_generation)?;
        let revision = active.revision.as_deref();
        if revision.is_none() && matches!(patch, ServicePatch::Edit { .. }) {
            return Err(ServicePatchWorkspaceError::SourcesUnavailable {
                generation: active_generation,
            });
        }
        let sources = apply_patch(revision, patch)?;
        Ok(PatchRevision::from_sources(sources).with_generation(active_generation))
    }

    pub fn record_activation(
        &self,
        expected_generation: ServiceGenerationId,
        installed_generation: ServiceGenerationId,
        revision: Option<PatchRevision>,
    ) -> Result<ServicePatchStateRollback, ServicePatchWorkspaceError> {
        let mut active = self.active.lock();
        validate_generation(&active, expected_generation)?;
        let replaced = active.clone();
        let installed = PublishedPatchRevision {
            service_generation_id: installed_generation,
            revision: revision
                .map(|revision| Arc::new(revision.with_generation(installed_generation))),
        };
        *active = installed.clone();
        Ok(ServicePatchStateRollback {
            replaced,
            installed,
        })
    }

    pub fn record_rollback(
        &self,
        rollback: ServicePatchStateRollback,
    ) -> Result<(), ServicePatchWorkspaceError> {
        let mut active = self.active.lock();
        validate_generation(&active, rollback.installed.service_generation_id)?;
        *active = rollback.replaced;
        Ok(())
    }
}

/// Source-state half of one conditional service rollback.
#[doc(hidden)]
pub struct ServicePatchStateRollback {
    replaced: PublishedPatchRevision,
    installed: PublishedPatchRevision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServicePatchWorkspaceError {
    InvalidSourcePath {
        path: String,
    },
    MissingSource {
        path: String,
    },
    DuplicateModulePath {
        first: String,
        second: String,
        module: String,
    },
    TooManySources {
        count: usize,
    },
    StaleRevision {
        expected: PatchRevisionChecksum,
        active: PatchRevisionChecksum,
    },
    StaleServiceGeneration {
        expected: ServiceGenerationId,
        active: ServiceGenerationId,
    },
    GenerationMismatch {
        expected: ServiceGenerationId,
        active: ServiceGenerationId,
    },
    SourcesUnavailable {
        generation: ServiceGenerationId,
    },
}

impl fmt::Display for ServicePatchWorkspaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSourcePath { path } => {
                write!(formatter, "invalid virtual Vela source path `{path}`")
            }
            Self::MissingSource { path } => {
                write!(formatter, "virtual Vela source `{path}` does not exist")
            }
            Self::DuplicateModulePath {
                first,
                second,
                module,
            } => write!(
                formatter,
                "virtual sources `{first}` and `{second}` both map to module `{module}`"
            ),
            Self::TooManySources { count } => {
                write!(
                    formatter,
                    "service patch has too many source files: {count}"
                )
            }
            Self::StaleRevision { expected, active } => write!(
                formatter,
                "stale service patch revision: expected {expected}, active {active}"
            ),
            Self::StaleServiceGeneration { expected, active } => write!(
                formatter,
                "stale service patch generation: expected {}, active {}",
                expected.get(),
                active.get()
            ),
            Self::GenerationMismatch { expected, active } => write!(
                formatter,
                "service patch sources belong to generation {}, active generation is {}",
                expected.get(),
                active.get()
            ),
            Self::SourcesUnavailable { generation } => write!(
                formatter,
                "service generation {} was loaded without its source workspace; replace the complete PatchSources before applying incremental edits",
                generation.get()
            ),
        }
    }
}

impl std::error::Error for ServicePatchWorkspaceError {}

fn validate_generation(
    active: &PublishedPatchRevision,
    expected: ServiceGenerationId,
) -> Result<(), ServicePatchWorkspaceError> {
    if active.service_generation_id != expected {
        return Err(ServicePatchWorkspaceError::GenerationMismatch {
            expected: active.service_generation_id,
            active: expected,
        });
    }
    Ok(())
}

fn apply_edit(
    sources: &mut PatchSources,
    edit: PatchEdit,
) -> Result<(), ServicePatchWorkspaceError> {
    match edit {
        PatchEdit::Put { path, source } => {
            sources.put(path, source)?;
        }
        PatchEdit::Remove { path } => {
            sources.remove(&path)?;
        }
    }
    Ok(())
}

fn apply_patch(
    current: Option<&PatchRevision>,
    patch: ServicePatch,
) -> Result<PatchSources, ServicePatchWorkspaceError> {
    match patch {
        ServicePatch::Edit {
            expected,
            expected_generation,
            edits,
        } => {
            let revision = current.expect("Edit requires a current revision");
            if let (Some(expected), Some(active)) =
                (expected_generation, revision.service_generation_id())
                && expected != active
            {
                return Err(ServicePatchWorkspaceError::StaleServiceGeneration {
                    expected,
                    active,
                });
            }
            if let Some(expected) = expected
                && expected != revision.checksum()
            {
                return Err(ServicePatchWorkspaceError::StaleRevision {
                    expected,
                    active: revision.checksum(),
                });
            }
            let mut sources = revision.sources().clone();
            for edit in edits {
                apply_edit(&mut sources, edit)?;
            }
            Ok(sources)
        }
        ServicePatch::Replace { mut sources, edits } => {
            for (path, _) in sources.iter() {
                validate_source_path(path)?;
            }
            for edit in edits {
                apply_edit(&mut sources, edit)?;
            }
            Ok(sources)
        }
    }
}

fn validate_source_path(path: &str) -> Result<(), ServicePatchWorkspaceError> {
    let path_ref = Path::new(path);
    if path.is_empty()
        || path_ref.is_absolute()
        || path_ref
            .extension()
            .and_then(|extension| extension.to_str())
            != Some("vela")
        || path_ref
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ServicePatchWorkspaceError::InvalidSourcePath {
            path: path.to_owned(),
        });
    }
    source_module_path(path).map(|_| ())
}

fn source_module_path(path: &str) -> Result<ModulePath, ServicePatchWorkspaceError> {
    let path = Path::new(path);
    let components = path.components().collect::<Vec<_>>();
    let mut segments = Vec::with_capacity(components.len());
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(component) = component else {
            return Err(ServicePatchWorkspaceError::InvalidSourcePath {
                path: path.display().to_string(),
            });
        };
        let segment = if index + 1 == components.len() {
            Path::new(component)
                .file_stem()
                .and_then(|stem| stem.to_str())
        } else {
            component.to_str()
        };
        let Some(segment) = segment else {
            return Err(ServicePatchWorkspaceError::InvalidSourcePath {
                path: path.display().to_string(),
            });
        };
        if segment.is_empty()
            || !segment.bytes().enumerate().all(|(index, byte)| {
                byte == b'_'
                    || byte.is_ascii_alphanumeric() && (index > 0 || !byte.is_ascii_digit())
            })
        {
            return Err(ServicePatchWorkspaceError::InvalidSourcePath {
                path: path.display().to_string(),
            });
        }
        segments.push(segment.to_owned());
    }
    if segments.is_empty() {
        return Err(ServicePatchWorkspaceError::InvalidSourcePath {
            path: path.display().to_string(),
        });
    }
    Ok(ModulePath::new(segments))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_are_deterministic_and_exact_base_checked() {
        let state = ServicePatchState::new(ServiceGenerationId::new(1));
        let initial = state
            .revision(ServiceGenerationId::new(1))
            .expect("initial");
        let first = state
            .prepare(
                ServiceGenerationId::new(1),
                ServicePatch::against(&initial)
                    .put("rules/grant.vela", "fn grant() { return 1; }")
                    .put("rules/audit.vela", "fn audit() { return 2; }"),
            )
            .expect("prepared");
        let reordered = PatchSources::from_files([
            ("rules/audit.vela", "fn audit() { return 2; }"),
            ("rules/grant.vela", "fn grant() { return 1; }"),
        ])
        .expect("sources");
        assert_eq!(first.checksum(), reordered.checksum());

        state
            .record_activation(
                ServiceGenerationId::new(1),
                ServiceGenerationId::new(2),
                Some(first),
            )
            .expect("activation");
        let error = state
            .prepare(
                ServiceGenerationId::new(2),
                ServicePatch::against(&initial).put("rules/grant.vela", "changed"),
            )
            .expect_err("old revision must be stale");
        assert!(matches!(
            error,
            ServicePatchWorkspaceError::StaleServiceGeneration { .. }
        ));

        let detached = PatchRevision::empty();
        let changed = detached
            .apply(PatchEdit::put("patch.vela", "fn patch() {}"))
            .expect("detached edit");
        assert!(matches!(
            changed.apply(
                ServicePatch::against(&detached).put("patch.vela", "fn patch() { return 1; }",)
            ),
            Err(ServicePatchWorkspaceError::StaleRevision { .. })
        ));
    }

    #[test]
    fn detached_bundle_requires_complete_source_replacement() {
        let state = ServicePatchState::new(ServiceGenerationId::new(1));
        state
            .record_activation(
                ServiceGenerationId::new(1),
                ServiceGenerationId::new(2),
                None,
            )
            .expect("detached activation");
        assert!(matches!(
            state.prepare(
                ServiceGenerationId::new(2),
                PatchEdit::put("patch.vela", "fn patch() {}").into(),
            ),
            Err(ServicePatchWorkspaceError::SourcesUnavailable { .. })
        ));

        let replacement =
            PatchSources::from_files([("patch.vela", "fn patch() {}")]).expect("sources");
        assert!(
            state
                .prepare(
                    ServiceGenerationId::new(2),
                    ServicePatch::replace(replacement),
                )
                .is_ok()
        );
    }

    #[test]
    fn virtual_paths_are_relative_vela_module_paths() {
        let mut sources = PatchSources::new();
        for path in [
            "",
            "/absolute.vela",
            "../escape.vela",
            "wrong.txt",
            "invalid-name.vela",
        ] {
            assert!(matches!(
                sources.put(path, "fn patch() {}"),
                Err(ServicePatchWorkspaceError::InvalidSourcePath { .. })
            ));
        }
        sources
            .put("operations/reconcile.vela", "fn patch() {}")
            .expect("valid nested module path");
        assert!(matches!(
            sources.remove("operations/missing.vela"),
            Err(ServicePatchWorkspaceError::MissingSource { .. })
        ));
    }
}
