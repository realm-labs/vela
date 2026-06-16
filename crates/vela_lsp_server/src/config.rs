use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value as JsonValue;
use vela_language_service::{SchemaConfig, WorkspaceConfig, WorkspaceRoot};

use crate::{
    ErrorCode, JsonRpcResult, LspServer, RequestId, document_uri_path, error_response,
    normalized_path, publish_diagnostics_notification,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LaunchConfiguration {
    workspace_roots: Vec<String>,
    host_schema: Option<String>,
}

impl LaunchConfiguration {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.workspace_roots.is_empty() && self.host_schema.is_none()
    }

    pub fn add_workspace_root(&mut self, root: impl Into<String>) {
        self.workspace_roots.push(root.into());
    }

    pub fn set_host_schema(&mut self, schema: impl Into<String>) {
        self.host_schema = Some(schema.into());
    }

    #[must_use]
    pub fn workspace_roots(&self) -> &[String] {
        &self.workspace_roots
    }

    #[must_use]
    pub fn host_schema(&self) -> Option<&str> {
        self.host_schema.as_deref()
    }

    fn into_editor_configuration(self) -> Option<EditorConfiguration> {
        (!self.is_empty()).then(|| EditorConfiguration {
            workspace: (!self.workspace_roots.is_empty()).then_some(EditorWorkspaceConfiguration {
                roots: Some(self.workspace_roots),
            }),
            host: self.host_schema.map(|schema| EditorHostConfiguration {
                schema: Some(schema),
            }),
            workspace_roots: None,
            host_schema: None,
        })
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EditorConfiguration {
    workspace: Option<EditorWorkspaceConfiguration>,
    host: Option<EditorHostConfiguration>,
    workspace_roots: Option<Vec<String>>,
    host_schema: Option<String>,
}

impl EditorConfiguration {
    pub(crate) fn from_settings(settings: JsonValue) -> Result<Self, serde_json::Error> {
        let settings = settings.get("vela").cloned().unwrap_or(settings);
        serde_json::from_value(settings)
    }

    fn workspace_roots(&self) -> Option<&[String]> {
        self.workspace
            .as_ref()
            .and_then(|workspace| workspace.roots.as_deref())
            .or(self.workspace_roots.as_deref())
    }

    fn host_schema(&self) -> Option<&str> {
        self.host
            .as_ref()
            .and_then(|host| host.schema.as_deref())
            .or(self.host_schema.as_deref())
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EditorWorkspaceConfiguration {
    roots: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EditorHostConfiguration {
    schema: Option<String>,
}

pub(crate) fn workspace_config_from_roots_and_editor_config(
    lsp_roots: &BTreeSet<String>,
    editor_config: Option<&EditorConfiguration>,
) -> Option<WorkspaceConfig> {
    let roots = editor_config
        .and_then(EditorConfiguration::workspace_roots)
        .filter(|roots| !roots.is_empty())
        .map(|roots| normalize_roots(roots, lsp_roots))
        .unwrap_or_else(|| lsp_roots.clone());

    let schema = editor_config
        .and_then(EditorConfiguration::host_schema)
        .map(|schema| normalize_path_or_uri(schema, roots.iter().next().map(String::as_str)));

    if roots.is_empty() && schema.is_none() {
        return None;
    }

    let mut config = WorkspaceConfig::workspace(roots.iter().cloned().map(WorkspaceRoot::from));
    if let Some(schema) = schema {
        config.set_schema(SchemaConfig::from_path(schema));
    }
    Some(config)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DidChangeConfigurationParams {
    settings: JsonValue,
}

impl LspServer {
    #[must_use]
    pub fn with_launch_configuration(configuration: LaunchConfiguration) -> Self {
        let mut server = Self::new();
        server.editor_config = configuration.into_editor_configuration();
        server.config = workspace_config_from_roots_and_editor_config(
            &server.workspace_roots,
            server.editor_config.as_ref(),
        );
        server.reload_schema_from_config();
        server
    }

    pub(crate) fn did_change_configuration(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        if let Some(id) = id {
            return JsonRpcResult::Response(error_response(
                Some(id),
                ErrorCode::InvalidRequest,
                "`workspace/didChangeConfiguration` must be sent as a notification",
            ));
        }

        let params = match serde_json::from_value::<DidChangeConfigurationParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::Notification(publish_diagnostics_notification(
                    "",
                    Vec::new(),
                    Some(format!("invalid didChangeConfiguration params: {error}")),
                ));
            }
        };
        let editor_config = match EditorConfiguration::from_settings(params.settings) {
            Ok(config) => config,
            Err(error) => {
                return JsonRpcResult::Notification(publish_diagnostics_notification(
                    "",
                    Vec::new(),
                    Some(format!("invalid didChangeConfiguration settings: {error}")),
                ));
            }
        };

        self.editor_config = Some(editor_config);
        if !self.has_config_file {
            self.config = workspace_config_from_roots_and_editor_config(
                &self.workspace_roots,
                self.editor_config.as_ref(),
            );
            self.databases.invalidate_project_config();
            self.reload_schema_from_config();
        }
        self.publish_open_diagnostics()
    }
}

fn normalize_roots(roots: &[String], lsp_roots: &BTreeSet<String>) -> BTreeSet<String> {
    let base = lsp_roots.iter().next().map(String::as_str);
    roots
        .iter()
        .map(|root| normalize_path_or_uri(root, base))
        .collect()
}

fn normalize_path_or_uri(value: &str, base: Option<&str>) -> String {
    let path = if value.starts_with("file://") {
        document_uri_path(value)
    } else {
        PathBuf::from(value)
    };
    let path = if path.is_absolute() {
        path
    } else {
        base.map_or(path.clone(), |base| Path::new(base).join(path))
    };
    normalized_path(path)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{EditorConfiguration, workspace_config_from_roots_and_editor_config};

    #[test]
    fn editor_configuration_reads_nested_vela_settings() {
        let config = EditorConfiguration::from_settings(serde_json::json!({
            "vela": {
                "workspace": {
                    "roots": ["file:///workspace/scripts"]
                },
                "host": {
                    "schema": "file:///workspace/target/vela/schema.json"
                }
            }
        }))
        .expect("nested vela settings should deserialize");
        let workspace =
            workspace_config_from_roots_and_editor_config(&BTreeSet::new(), Some(&config))
                .expect("settings should produce workspace config");

        assert_eq!(workspace.roots()[0].path(), "/workspace/scripts");
        assert_eq!(
            workspace.schema().path(),
            Some("/workspace/target/vela/schema.json")
        );
    }
}
