use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use vela_language_service::{
    DocumentId, LanguageServiceDatabases, ProjectDiagnostic, ProjectSources, SourceFileSnapshot,
    Workspace, WorkspaceConfig, WorkspaceSnapshot, assemble_package_project_sources,
    assemble_project_sources, load_package_project, missing_import_diagnostics,
};
use vela_package::PackageGraph;

use crate::{
    LaunchConfiguration,
    config::{EditorConfiguration, workspace_config_from_roots_and_editor_config},
    config_change::{ConfigChange, WorkspaceConfigChange},
    paths::{CONFIG_FILE, SOURCE_EXTENSION, document_path_uri, document_uri_path, normalized_path},
};

#[derive(Debug, Default)]
pub(super) struct ProjectState {
    pub(super) workspace: Workspace,
    pub(super) databases: Arc<LanguageServiceDatabases>,
    pub(super) config: Option<WorkspaceConfig>,
    package_graph: Option<PackageGraph>,
    root_manifest: Option<PathBuf>,
    watched_project_changed: bool,
    has_config_file: bool,
    project_config_update_pending: bool,
    pub(super) config_diagnostics: Vec<vela_language_service::ProjectDiagnostic>,
    pub(super) analysis_diagnostics: Vec<ProjectDiagnostic>,
    pub(super) config_documents: BTreeSet<DocumentId>,
    pub(super) schema_documents: BTreeSet<DocumentId>,
    pub(super) workspace_roots: BTreeSet<String>,
    pub(super) editor_config: Option<EditorConfiguration>,
    pub(super) disk_sources: BTreeMap<DocumentId, SourceFileSnapshot>,
    pub(super) open_documents: BTreeSet<DocumentId>,
}

impl ProjectState {
    pub(super) fn new(configuration: LaunchConfiguration) -> Self {
        let mut state = Self::default();
        state.apply_config_change(ConfigChange::from_launch(configuration));
        state
    }

    pub(super) fn workspace_snapshot(&self) -> WorkspaceSnapshot {
        self.workspace.snapshot()
    }

    #[cfg(test)]
    pub(super) fn package_graph(&self) -> Option<&PackageGraph> {
        self.package_graph.as_ref()
    }

    pub(super) fn apply_config_change(&mut self, mut change: ConfigChange) {
        if let Some(workspace_roots) = change.take_workspace_roots() {
            self.workspace_roots = workspace_roots;
        }
        if let Some(editor_config) = change.take_editor_config() {
            self.editor_config = Some(editor_config);
        }

        match change.workspace_config_change() {
            WorkspaceConfigChange::Unchanged => {}
            WorkspaceConfigChange::RecomputeFromEditor => {
                if !self.has_config_file {
                    self.package_graph = None;
                    self.config = workspace_config_from_roots_and_editor_config(
                        &self.workspace_roots,
                        self.editor_config.as_ref(),
                    );
                    self.project_config_update_pending = true;
                    self.reload_schema_from_config();
                }
            }
            WorkspaceConfigChange::WorkspaceFile(config) => {
                self.has_config_file = true;
                self.config = Some(config);
                self.project_config_update_pending = true;
                self.reload_schema_from_config();
            }
            WorkspaceConfigChange::ClearWorkspaceFile => {
                self.has_config_file = false;
                self.package_graph = None;
                self.config = workspace_config_from_roots_and_editor_config(
                    &self.workspace_roots,
                    self.editor_config.as_ref(),
                );
                self.project_config_update_pending = true;
                self.reload_schema_from_config();
            }
        }
    }

    pub(super) fn upsert_watched_file(&mut self, uri: &str) -> Option<ConfigChange> {
        if is_config_uri(uri) {
            self.reload_package_project(uri)
        } else if self.is_schema_uri(uri) {
            self.upsert_schema_artifact(uri);
            self.watched_project_changed = true;
            None
        } else if is_source_uri(uri) {
            let text = read_document_uri(uri)?;
            let document_id = DocumentId::from(uri.to_owned());
            self.disk_sources.insert(
                document_id.clone(),
                SourceFileSnapshot::new(document_id, text),
            );
            self.watched_project_changed = true;
            None
        } else {
            None
        }
    }

