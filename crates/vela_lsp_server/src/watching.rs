use std::collections::BTreeSet;

use lsp_types::{
    DidChangeWatchedFilesRegistrationOptions, FileSystemWatcher, GlobPattern, OneOf, Registration,
    RegistrationParams, RelativePattern, Unregistration, UnregistrationParams, Url, WatchKind,
    request::{RegisterCapability, Request as LspRequest, UnregisterCapability},
};
use vela_language_service::WorkspaceConfig;

use crate::paths::{CONFIG_FILE, SOURCE_EXTENSION, document_path_uri, normalized_path};

const WATCHED_FILES_REGISTRATION_ID: &str = "vela/watched-files";

#[derive(Default)]
pub(crate) struct RegistrationState {
    initialized: bool,
    watchers: Vec<FileSystemWatcher>,
    registration_id: Option<String>,
    revision: u64,
}

impl RegistrationState {
    pub(crate) fn mark_initialized(&mut self) {
        self.initialized = true;
    }

    pub(crate) fn registered(&self) -> bool {
        self.registration_id.is_some()
    }

    pub(crate) fn refresh(
        &mut self,
        config: Option<&WorkspaceConfig>,
        workspace_roots: &BTreeSet<String>,
        client_supports_registration: bool,
        enabled: bool,
    ) -> Vec<lsp_server::Message> {
        if !self.initialized || !client_supports_registration || !enabled {
            return Vec::new();
        }
        let watchers = watched_file_watchers(config, workspace_roots);
        if watchers == self.watchers {
            return Vec::new();
        }
        let mut messages = Vec::new();
        if let Some(previous) = self.registration_id.take() {
            messages.push(unregistration_request(previous, self.revision));
        }
        if !watchers.is_empty() {
            let id = if self.revision == 0 {
                WATCHED_FILES_REGISTRATION_ID.to_owned()
            } else {
                format!("{WATCHED_FILES_REGISTRATION_ID}/{}", self.revision)
            };
            messages.push(registration_request(watchers.clone(), &id));
            self.registration_id = Some(id);
        }
        self.watchers = watchers;
        self.revision = self.revision.saturating_add(1);
        messages
    }
}

fn registration_request(watchers: Vec<FileSystemWatcher>, id: &str) -> lsp_server::Message {
    let register_options = DidChangeWatchedFilesRegistrationOptions { watchers };
    let params = RegistrationParams {
        registrations: vec![Registration {
            id: id.to_owned(),
            method: "workspace/didChangeWatchedFiles".to_owned(),
            register_options: Some(
                serde_json::to_value(register_options)
                    .expect("watched-files registration options should serialize"),
            ),
        }],
    };
    let request = lsp_server::Request {
        id: lsp_server::RequestId::from(id.to_owned()),
        method: RegisterCapability::METHOD.to_owned(),
        params: serde_json::to_value(params).expect("registration params should serialize"),
    };
    lsp_server::Message::Request(request)
}

fn unregistration_request(previous: String, revision: u64) -> lsp_server::Message {
    let params = UnregistrationParams {
        unregisterations: vec![Unregistration {
            id: previous,
            method: "workspace/didChangeWatchedFiles".to_owned(),
        }],
    };
    lsp_server::Message::Request(lsp_server::Request {
        id: lsp_server::RequestId::from(format!(
            "{WATCHED_FILES_REGISTRATION_ID}/unregister/{revision}"
        )),
        method: UnregisterCapability::METHOD.to_owned(),
        params: serde_json::to_value(params).expect("unregistration params should serialize"),
    })
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
