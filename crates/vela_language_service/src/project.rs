use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use vela_common::SourceId;
use vela_hir::module_graph::ModuleSource;
use vela_package::{ManifestFileId, parse_manifest};
use vela_package::{
    ModuleKey, ModulePath, PackageAlias, PackageGraph, PackageGraphError, PackageId,
};

use crate::{DocumentId, SourceVersion, WorkspaceSnapshot};

const SOURCE_EXTENSION: &str = "vela";
const CONFIG_FILE: &str = "vela.toml";

pub fn load_package_project(
    manifest: impl AsRef<Path>,
    authorized_roots: &[PathBuf],
) -> Result<PackageGraph, PackageGraphError> {
    vela_package::load_package_graph(manifest, authorized_roots)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceRoot {
    path: Arc<str>,
    package: PackageId,
}

impl WorkspaceRoot {
    #[must_use]
    pub fn new(path: impl Into<Arc<str>>) -> Self {
        Self {
            path: path.into(),
            package: PackageId::anonymous(),
        }
    }

    #[must_use]
    pub fn for_package(path: impl Into<Arc<str>>, package: PackageId) -> Self {
        Self {
            path: path.into(),
            package,
        }
    }

    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[must_use]
    pub const fn package(&self) -> &PackageId {
        &self.package
    }
}

impl From<&str> for WorkspaceRoot {
    fn from(value: &str) -> Self {
        Self::new(normalize_document_path(value))
    }
}

impl From<String> for WorkspaceRoot {
    fn from(value: String) -> Self {
        Self::new(normalize_document_path(&value))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectMode {
    Workspace,
    Scratch { document: DocumentId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaConfig {
    path: Option<Arc<str>>,
}

impl SchemaConfig {
    #[must_use]
    pub fn none() -> Self {
        Self { path: None }
    }

    #[must_use]
    pub fn from_path(path: impl Into<Arc<str>>) -> Self {
        Self {
            path: Some(path.into()),
        }
    }

    #[must_use]
    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }
}

impl Default for SchemaConfig {
    fn default() -> Self {
        Self::none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceConfig {
    roots: Vec<WorkspaceRoot>,
    mode: ProjectMode,
    schema: SchemaConfig,
}

impl WorkspaceConfig {
    #[must_use]
    pub fn workspace(roots: impl IntoIterator<Item = WorkspaceRoot>) -> Self {
        Self {
            roots: roots.into_iter().collect(),
            mode: ProjectMode::Workspace,
            schema: SchemaConfig::none(),
        }
    }

    #[must_use]
    pub fn scratch(document: impl Into<DocumentId>) -> Self {
        Self {
            roots: Vec::new(),
            mode: ProjectMode::Scratch {
                document: document.into(),
            },
            schema: SchemaConfig::none(),
        }
    }

    #[must_use]
    pub fn from_package_graph(graph: &PackageGraph, schema: SchemaConfig) -> Self {
        let roots = graph
            .packages()
            .values()
            .flat_map(|package| {
                package.source_roots.iter().map(|root| {
                    WorkspaceRoot::for_package(
                        normalize_document_path(&root.display().to_string()),
                        package.id.clone(),
                    )
                })
            })
            .collect();
        Self {
            roots,
            mode: ProjectMode::Workspace,
            schema,
        }
    }

    #[must_use]
    pub fn from_vela_toml(config_root: impl AsRef<str>, text: &str) -> ConfigParseResult {
        parse_config(config_root.as_ref(), text)
    }

    #[must_use]
    pub fn roots(&self) -> &[WorkspaceRoot] {
        &self.roots
    }

    #[must_use]
    pub const fn mode(&self) -> &ProjectMode {
        &self.mode
    }

    #[must_use]
    pub const fn schema(&self) -> &SchemaConfig {
        &self.schema
    }

    pub fn set_schema(&mut self, schema: SchemaConfig) {
        self.schema = schema;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFileSnapshot {
    document_id: DocumentId,
    text: Arc<str>,
}

impl SourceFileSnapshot {
    #[must_use]
    pub fn new(document_id: impl Into<DocumentId>, text: impl Into<Arc<str>>) -> Self {
        Self {
            document_id: document_id.into(),
            text: text.into(),
        }
    }

    #[must_use]
    pub fn document_id(&self) -> &DocumentId {
        &self.document_id
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectDiagnostic {
    document_id: Option<DocumentId>,
    message: String,
}

impl ProjectDiagnostic {
    #[must_use]
    pub fn new(document_id: Option<DocumentId>, message: impl Into<String>) -> Self {
        Self {
            document_id,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn document_id(&self) -> Option<&DocumentId> {
        self.document_id.as_ref()
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigParseResult {
    pub config: WorkspaceConfig,
    pub diagnostics: Vec<ProjectDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSources {
    sources: Vec<ModuleSource>,
    document_modules: BTreeMap<DocumentId, ModuleKey>,
    document_versions: BTreeMap<DocumentId, SourceVersion>,
    package_dependencies: BTreeMap<PackageId, BTreeMap<PackageAlias, PackageId>>,
    diagnostics: Vec<ProjectDiagnostic>,
}

impl ProjectSources {
    #[must_use]
    pub fn sources(&self) -> &[ModuleSource] {
        &self.sources
    }

    #[must_use]
    pub fn document_modules(&self) -> &BTreeMap<DocumentId, ModuleKey> {
        &self.document_modules
    }

    #[must_use]
    pub fn document_versions(&self) -> &BTreeMap<DocumentId, SourceVersion> {
        &self.document_versions
    }

    #[must_use]
    pub const fn package_dependencies(
        &self,
    ) -> &BTreeMap<PackageId, BTreeMap<PackageAlias, PackageId>> {
        &self.package_dependencies
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[ProjectDiagnostic] {
        &self.diagnostics
    }
}

#[must_use]
pub fn assemble_project_sources(
    config: &WorkspaceConfig,
    files: &[SourceFileSnapshot],
    snapshot: &WorkspaceSnapshot,
) -> ProjectSources {
    match config.mode() {
        ProjectMode::Workspace => assemble_workspace_sources(config, files, snapshot),
        ProjectMode::Scratch { document } => assemble_scratch_source(document, snapshot, files),
    }
}

#[must_use]
pub fn assemble_package_project_sources(
    graph: &PackageGraph,
    files: &[SourceFileSnapshot],
    snapshot: &WorkspaceSnapshot,
) -> ProjectSources {
    let config = WorkspaceConfig::from_package_graph(graph, SchemaConfig::none());
    let mut sources = assemble_workspace_sources(&config, files, snapshot);
    sources.package_dependencies = graph.dependency_map().clone();
    sources
}

fn assemble_workspace_sources(
    config: &WorkspaceConfig,
    files: &[SourceFileSnapshot],
    snapshot: &WorkspaceSnapshot,
) -> ProjectSources {
    let mut inputs = BTreeMap::<DocumentId, Arc<str>>::new();
    let mut document_versions = BTreeMap::<DocumentId, SourceVersion>::new();
    for file in files {
        if is_vela_source(file.document_id.as_str()) {
            inputs.insert(file.document_id.clone(), Arc::clone(&file.text));
            document_versions.insert(file.document_id.clone(), SourceVersion::INITIAL);
        }
    }
    for document_id in snapshot.open_document_ids() {
        if is_vela_source(document_id.as_str())
            && let Some(document) = snapshot.document(&document_id)
        {
            document_versions.insert(document_id.clone(), document.version());
            inputs.insert(document_id, Arc::<str>::from(document.text()));
        }
    }

    let mut diagnostics = Vec::new();
    let mut module_inputs = BTreeMap::<ModuleKey, (DocumentId, Arc<str>)>::new();
    let mut document_modules = BTreeMap::new();
    for (document_id, text) in inputs {
        let Some(module_key) = module_key_for_roots(config.roots(), &document_id) else {
            continue;
        };
        if let Some((previous, _)) =
            module_inputs.insert(module_key.clone(), (document_id.clone(), text))
        {
            diagnostics.push(ProjectDiagnostic::new(
                Some(document_id.clone()),
                format!(
                    "duplicate module `{}` also provided by `{}`",
                    module_key.path.join(),
                    previous.as_str()
                ),
            ));
        }
        document_modules.insert(document_id, module_key);
    }

    let sources = module_inputs
        .into_iter()
        .enumerate()
        .filter_map(|(index, (key, (_, text)))| {
            source_id(index)
                .map(|id| ModuleSource::new(id, key.package, key.path, text.as_ref()))
                .or_else(|| {
                    diagnostics.push(ProjectDiagnostic::new(
                        None,
                        "too many source files for language-service source ids",
                    ));
                    None
                })
        })
        .collect();

    ProjectSources {
        sources,
        document_modules,
        document_versions,
        package_dependencies: BTreeMap::new(),
        diagnostics,
    }
}

fn assemble_scratch_source(
    document: &DocumentId,
    snapshot: &WorkspaceSnapshot,
    files: &[SourceFileSnapshot],
) -> ProjectSources {
    let text = snapshot.document(document).map_or_else(
        || {
            files
                .iter()
                .find(|file| file.document_id() == document)
                .map(|file| Arc::<str>::from(file.text()))
        },
        |document| Some(Arc::<str>::from(document.text())),
    );
    let Some(text) = text else {
        return ProjectSources {
            sources: Vec::new(),
            document_modules: BTreeMap::new(),
            document_versions: BTreeMap::new(),
            package_dependencies: BTreeMap::new(),
            diagnostics: vec![ProjectDiagnostic::new(
                Some(document.clone()),
                "scratch source is missing",
            )],
        };
    };

    let module_key = ModuleKey::new(PackageId::scratch(), ModulePath::from_qualified("main"));
    let mut document_modules = BTreeMap::new();
    document_modules.insert(document.clone(), module_key.clone());
    let mut document_versions = BTreeMap::new();
    let version = snapshot
        .document(document)
        .map_or(SourceVersion::INITIAL, |document| document.version());
    document_versions.insert(document.clone(), version);
    ProjectSources {
        sources: vec![ModuleSource::new(
            SourceId::new(1),
            module_key.package,
            module_key.path,
            text.as_ref(),
        )],
        document_modules,
        document_versions,
        package_dependencies: BTreeMap::new(),
        diagnostics: Vec::new(),
    }
}

fn module_key_for_roots(roots: &[WorkspaceRoot], document_id: &DocumentId) -> Option<ModuleKey> {
    let document_path = normalize_document_path(document_id.as_str());
    let document_path = canonicalize_existing_ancestor(Path::new(&document_path))
        .unwrap_or_else(|| PathBuf::from(&document_path))
        .display()
        .to_string()
        .replace('\\', "/");
    roots
        .iter()
        .filter_map(|root| {
            let root_path = canonicalize_existing_ancestor(Path::new(root.path()))
                .unwrap_or_else(|| PathBuf::from(root.path()))
                .display()
                .to_string()
                .replace('\\', "/");
            let relative = strip_root(&root_path, &document_path)?;
            module_path_from_relative(relative)
                .map(|path| ModuleKey::new(root.package().clone(), path))
        })
        .max_by_key(|key| key.path.segments().len())
}

fn module_path_from_relative(relative: &str) -> Option<ModulePath> {
    let path = Path::new(relative);
    if path.extension().and_then(|extension| extension.to_str()) != Some(SOURCE_EXTENSION) {
        return None;
    }
    let components = path.components().collect::<Vec<_>>();
    let mut segments = Vec::with_capacity(components.len());
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(component_path) = component else {
            return None;
        };
        let segment = if index + 1 == components.len() {
            Path::new(component_path)
                .file_stem()
                .and_then(|stem| stem.to_str())?
        } else {
            component_path.to_str()?
        };
        if segment.is_empty() {
            return None;
        }
        segments.push(segment.to_owned());
    }
    (!segments.is_empty()).then(|| ModulePath::new(segments))
}

fn parse_config(config_root: &str, text: &str) -> ConfigParseResult {
    let base = config_root_from_document(config_root);
    let parsed = parse_manifest(ManifestFileId::new(1), text);
    let diagnostics = parsed
        .diagnostics
        .into_iter()
        .map(|diagnostic| {
            ProjectDiagnostic::new(
                Some(DocumentId::from(config_root.to_owned())),
                format!(
                    "{} at bytes {}..{}",
                    diagnostic.message, diagnostic.span.start, diagnostic.span.end
                ),
            )
        })
        .collect::<Vec<_>>();
    let Some(manifest) = parsed.manifest else {
        return ConfigParseResult {
            config: WorkspaceConfig::workspace([]),
            diagnostics,
        };
    };
    let roots = manifest
        .package
        .as_ref()
        .map(|_| {
            manifest
                .source
                .roots
                .iter()
                .map(|root| WorkspaceRoot::new(join_config_relative(&base, root)))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut config = WorkspaceConfig::workspace(roots);
    if let Some(schema) = manifest.host.and_then(|host| host.schema) {
        config.set_schema(SchemaConfig::from_path(join_config_relative(
            &base, &schema,
        )));
    }
    ConfigParseResult {
        config,
        diagnostics,
    }
}

fn source_id(index: usize) -> Option<SourceId> {
    u32::try_from(index.checked_add(1)?).ok().map(SourceId::new)
}

fn is_vela_source(path: &str) -> bool {
    Path::new(path.trim_start_matches("file://"))
        .extension()
        .and_then(|extension| extension.to_str())
        == Some(SOURCE_EXTENSION)
}

fn config_root_from_document(config_document: &str) -> String {
    let normalized = normalize_document_path(config_document);
    if normalized.ends_with(CONFIG_FILE) {
        Path::new(&normalized)
            .parent()
            .map(|path| normalize_document_path(&path.display().to_string()))
            .unwrap_or_else(|| ".".to_owned())
    } else {
        normalized
    }
}

fn join_config_relative(base: &str, path: &str) -> String {
    let path = normalize_document_path(path);
    if is_absolute_document_path(&path) {
        path
    } else {
        normalize_document_path(&Path::new(base).join(path).display().to_string())
    }
}

fn strip_root<'a>(root: &str, path: &'a str) -> Option<&'a str> {
    let root = root.trim_end_matches('/');
    if path == root {
        return Some("");
    }
    path.strip_prefix(root)?.strip_prefix('/')
}

fn normalize_document_path(path: &str) -> String {
    let mut path = path.trim_start_matches("file://").replace('\\', "/");
    if cfg!(windows)
        && path.as_bytes().first() == Some(&b'/')
        && path.as_bytes().get(2) == Some(&b':')
    {
        path.remove(0);
    }
    let mut normalized = PathBuf::new();
    for component in Path::new(&path).components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(segment) => normalized.push(segment),
        }
    }
    normalized.display().to_string().replace('\\', "/")
}

fn canonicalize_existing_ancestor(path: &Path) -> Option<PathBuf> {
    if path.exists() {
        return std::fs::canonicalize(path).ok();
    }
    let parent = canonicalize_existing_ancestor(path.parent()?)?;
    Some(parent.join(path.file_name()?))
}

fn is_absolute_document_path(path: &str) -> bool {
    Path::new(path).is_absolute()
}

#[must_use]
pub fn missing_import_diagnostics(project: &ProjectSources) -> Vec<ProjectDiagnostic> {
    let mut graph = vela_hir::module_graph::ModuleGraph::with_package_dependencies(
        project.package_dependencies().clone(),
    );
    for source in project.sources() {
        graph.add_source(source.clone());
    }
    graph.resolve_imports();
    let source_ids = project
        .sources()
        .iter()
        .map(|source| {
            (
                source.id,
                ModuleKey::new(source.package.clone(), source.path.clone()),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let document_by_module = project
        .document_modules()
        .iter()
        .map(|(document_id, module)| (module.clone(), document_id.clone()))
        .collect::<BTreeMap<_, _>>();
    let unresolved_codes = BTreeSet::from([
        "hir::unresolved_module",
        "hir::unresolved_import",
        "hir::empty_import",
    ]);
    graph
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            diagnostic
                .code
                .as_deref()
                .is_some_and(|code| unresolved_codes.contains(code))
        })
        .map(|diagnostic| {
            let source = diagnostic
                .span
                .and_then(|span| source_ids.get(&span.source).cloned())
                .and_then(|module| document_by_module.get(&module).cloned());
            ProjectDiagnostic::new(source, diagnostic.message.clone())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SourceVersion, Workspace};

    fn file(path: &str, text: &str) -> SourceFileSnapshot {
        SourceFileSnapshot::new(path, text)
    }

    #[test]
    fn configured_roots_build_module_paths() {
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let workspace = Workspace::new();
        let project = assemble_project_sources(
            &config,
            &[
                file("/workspace/scripts/game/main.vela", "pub fn main() {}"),
                file("/workspace/scripts/config.vela", "pub const value = 1"),
            ],
            &workspace.snapshot(),
        );

        let modules = project
            .sources()
            .iter()
            .map(|source| source.path.join())
            .collect::<Vec<_>>();
        assert_eq!(modules, vec!["config", "game::main"]);
        assert!(project.diagnostics().is_empty());
    }

    #[test]
    fn scratch_file_uses_single_file_mode() {
        let mut workspace = Workspace::new();
        let document = DocumentId::from("/scratch/current.vela");
        workspace.open_document(
            document.clone(),
            "pub fn main() { return 1 }",
            SourceVersion::new(1),
        );

        let project = assemble_project_sources(
            &WorkspaceConfig::scratch(document),
            &[],
            &workspace.snapshot(),
        );

        assert_eq!(project.sources().len(), 1);
        assert_eq!(project.sources()[0].path.join(), "main");
        assert_eq!(project.sources()[0].text, "pub fn main() { return 1 }");
    }

    #[test]
    fn language_service_scratch_document_uses_reserved_explicit_package() {
        let document = DocumentId::from("/scratch/current.vela");
        let project = assemble_project_sources(
            &WorkspaceConfig::scratch(document.clone()),
            &[file(document.as_str(), "pub fn main() {}")],
            &Workspace::new().snapshot(),
        );

        assert_eq!(project.sources()[0].package, PackageId::scratch());
        assert_eq!(
            project.document_modules().get(&document),
            Some(&ModuleKey::new(
                PackageId::scratch(),
                ModulePath::from_qualified("main"),
            ))
        );
    }

    #[test]
    fn open_overlay_wins_over_disk_source() {
        let mut workspace = Workspace::new();
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        workspace.open_document(
            document.clone(),
            "pub fn main() { return 2 }",
            SourceVersion::new(2),
        );
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);

        let project = assemble_project_sources(
            &config,
            &[file(
                "/workspace/scripts/game/main.vela",
                "pub fn main() { return 1 }",
            )],
            &workspace.snapshot(),
        );

        assert_eq!(project.sources()[0].text, "pub fn main() { return 2 }");
    }

    #[test]
    fn missing_import_reports_diagnostic() {
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let workspace = Workspace::new();
        let project = assemble_project_sources(
            &config,
            &[file(
                "/workspace/scripts/game/main.vela",
                "use game::reward::grant\npub fn main() { grant() }",
            )],
            &workspace.snapshot(),
        );

        let diagnostics = missing_import_diagnostics(&project);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].document_id(),
            Some(&DocumentId::from("/workspace/scripts/game/main.vela"))
        );
        assert!(diagnostics[0].message().contains("unresolved module"));
    }

    #[test]
    fn multi_root_config_keeps_module_paths_stable() {
        let config = WorkspaceConfig::workspace([
            WorkspaceRoot::from("/workspace/scripts"),
            WorkspaceRoot::from("/vendor/scripts"),
        ]);
        let workspace = Workspace::new();
        let project = assemble_project_sources(
            &config,
            &[
                file("/workspace/scripts/game/main.vela", "pub fn main() {}"),
                file("/vendor/scripts/std/prelude.vela", "pub fn help() {}"),
            ],
            &workspace.snapshot(),
        );

        let modules = project
            .sources()
            .iter()
            .map(|source| source.path.join())
            .collect::<Vec<_>>();
        assert_eq!(modules, vec!["game::main", "std::prelude"]);
    }

    #[test]
    fn vela_toml_parses_package_sources_and_schema() {
        let result = WorkspaceConfig::from_vela_toml(
            "/workspace/vela.toml",
            r#"
                [package]
                id = "com.example.workspace"
                name = "workspace"
                version = "0.1.0"

                [source]
                roots = ["scripts", "shared/scripts"]

                [host]
                schema = "target/vela/schema.json"
            "#,
        );

        assert!(result.diagnostics.is_empty());
        assert_eq!(result.config.roots()[0].path(), "/workspace/scripts");
        assert_eq!(result.config.roots()[1].path(), "/workspace/shared/scripts");
        assert_eq!(
            result.config.schema().path(),
            Some("/workspace/target/vela/schema.json")
        );
    }
}
