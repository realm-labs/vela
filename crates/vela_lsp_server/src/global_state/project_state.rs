use std::collections::{BTreeMap, BTreeSet};

use vela_language_service::{
    DocumentId, LanguageServiceDatabases, ProjectDiagnostic, ProjectSources, SourceFileSnapshot,
    Workspace, WorkspaceConfig, WorkspaceSnapshot, assemble_project_sources,
    missing_import_diagnostics,
};

use crate::{
    LaunchConfiguration,
    config::{EditorConfiguration, workspace_config_from_roots_and_editor_config},
    config_change::{ConfigChange, WorkspaceConfigChange},
    paths::{CONFIG_FILE, SOURCE_EXTENSION, document_path_uri, document_uri_path, normalized_path},
};

#[derive(Debug, Default)]
pub(super) struct ProjectState {
    pub(super) workspace: Workspace,
    pub(super) databases: LanguageServiceDatabases,
    pub(super) config: Option<WorkspaceConfig>,
    has_config_file: bool,
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
                    self.config = workspace_config_from_roots_and_editor_config(
                        &self.workspace_roots,
                        self.editor_config.as_ref(),
                    );
                    self.databases.invalidate_project_config();
                    self.reload_schema_from_config();
                }
            }
            WorkspaceConfigChange::WorkspaceFile(config) => {
                self.has_config_file = true;
                self.config = Some(config);
                self.databases.invalidate_project_config();
                self.reload_schema_from_config();
            }
            WorkspaceConfigChange::ClearWorkspaceFile => {
                self.has_config_file = false;
                self.config = workspace_config_from_roots_and_editor_config(
                    &self.workspace_roots,
                    self.editor_config.as_ref(),
                );
                self.databases.invalidate_project_config();
                self.reload_schema_from_config();
            }
        }
    }

    pub(super) fn upsert_watched_file(&mut self, uri: &str) -> Option<ConfigChange> {
        if is_config_uri(uri) {
            let text = read_document_uri(uri)?;
            let document_id = DocumentId::from(uri.to_owned());
            let result = WorkspaceConfig::from_vela_toml(uri, &text);
            if !result.diagnostics.is_empty() || self.config_documents.contains(&document_id) {
                self.config_documents.insert(document_id);
            }
            self.config_diagnostics = result.diagnostics;
            Some(ConfigChange::from_workspace_file(result.config))
        } else if self.is_schema_uri(uri) {
            self.upsert_schema_artifact(uri);
            None
        } else if is_source_uri(uri) {
            let text = read_document_uri(uri)?;
            let document_id = DocumentId::from(uri.to_owned());
            self.disk_sources.insert(
                document_id.clone(),
                SourceFileSnapshot::new(document_id, text),
            );
            None
        } else {
            None
        }
    }

    pub(super) fn remove_watched_file(&mut self, uri: &str) -> Option<ConfigChange> {
        if is_config_uri(uri) {
            self.config_diagnostics.clear();
            self.config_documents
                .insert(DocumentId::from(uri.to_owned()));
            Some(ConfigChange::clear_workspace_file())
        } else if self.is_schema_uri(uri) {
            self.mark_schema_artifact_missing();
            None
        } else if is_source_uri(uri) {
            self.disk_sources.remove(&DocumentId::from(uri.to_owned()));
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
        let project = assemble_project_sources(config, &files, &self.workspace.snapshot());
        self.databases
            .update_with_open_documents(&project, &self.open_documents);
        self.analysis_diagnostics = project_diagnostics(&project);
    }

    fn reload_schema_from_config(&mut self) {
        let Some(schema_path) = self.schema_path().map(str::to_owned) else {
            self.databases.clear_schema();
            return;
        };
        self.schema_documents
            .insert(DocumentId::from(document_path_uri(&schema_path)));
        match std::fs::read_to_string(&schema_path) {
            Ok(source) => self
                .databases
                .load_schema_artifact_json(&schema_path, &source),
            Err(_) => self.databases.mark_schema_missing(schema_path),
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
                .databases
                .load_schema_artifact_json(&schema_path, &source),
            None => self.databases.mark_schema_missing(schema_path),
        }
    }

    fn mark_schema_artifact_missing(&mut self) {
        let Some(schema_path) = self.schema_path().map(str::to_owned) else {
            return;
        };
        self.schema_documents
            .insert(DocumentId::from(document_path_uri(&schema_path)));
        self.databases.mark_schema_missing(schema_path);
    }

    fn is_schema_uri(&self, uri: &str) -> bool {
        self.schema_path().is_some_and(|schema_path| {
            normalized_path(document_uri_path(uri)) == normalized_path(schema_path)
        })
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
