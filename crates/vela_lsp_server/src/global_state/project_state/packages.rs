use std::path::PathBuf;

use vela_language_service::{DocumentId, ProjectDiagnostic, SourceFileSnapshot, WorkspaceConfig};

use super::{ProjectState, read_document_uri, same_path};
use crate::{
    config::workspace_config_from_roots_and_editor_config,
    config_change::ConfigChange,
    paths::{CONFIG_FILE, document_path_uri, document_uri_path, workspace_document_uri},
};

impl ProjectState {
    pub(in crate::global_state) fn load_initial_project(&mut self) {
        let manifests = self
            .workspace_roots
            .iter()
            .map(|root| document_uri_path(root).join(CONFIG_FILE))
            .filter(|path| path.is_file())
            .collect::<Vec<_>>();
        if let Some(manifest) = manifests.first()
            && let Some(change) =
                self.reload_package_project(&document_path_uri(&manifest.display().to_string()))
        {
            self.apply_config_change(change);
        }
    }

    pub(super) fn reload_package_project(&mut self, changed_uri: &str) -> Option<ConfigChange> {
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
        let mut manifests = self.workspace_manifests();
        if !manifests
            .iter()
            .any(|manifest| same_path(manifest, &root_manifest))
        {
            manifests.push(root_manifest.clone());
        }
        let loaded = match vela_package::load_package_graphs(&manifests, &authorized_roots) {
            Ok(graph) => {
                self.disk_sources = graph
                    .sources()
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
                    .collect();
                result.config =
                    WorkspaceConfig::from_package_graph(&graph, result.config.schema().clone());
                let fallback = self
                    .workspace_roots
                    .iter()
                    .filter(|root| {
                        !manifests.iter().any(|manifest| {
                            manifest
                                .parent()
                                .is_some_and(|parent| same_path(parent, &document_uri_path(root)))
                        })
                    })
                    .cloned()
                    .collect();
                if let Some(config) = workspace_config_from_roots_and_editor_config(&fallback, None)
                {
                    let schema = result.config.schema().clone();
                    result.config = WorkspaceConfig::workspace(
                        result.config.roots().iter().chain(config.roots()).cloned(),
                    );
                    result.config.set_schema(schema);
                }
                self.package_graph = Some(graph);
                self.root_manifest = Some(root_manifest.clone());
                self.watched_project_changed = true;
                true
            }
            Err(error) => {
                if let vela_package::PackageGraphError::Manifest { path, diagnostics } = &error
                    && !same_path(path, &root_manifest)
                {
                    let uri = workspace_document_uri(path, &self.workspace_roots);
                    result.diagnostics = diagnostics
                        .iter()
                        .map(|diagnostic| {
                            ProjectDiagnostic::new(
                                Some(DocumentId::from(uri.clone())),
                                format!(
                                    "{} at bytes {}..{}",
                                    diagnostic.message, diagnostic.span.start, diagnostic.span.end
                                ),
                            )
                        })
                        .collect();
                }
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

    pub(super) fn workspace_manifests(&self) -> Vec<PathBuf> {
        self.workspace_roots
            .iter()
            .map(|root| document_uri_path(root).join(CONFIG_FILE))
            .filter(|manifest| manifest.is_file())
            .collect()
    }
}
