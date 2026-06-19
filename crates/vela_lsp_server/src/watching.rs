use std::collections::BTreeSet;

use lsp_types::{
    DidChangeWatchedFilesRegistrationOptions, FileSystemWatcher, GlobPattern, OneOf, Registration,
    RegistrationParams, RelativePattern, Url, WatchKind,
    request::{RegisterCapability, Request as LspRequest},
};
use vela_language_service::WorkspaceConfig;

use crate::{CONFIG_FILE, SOURCE_EXTENSION, document_path_uri, normalized_path, transport};

const WATCHED_FILES_REGISTRATION_ID: &str = "vela/watched-files";

pub(crate) fn registration_request(
    config: Option<&WorkspaceConfig>,
    workspace_roots: &BTreeSet<String>,
) -> Option<String> {
    let watchers = watched_file_watchers(config, workspace_roots);
    if watchers.is_empty() {
        return None;
    }

    let register_options = DidChangeWatchedFilesRegistrationOptions { watchers };
    let params = RegistrationParams {
        registrations: vec![Registration {
            id: WATCHED_FILES_REGISTRATION_ID.to_owned(),
            method: "workspace/didChangeWatchedFiles".to_owned(),
            register_options: Some(
                serde_json::to_value(register_options)
                    .expect("watched-files registration options should serialize"),
            ),
        }],
    };
    let request = lsp_server::Request {
        id: lsp_server::RequestId::from(WATCHED_FILES_REGISTRATION_ID.to_owned()),
        method: RegisterCapability::METHOD.to_owned(),
        params: serde_json::to_value(params).expect("registration params should serialize"),
    };
    Some(
        transport::serialize_json_rpc_message(&lsp_server::Message::Request(request))
            .expect("registration request should serialize"),
    )
}

fn watched_file_watchers(
    config: Option<&WorkspaceConfig>,
    workspace_roots: &BTreeSet<String>,
) -> Vec<FileSystemWatcher> {
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

fn relative_file_watcher(base_uri: String, pattern: impl Into<String>) -> FileSystemWatcher {
    FileSystemWatcher {
        glob_pattern: GlobPattern::Relative(RelativePattern {
            base_uri: OneOf::Right(Url::parse(&base_uri).expect("base URI should parse")),
            pattern: pattern.into(),
        }),
        kind: Some(watch_all_kinds()),
    }
}

fn exact_file_watcher(path: &str) -> FileSystemWatcher {
    FileSystemWatcher {
        glob_pattern: GlobPattern::String(normalized_path(path)),
        kind: Some(watch_all_kinds()),
    }
}

fn watch_all_kinds() -> WatchKind {
    WatchKind::Create | WatchKind::Change | WatchKind::Delete
}
