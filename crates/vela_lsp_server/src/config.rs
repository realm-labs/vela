use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use vela_language_service::{SchemaConfig, WorkspaceConfig, WorkspaceRoot};

use crate::{document_uri_path, normalized_path};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EditorConfiguration {
    workspace: Option<EditorWorkspaceConfiguration>,
    host: Option<EditorHostConfiguration>,
    workspace_roots: Option<Vec<String>>,
    host_schema: Option<String>,
}

impl EditorConfiguration {
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