    fn reload_package_project(&mut self, changed_uri: &str) -> Option<ConfigChange> {
        let changed_path = document_uri_path(changed_uri);
        let root_manifest = self.root_manifest_for_change(&changed_path);
        let root_uri = document_path_uri(&root_manifest.display().to_string());
        let text = read_document_uri(&root_uri);
        let mut result = text.as_deref().map_or_else(
            || vela_language_service::ConfigParseResult {
                config: self
                    .config
                    .clone()
                    .unwrap_or_else(|| WorkspaceConfig::workspace([])),
                diagnostics: vec![ProjectDiagnostic::new(
                    Some(DocumentId::from(changed_uri.to_owned())),
                    format!("manifest `{}` cannot be read", root_manifest.display()),
                )],
            },
            |text| WorkspaceConfig::from_vela_toml(&root_uri, text),
        );
        let authorized_roots = self.authorized_package_roots(&root_uri);
        let had_valid_graph = self.package_graph.is_some();
        let loaded = match load_package_project(&root_manifest, &authorized_roots) {
            Ok(graph) => {
                result.config =
                    WorkspaceConfig::from_package_graph(&graph, result.config.schema().clone());
                self.package_graph = Some(graph);
                self.root_manifest = Some(root_manifest.clone());
                self.watched_project_changed = true;
                true
            }
            Err(error) => {
                if result.diagnostics.is_empty() {
                    result.diagnostics.push(ProjectDiagnostic::new(
                        Some(DocumentId::from(changed_uri.to_owned())),
                        error.to_string(),
                    ));
                }
                false
            }
        };
        for document in result
            .diagnostics
            .iter()
            .filter_map(ProjectDiagnostic::document_id)
        {
            self.config_documents.insert(document.clone());
        }
        self.config_diagnostics = result.diagnostics;
        if !loaded && had_valid_graph {
            return None;
        }
        if !loaded {
            self.root_manifest = Some(root_manifest);
            self.watched_project_changed = true;
        }
        Some(ConfigChange::from_workspace_file(result.config))
    }

    pub(super) fn remove_watched_file(&mut self, uri: &str) -> Option<ConfigChange> {
        if is_config_uri(uri) {
            let path = document_uri_path(uri);
            if self
                .root_manifest
                .as_ref()
                .is_some_and(|root| !same_path(root, &path))
            {
                return self.reload_package_project(uri);
            }
            self.package_graph = None;
            self.root_manifest = None;
            self.watched_project_changed = true;
            self.config_diagnostics.clear();
            self.config_documents
                .insert(DocumentId::from(uri.to_owned()));
            Some(ConfigChange::clear_workspace_file())
        } else if self.is_schema_uri(uri) {
            self.mark_schema_artifact_missing();
            self.watched_project_changed = true;
            None
        } else if is_source_uri(uri) {
            self.watched_project_changed |= self
                .disk_sources
                .remove(&DocumentId::from(uri.to_owned()))
                .is_some();
            None
        } else {
            None
        }
    }

    pub(super) fn schema_path(&self) -> Option<&str> {
        self.config
            .as_ref()
            .and_then(|config| config.schema().path())
    }

    /// Mutable access to the shared databases.
    ///
    /// Outstanding [`GlobalStateSnapshot`](super::GlobalStateSnapshot) copies
    /// hold the same allocation, so a write while a background request is in
    /// flight pays one copy here instead of every snapshot paying one.
    pub(super) fn databases_mut(&mut self) -> &mut LanguageServiceDatabases {
        Arc::make_mut(&mut self.databases)
    }

    pub(super) fn refresh_databases(&mut self) {
        let config = self.config.clone().unwrap_or_else(|| {
            self.open_documents
                .iter()
                .next()
                .cloned()
                .map_or_else(|| WorkspaceConfig::workspace([]), WorkspaceConfig::scratch)
        });
        self.refresh_databases_with_config(&config);
    }

    pub(super) fn refresh_document_databases(&mut self, document_id: &DocumentId) {
        let config = self
            .config
            .clone()
            .unwrap_or_else(|| WorkspaceConfig::scratch(document_id.clone()));
        self.refresh_databases_with_config(&config);
    }

