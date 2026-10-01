mod packages;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use vela_language_service::{
    DocumentId, LanguageServiceDatabases, ProjectDiagnostic, SourceFileSnapshot, Workspace,
    WorkspaceConfig, WorkspaceSnapshot, assemble_package_project_sources_with_config,
    assemble_project_sources,
};
use vela_package::PackageGraph;

use crate::{
    LaunchConfiguration,
    config::{EditorConfiguration, workspace_config_from_roots_and_editor_config},
    config_change::{ConfigChange, WorkspaceConfigChange},
    paths::{
        CONFIG_FILE, SOURCE_EXTENSION, document_path_uri, document_uri_path, normalized_path,
        same_file_path, workspace_document_uri,
    },
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
    pub(super) source_diagnostics: Vec<ProjectDiagnostic>,
    pub(super) source_documents: BTreeSet<DocumentId>,
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

    pub(super) fn reload_workspace_sources(&mut self) {
        self.source_diagnostics.clear();
        let roots = self.config.as_ref().map_or_else(Vec::new, |config| {
            config
                .roots()
                .iter()
                .filter(|root| {
                    self.package_graph.is_none()
                        || root.package() == &vela_package::PackageId::anonymous()
                })
                .map(|root| document_uri_path(root.path()))
                .collect()
        });
        match vela_package::load_workspace_sources(&roots) {
            Ok(sources) => {
                let discovered = sources
                    .sources()
                    .iter()
                    .map(|source| {
                        let document = DocumentId::from(workspace_document_uri(
                            &source.path,
                            &self.workspace_roots,
                        ));
                        (
                            document.clone(),
                            SourceFileSnapshot::new(document, source.text.as_str()),
                        )
                    })
                    .collect::<BTreeMap<_, _>>();
                if let Some(graph) = &self.package_graph {
                    let package_documents = graph
                        .sources()
                        .sources()
                        .iter()
                        .map(|source| {
                            DocumentId::from(workspace_document_uri(
                                &source.path,
                                &self.workspace_roots,
                            ))
                        })
                        .collect::<BTreeSet<_>>();
                    self.disk_sources
                        .retain(|document, _| package_documents.contains(document));
                    self.disk_sources.extend(discovered);
                } else {
                    self.disk_sources = discovered;
                }
                self.watched_project_changed = true;
            }
            Err(vela_package::PackageGraphError::Io { path, message }) => {
                let document =
                    DocumentId::from(workspace_document_uri(&path, &self.workspace_roots));
                self.source_documents.insert(document.clone());
                self.source_diagnostics.push(ProjectDiagnostic::new(
                    Some(document),
                    format!("{}: {message}", path.display()),
                ));
            }
            Err(error) => unreachable!("source discovery only produces I/O errors: {error}"),
        }
    }

    pub(super) fn reindex_workspace_roots(&mut self, roots: BTreeSet<String>) {
        self.package_graph = None;
        self.root_manifest = None;
        self.has_config_file = false;
        self.config_diagnostics.clear();
        self.apply_config_change(ConfigChange::from_workspace_roots(roots));
        self.load_initial_project();
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
            let document_id = DocumentId::from(workspace_document_uri(
                &document_uri_path(uri),
                &self.workspace_roots,
            ));
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

    pub(super) fn remove_watched_file(&mut self, uri: &str) -> Option<ConfigChange> {
        if is_config_uri(uri) {
            let path = document_uri_path(uri);
            if self
                .workspace_manifests()
                .iter()
                .any(|manifest| !same_path(manifest, &path))
            {
                self.root_manifest = self
                    .workspace_manifests()
                    .into_iter()
                    .find(|manifest| !same_path(manifest, &path));
                return self.reload_package_project(uri);
            }
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
                .remove(&DocumentId::from(workspace_document_uri(
                    &document_uri_path(uri),
                    &self.workspace_roots,
                )))
                .is_some();
            None
        } else {
            None
        }
    }

    pub(super) fn restore_closed_source_from_disk(&mut self, uri: &str) {
        if !is_source_uri(uri) {
            return;
        }
        let document_id = DocumentId::from(workspace_document_uri(
            &document_uri_path(uri),
            &self.workspace_roots,
        ));
        if let Some(text) = read_document_uri(uri) {
            self.disk_sources.insert(
                document_id.clone(),
                SourceFileSnapshot::new(document_id, text),
            );
        } else {
            self.disk_sources.remove(&document_id);
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
        if self.project_config_update_pending {
            self.reload_workspace_sources();
        }
        let files = self.disk_sources.values().cloned().collect::<Vec<_>>();
        let snapshot = self.workspace.snapshot();
        let project = self.package_graph.as_ref().map_or_else(
            || assemble_project_sources(config, &files, &snapshot),
            |graph| assemble_package_project_sources_with_config(graph, config, &files, &snapshot),
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
        self.analysis_diagnostics = project.diagnostics().to_vec();
        // Any completed refresh already incorporates pending disk/config work,
        // including the initial package load and document-driven refreshes.
        self.watched_project_changed = false;
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
            .insert(DocumentId::from(document_path_uri(&schema_path)));
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
        self.schema_path()
            .is_some_and(|schema_path| same_file_path(document_uri_path(uri), schema_path))
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
