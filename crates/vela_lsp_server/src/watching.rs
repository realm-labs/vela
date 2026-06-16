use std::collections::BTreeSet;

use serde_json::{Value as JsonValue, json};
use vela_language_service::WorkspaceConfig;

use crate::{
    CONFIG_FILE, JSONRPC_VERSION, RequestId, SOURCE_EXTENSION, document_path_uri, normalized_path,
};

const FILE_WATCHER_KIND_ALL: u8 = 7;
const WATCHED_FILES_REGISTRATION_ID: &str = "vela/watched-files";

pub(crate) fn registration_request(
    config: Option<&WorkspaceConfig>,
    workspace_roots: &BTreeSet<String>,
) -> Option<String> {
    let watchers = watched_file_watchers(config, workspace_roots);
    if watchers.is_empty() {
        return None;
    }

    Some(
        json!({
            "jsonrpc": JSONRPC_VERSION,
            "id": RequestId::String(WATCHED_FILES_REGISTRATION_ID.to_owned()),
            "method": "client/registerCapability",
            "params": {
                "registrations": [
                    {
                        "id": WATCHED_FILES_REGISTRATION_ID,
                        "method": "workspace/didChangeWatchedFiles",
                        "registerOptions": {
                            "watchers": watchers
                        }
                    }
                ]
            }
        })
        .to_string(),
    )
}

fn watched_file_watchers(
    config: Option<&WorkspaceConfig>,
    workspace_roots: &BTreeSet<String>,
) -> Vec<JsonValue> {
    let mut watchers = Vec::new();

    for root in source_roots(config, workspace_roots) {
        watchers.push(relative_file_watcher(
            document_path_uri(&root),
            format!("**/*{SOURCE_EXTENSION}"),
        ));
    }

    for root in config_roots(config, workspace_roots) {
        watchers.push(relative_file_watcher(document_path_uri(&root), CONFIG_FILE));
    }

    if let Some(schema) = config.and_then(|config| config.schema().path()) {
        watchers.push(exact_file_watcher(schema));
    }

    watchers
}

fn source_roots(
    config: Option<&WorkspaceConfig>,
    workspace_roots: &BTreeSet<String>,
) -> BTreeSet<String> {
    config
        .map(config_roots_from_workspace)
        .filter(|roots| !roots.is_empty())
        .unwrap_or_else(|| workspace_roots.clone())
}

fn config_roots(
    config: Option<&WorkspaceConfig>,
    workspace_roots: &BTreeSet<String>,
) -> BTreeSet<String> {
    if workspace_roots.is_empty() {
        config.map(config_roots_from_workspace).unwrap_or_default()
    } else {
        workspace_roots.clone()
    }
}

fn config_roots_from_workspace(config: &WorkspaceConfig) -> BTreeSet<String> {
    config
        .roots()
        .iter()
        .map(|root| root.path().to_owned())
        .collect()
}

fn relative_file_watcher(base_uri: String, pattern: impl Into<String>) -> JsonValue {
    json!({
        "globPattern": {
            "baseUri": base_uri,
            "pattern": pattern.into()
        },
        "kind": FILE_WATCHER_KIND_ALL
    })
}

fn exact_file_watcher(path: &str) -> JsonValue {
    json!({
        "globPattern": normalized_path(path),
        "kind": FILE_WATCHER_KIND_ALL
    })
}