    fn refresh_databases_with_config(&mut self, config: &WorkspaceConfig) {
        let files = self.disk_sources.values().cloned().collect::<Vec<_>>();
        let snapshot = self.workspace.snapshot();
        let project = self.package_graph.as_ref().map_or_else(
            || assemble_project_sources(config, &files, &snapshot),
            |graph| assemble_package_project_sources(graph, &files, &snapshot),
        );
        let Self {
            databases,
            open_documents,
            project_config_update_pending,
            ..
        } = self;
        if std::mem::take(project_config_update_pending) {
            Arc::make_mut(databases)
                .update_after_project_config_change_with_open_documents(&project, open_documents);
        } else {
            Arc::make_mut(databases).update_with_open_documents(&project, open_documents);
        }
        self.analysis_diagnostics = project_diagnostics(&project);
    }

    fn reload_schema_from_config(&mut self) {
        let Some(schema_path) = self.schema_path().map(str::to_owned) else {
            self.databases_mut().clear_schema();
            return;
        };
        self.schema_documents
            .insert(DocumentId::from(document_path_uri(&schema_path)));
        match std::fs::read_to_string(&schema_path) {
            Ok(source) => self
                .databases_mut()
                .load_schema_artifact_json(&schema_path, &source),
            Err(_) => self.databases_mut().mark_schema_missing(schema_path),
        }
    }

    fn upsert_schema_artifact(&mut self, uri: &str) {
        let Some(schema_path) = self.schema_path().map(str::to_owned) else {
            return;
        };
        self.schema_documents
            .insert(DocumentId::from(uri.to_owned()));
        match read_document_uri(uri) {
            Some(source) => self
                .databases_mut()
                .load_schema_artifact_json(&schema_path, &source),
            None => self.databases_mut().mark_schema_missing(schema_path),
        }
    }

    fn mark_schema_artifact_missing(&mut self) {
        let Some(schema_path) = self.schema_path().map(str::to_owned) else {
            return;
        };
        self.schema_documents
            .insert(DocumentId::from(document_path_uri(&schema_path)));
        self.databases_mut().mark_schema_missing(schema_path);
    }

    fn is_schema_uri(&self, uri: &str) -> bool {
        self.schema_path().is_some_and(|schema_path| {
            normalized_path(document_uri_path(uri)) == normalized_path(schema_path)
        })
    }

    fn authorized_package_roots(&self, config_uri: &str) -> Vec<PathBuf> {
        let roots = self
            .workspace_roots
            .iter()
            .map(|root| document_uri_path(root))
            .collect::<Vec<_>>();
        if roots.is_empty() {
            document_uri_path(config_uri)
                .parent()
                .map(|parent| vec![parent.to_owned()])
                .unwrap_or_default()
        } else {
            roots
        }
    }

    fn root_manifest_for_change(&self, changed: &std::path::Path) -> PathBuf {
        if let Some(root) = &self.root_manifest {
            return root.clone();
        }
        self.workspace_roots
            .iter()
            .map(|root| document_uri_path(root))
            .filter(|root| changed.starts_with(root))
            .map(|root| root.join(CONFIG_FILE))
            .filter(|manifest| manifest.is_file())
            .max_by_key(|manifest| manifest.components().count())
            .unwrap_or_else(|| changed.to_owned())
    }

    pub(super) fn take_watched_project_changed(&mut self) -> bool {
        std::mem::take(&mut self.watched_project_changed)
    }

    pub(super) fn refresh_databases_after_watched_changes(&mut self) {
        if self.take_watched_project_changed() {
            self.refresh_databases();
        }
    }
}

fn project_diagnostics(project: &ProjectSources) -> Vec<ProjectDiagnostic> {
    let mut diagnostics = project.diagnostics().to_vec();
    diagnostics.extend(missing_import_diagnostics(project));
    diagnostics
}

fn is_config_uri(uri: &str) -> bool {
    uri.trim_end_matches('/').ends_with(CONFIG_FILE)
}

fn is_source_uri(uri: &str) -> bool {
    uri.ends_with(SOURCE_EXTENSION)
}

fn read_document_uri(uri: &str) -> Option<String> {
    std::fs::read_to_string(document_uri_path(uri)).ok()
}

fn same_path(left: &std::path::Path, right: &std::path::Path) -> bool {
    fn comparable(path: &std::path::Path) -> String {
        let path = normalized_path(path);
        let path = path.strip_prefix("//?/").unwrap_or(&path);
        if cfg!(windows) {
            path.to_ascii_lowercase()
        } else {
            path.to_owned()
        }
    }
    comparable(left) == comparable(right)
}
