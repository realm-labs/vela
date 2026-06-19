//! Native LSP protocol boundary for Vela editor tooling.

mod capabilities;
mod client;
mod completion;
mod config;
mod config_change;
mod global_state;
mod handlers;
mod lifecycle;
mod line_index;
mod lsp;
pub mod main_loop;
mod protocol;
mod queries;
mod reload;
mod rpc;
mod semantic_tokens;
pub mod stdio;
mod task;
mod tracing;
pub mod transport;
mod watching;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use lsp_server::{Message, RequestId};
use protocol::{LspPosition, LspRange};
use serde::Deserialize;
use serde_json::{Value as JsonValue, json};
use vela_language_service::{
    DocumentId, LanguageServiceDatabases, ProjectDiagnostic, ProjectSources, SourceFileSnapshot,
    SourceVersion, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
    missing_import_diagnostics,
};

use crate::client::WorkspaceFolder;
use crate::config::EditorConfiguration;
use crate::config_change::ConfigChange;
use crate::lsp::to_proto;
pub use crate::rpc::JsonRpcResult;
pub(crate) use crate::rpc::{ErrorCode, JSONRPC_VERSION};
use crate::semantic_tokens::SemanticTokenProjection;

pub use crate::config::LaunchConfiguration;

const FILE_CHANGE_DELETED: u8 = 3;
const CONFIG_FILE: &str = "vela.toml";
const SOURCE_EXTENSION: &str = ".vela";
const WORKSPACE_DIAGNOSTICS_PROGRESS_TOKEN: &str = "vela/workspace-diagnostics";

#[derive(Debug, Default)]
pub struct LspServer {
    workspace: Workspace,
    databases: LanguageServiceDatabases,
    config: Option<WorkspaceConfig>,
    has_config_file: bool,
    config_diagnostics: Vec<ProjectDiagnostic>,
    config_documents: BTreeSet<DocumentId>,
    schema_documents: BTreeSet<DocumentId>,
    workspace_roots: BTreeSet<String>,
    editor_config: Option<EditorConfiguration>,
    disk_sources: BTreeMap<DocumentId, SourceFileSnapshot>,
    open_documents: BTreeSet<DocumentId>,
    client_supports_work_done_progress: bool,
    client_supports_watched_file_registration: bool,
    watched_files_registered: bool,
    file_watching_disabled: bool,
    semantic_token_projection: SemanticTokenProjection,
    initialized: bool,
    shutdown_requested: bool,
    exited: bool,
}

impl LspServer {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub const fn is_initialized(&self) -> bool {
        self.initialized
    }

    #[must_use]
    pub const fn is_shutdown_requested(&self) -> bool {
        self.shutdown_requested
    }

    #[must_use]
    pub const fn is_exited(&self) -> bool {
        self.exited
    }

    pub fn handle_json(&mut self, input: &str) -> JsonRpcResult {
        if self.exited {
            return JsonRpcResult::None;
        }

        let value = match serde_json::from_str::<JsonValue>(input) {
            Ok(value) => value,
            Err(error) => {
                return JsonRpcResult::error(
                    None,
                    ErrorCode::ParseError,
                    format!("failed to parse JSON-RPC message: {error}"),
                );
            }
        };

        let id = legacy_message_id(&value);
        match value.get("jsonrpc").and_then(JsonValue::as_str) {
            Some(JSONRPC_VERSION) => {}
            Some(_) => {
                return id.map_or(JsonRpcResult::None, |id| {
                    JsonRpcResult::error(
                        Some(id),
                        ErrorCode::InvalidRequest,
                        "unsupported JSON-RPC version",
                    )
                });
            }
            None => {
                return JsonRpcResult::error(
                    None,
                    ErrorCode::ParseError,
                    "failed to parse JSON-RPC message: missing or invalid JSON-RPC version",
                );
            }
        }

        if value.get("method").is_none() {
            if value.get("result").is_some() || value.get("error").is_some() {
                return JsonRpcResult::None;
            }
            return id.map_or(JsonRpcResult::None, |id| {
                JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    "missing JSON-RPC method",
                )
            });
        }

        let message = match transport::message_from_json_rpc(value) {
            Ok(message) => message,
            Err(error) => {
                return JsonRpcResult::error(
                    id,
                    ErrorCode::InvalidRequest,
                    format!("invalid JSON-RPC message: {error}"),
                );
            }
        };

        self.handle_message(message)
    }

    fn handle_message(&mut self, message: Message) -> JsonRpcResult {
        let (id, method, params) = match message {
            Message::Request(request) => (Some(request.id), request.method, request.params),
            Message::Notification(notification) => (None, notification.method, notification.params),
            Message::Response(_) => return JsonRpcResult::None,
        };

        if self.shutdown_requested && method != "exit" {
            return id.map_or(JsonRpcResult::None, |id| {
                JsonRpcResult::error(Some(id), ErrorCode::InvalidRequest, "server has shut down")
            });
        }

        if !self.initialized && !lifecycle::is_pre_initialize_method(&method) {
            return id.map_or(JsonRpcResult::None, |id| {
                JsonRpcResult::error(
                    Some(id),
                    ErrorCode::ServerNotInitialized,
                    "server has not been initialized",
                )
            });
        }

        match method.as_str() {
            "$/cancelRequest" => self.cancel_request(id, params),
            "initialize" => self.initialize(id, params),
            "initialized" => self.initialized(id),
            "shutdown" => self.shutdown(id),
            "exit" => self.exit(id),
            "textDocument/didOpen" => self.did_open(id, params),
            "textDocument/didChange" => self.did_change(id, params),
            "textDocument/didClose" => self.did_close(id, params),
            "textDocument/completion" => self.completion(id, params),
            "completionItem/resolve" => self.completion_resolve(id, params),
            "textDocument/codeAction" => self.code_action(id, params),
            "textDocument/signatureHelp" => self.signature_help(id, params),
            "textDocument/hover" => self.hover(id, params),
            "textDocument/definition" => self.definition(id, params),
            "textDocument/declaration" => self.declaration(id, params),
            "textDocument/typeDefinition" => self.type_definition(id, params),
            "textDocument/references" => self.references(id, params),
            "textDocument/prepareRename" => self.prepare_rename(id, params),
            "textDocument/rename" => self.rename(id, params),
            "textDocument/prepareCallHierarchy" => self.prepare_call_hierarchy(id, params),
            "callHierarchy/incomingCalls" => self.incoming_calls(id, params),
            "callHierarchy/outgoingCalls" => self.outgoing_calls(id, params),
            "textDocument/documentHighlight" => self.document_highlight(id, params),
            "textDocument/documentSymbol" => self.document_symbol(id, params),
            "textDocument/foldingRange" => self.folding_range(id, params),
            "textDocument/formatting" => self.formatting(id, params),
            "textDocument/rangeFormatting" => self.range_formatting(id, params),
            "textDocument/onTypeFormatting" => self.on_type_formatting(id, params),
            "textDocument/selectionRange" => self.selection_range(id, params),
            "textDocument/semanticTokens/full" => self.semantic_tokens_full(id, params),
            "textDocument/semanticTokens/full/delta" => self.semantic_tokens_full_delta(id, params),
            "textDocument/semanticTokens/range" => self.semantic_tokens_range(id, params),
            "textDocument/inlayHint" => self.inlay_hint(id, params),
            "workspace/symbol" => self.workspace_symbol(id, params),
            "workspace/didChangeWatchedFiles" => self.did_change_watched_files(id, params),
            "workspace/didChangeConfiguration" => self.did_change_configuration(id, params),
            "workspace/didChangeWorkspaceFolders" => self.did_change_workspace_folders(id, params),
            method => self.method_not_found(id, method),
        }
    }

    fn did_open(&mut self, id: Option<RequestId>, params: JsonValue) -> JsonRpcResult {
        if let Some(id) = id {
            return JsonRpcResult::error(
                Some(id),
                ErrorCode::InvalidRequest,
                "`textDocument/didOpen` must be sent as a notification",
            );
        }

        let params = match serde_json::from_value::<DidOpenTextDocumentParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::Notification(publish_diagnostics_notification(
                    "",
                    Vec::new(),
                    Some(format!("invalid didOpen params: {error}")),
                ));
            }
        };

        let uri = params.text_document.uri;
        let document_id = DocumentId::from(uri.clone());
        let version = source_version(params.text_document.version);
        self.workspace
            .open_document(document_id.clone(), params.text_document.text, version);
        self.open_documents.insert(document_id.clone());

        self.publish_current_diagnostics(&uri, &document_id)
    }

    fn did_change(&mut self, id: Option<RequestId>, params: JsonValue) -> JsonRpcResult {
        if let Some(id) = id {
            return JsonRpcResult::error(
                Some(id),
                ErrorCode::InvalidRequest,
                "`textDocument/didChange` must be sent as a notification",
            );
        }

        let params = match serde_json::from_value::<DidChangeTextDocumentParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::Notification(publish_diagnostics_notification(
                    "",
                    Vec::new(),
                    Some(format!("invalid didChange params: {error}")),
                ));
            }
        };

        if params.content_changes.is_empty() {
            return JsonRpcResult::Notification(publish_diagnostics_notification(
                &params.text_document.uri,
                Vec::new(),
                Some("didChange requires at least one content change".to_owned()),
            ));
        }

        let uri = params.text_document.uri;
        let document_id = DocumentId::from(uri.clone());
        let version = source_version(params.text_document.version);
        let current_text = self
            .workspace
            .document_text(&document_id)
            .map(std::borrow::ToOwned::to_owned);
        let text = match apply_document_changes(current_text.as_deref(), params.content_changes) {
            Ok(text) => text,
            Err(error) => {
                return JsonRpcResult::Notification(publish_diagnostics_notification(
                    &uri,
                    Vec::new(),
                    Some(error),
                ));
            }
        };

        self.workspace
            .change_document(document_id.clone(), text, version);
        self.open_documents.insert(document_id.clone());

        self.publish_current_diagnostics(&uri, &document_id)
    }

    fn did_close(&mut self, id: Option<RequestId>, params: JsonValue) -> JsonRpcResult {
        if let Some(id) = id {
            return JsonRpcResult::error(
                Some(id),
                ErrorCode::InvalidRequest,
                "`textDocument/didClose` must be sent as a notification",
            );
        }

        let params = match serde_json::from_value::<DidCloseTextDocumentParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::Notification(publish_diagnostics_notification(
                    "",
                    Vec::new(),
                    Some(format!("invalid didClose params: {error}")),
                ));
            }
        };

        let uri = params.text_document.uri;
        let document_id = DocumentId::from(uri.clone());
        self.workspace.close_document(&document_id);
        self.open_documents.remove(&document_id);

        if self.disk_sources.contains_key(&document_id) {
            self.publish_current_diagnostics(&uri, &document_id)
        } else {
            JsonRpcResult::Notification(publish_diagnostics_notification(&uri, Vec::new(), None))
        }
    }

    fn did_change_watched_files(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        if let Some(id) = id {
            return JsonRpcResult::error(
                Some(id),
                ErrorCode::InvalidRequest,
                "`workspace/didChangeWatchedFiles` must be sent as a notification",
            );
        }

        let params = match serde_json::from_value::<DidChangeWatchedFilesParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::Notification(publish_diagnostics_notification(
                    "",
                    Vec::new(),
                    Some(format!("invalid didChangeWatchedFiles params: {error}")),
                ));
            }
        };

        for change in coalesced_watched_file_changes(params.changes) {
            let config_change = if change.kind == FILE_CHANGE_DELETED {
                self.remove_watched_file(&change.uri)
            } else {
                self.upsert_watched_file(&change.uri)
            };
            if let Some(config_change) = config_change {
                self.apply_config_change(config_change);
            }
        }

        let has_open_documents = !self.open_documents.is_empty();
        let result = self.publish_open_diagnostics();
        if has_open_documents && self.client_supports_work_done_progress {
            with_work_done_progress(result, "Vela workspace diagnostics")
        } else {
            result
        }
    }

    fn did_change_workspace_folders(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        if let Some(id) = id {
            return JsonRpcResult::error(
                Some(id),
                ErrorCode::InvalidRequest,
                "`workspace/didChangeWorkspaceFolders` must be sent as a notification",
            );
        }

        let params = match serde_json::from_value::<DidChangeWorkspaceFoldersParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::Notification(publish_diagnostics_notification(
                    "",
                    Vec::new(),
                    Some(format!("invalid didChangeWorkspaceFolders params: {error}")),
                ));
            }
        };

        let mut workspace_roots = self.workspace_roots.clone();
        for folder in params.event.removed {
            let root = WorkspaceRoot::from(folder.uri);
            workspace_roots.remove(root.path());
        }
        for folder in params.event.added {
            let root = WorkspaceRoot::from(folder.uri);
            workspace_roots.insert(root.path().to_owned());
        }
        self.apply_config_change(ConfigChange::from_workspace_roots(workspace_roots));

        let has_open_documents = !self.open_documents.is_empty();
        let result = self.publish_open_diagnostics();
        if has_open_documents && self.client_supports_work_done_progress {
            with_work_done_progress(result, "Vela workspace diagnostics")
        } else {
            result
        }
    }

    fn upsert_watched_file(&mut self, uri: &str) -> Option<ConfigChange> {
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

    fn remove_watched_file(&mut self, uri: &str) -> Option<ConfigChange> {
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

    fn reload_schema_from_config(&mut self) {
        let Some(schema_path) = self
            .config
            .as_ref()
            .and_then(|config| config.schema().path())
            .map(str::to_owned)
        else {
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

    pub(crate) fn schema_path(&self) -> Option<&str> {
        self.config
            .as_ref()
            .and_then(|config| config.schema().path())
    }

    pub(crate) fn publish_open_diagnostics(&mut self) -> JsonRpcResult {
        let mut notifications = Vec::new();

        if !self.open_documents.is_empty() {
            let config = self.config.clone().unwrap_or_else(|| {
                self.open_documents
                    .iter()
                    .next()
                    .cloned()
                    .map_or_else(|| WorkspaceConfig::workspace([]), WorkspaceConfig::scratch)
            });
            let files = self.disk_sources.values().cloned().collect::<Vec<_>>();
            let project = self.update_databases(&config, &files);
            let project_diagnostics = self.current_project_diagnostics(&project);

            notifications.extend(self.open_documents.iter().map(|document_id| {
                let diagnostics = self.databases.diagnostics_for_document(document_id);
                let mut diagnostics = to_proto::diagnostics(&diagnostics);
                diagnostics.extend(to_proto::project_diagnostics(
                    &project_diagnostics,
                    document_id,
                ));
                publish_diagnostics_notification(document_id.as_str(), diagnostics, None)
            }));
        }

        notifications.extend(self.config_diagnostic_notifications());
        notifications.extend(self.schema_diagnostic_notifications());
        if notifications.is_empty() {
            JsonRpcResult::None
        } else {
            JsonRpcResult::Notifications(notifications)
        }
    }

    pub(crate) fn publish_current_diagnostics(
        &mut self,
        uri: &str,
        document_id: &DocumentId,
    ) -> JsonRpcResult {
        let config = self
            .config
            .clone()
            .unwrap_or_else(|| WorkspaceConfig::scratch(document_id.clone()));
        let files = self.disk_sources.values().cloned().collect::<Vec<_>>();
        let project = self.update_databases(&config, &files);
        let diagnostics = self.databases.diagnostics_for_document(document_id);
        let mut diagnostics = to_proto::diagnostics(&diagnostics);
        diagnostics.extend(to_proto::project_diagnostics(
            &self.current_project_diagnostics(&project),
            document_id,
        ));

        JsonRpcResult::Notification(publish_diagnostics_notification(uri, diagnostics, None))
    }

    fn refresh_databases_for_query(&mut self, document_id: &DocumentId) {
        let config = self
            .config
            .clone()
            .unwrap_or_else(|| WorkspaceConfig::scratch(document_id.clone()));
        let files = self.disk_sources.values().cloned().collect::<Vec<_>>();
        self.update_databases(&config, &files);
    }

    fn refresh_databases_for_workspace_query(&mut self) {
        let config = self.config.clone().unwrap_or_else(|| {
            self.open_documents
                .iter()
                .next()
                .cloned()
                .map_or_else(|| WorkspaceConfig::workspace([]), WorkspaceConfig::scratch)
        });
        let files = self.disk_sources.values().cloned().collect::<Vec<_>>();
        self.update_databases(&config, &files);
    }

    fn update_databases(
        &mut self,
        config: &WorkspaceConfig,
        files: &[SourceFileSnapshot],
    ) -> ProjectSources {
        let project = assemble_project_sources(config, files, &self.workspace.snapshot());
        self.databases
            .update_with_open_documents(&project, &self.open_documents);
        project
    }

    fn current_project_diagnostics(&self, project: &ProjectSources) -> Vec<ProjectDiagnostic> {
        let mut diagnostics = self.config_diagnostics.clone();
        diagnostics.extend(project_diagnostics(project));
        diagnostics
    }

    fn config_diagnostic_notifications(&self) -> Vec<String> {
        self.config_documents
            .iter()
            .map(|document_id| {
                publish_diagnostics_notification(
                    document_id.as_str(),
                    to_proto::project_diagnostics(&self.config_diagnostics, document_id),
                    None,
                )
            })
            .collect()
    }

    fn schema_diagnostic_notifications(&self) -> Vec<String> {
        let diagnostics = to_proto::schema_diagnostics(self.databases.schema_db().diagnostics());
        let active_document = self
            .schema_path()
            .map(document_path_uri)
            .map(DocumentId::from);
        self.schema_documents
            .iter()
            .map(|document_id| {
                let diagnostics = if active_document.as_ref() == Some(document_id) {
                    diagnostics.clone()
                } else {
                    Vec::new()
                };
                publish_diagnostics_notification(document_id.as_str(), diagnostics, None)
            })
            .collect()
    }
}

fn legacy_message_id(value: &JsonValue) -> Option<RequestId> {
    value
        .get("id")
        .and_then(|id| transport::request_id_from_json(id).ok())
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DidOpenTextDocumentParams {
    text_document: TextDocumentItem,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TextDocumentItem {
    uri: String,
    #[serde(rename = "languageId")]
    _language_id: String,
    version: i32,
    text: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DidChangeTextDocumentParams {
    text_document: VersionedTextDocumentIdentifier,
    content_changes: Vec<TextDocumentContentChangeEvent>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DidCloseTextDocumentParams {
    text_document: TextDocumentIdentifier,
}

#[derive(Debug, Clone, Deserialize)]
struct TextDocumentIdentifier {
    uri: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VersionedTextDocumentIdentifier {
    uri: String,
    version: i32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TextDocumentContentChangeEvent {
    #[serde(default)]
    range: Option<LspRange>,
    #[serde(rename = "rangeLength")]
    #[allow(dead_code)]
    range_length: Option<u32>,
    text: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DidChangeWatchedFilesParams {
    changes: Vec<FileEvent>,
}

#[derive(Debug, Clone, Deserialize)]
struct DidChangeWorkspaceFoldersParams {
    event: WorkspaceFoldersChangeEvent,
}

#[derive(Debug, Clone, Deserialize)]
struct WorkspaceFoldersChangeEvent {
    added: Vec<WorkspaceFolder>,
    removed: Vec<WorkspaceFolder>,
}

#[derive(Debug, Clone, Deserialize)]
struct FileEvent {
    uri: String,
    #[serde(rename = "type")]
    kind: u8,
}

fn coalesced_watched_file_changes(changes: Vec<FileEvent>) -> Vec<FileEvent> {
    let mut latest_by_uri = std::collections::BTreeMap::<String, (usize, u8)>::new();
    for (index, change) in changes.into_iter().enumerate() {
        latest_by_uri.insert(change.uri, (index, change.kind));
    }

    let mut events = latest_by_uri
        .into_iter()
        .map(|(uri, (index, kind))| (index, FileEvent { uri, kind }))
        .collect::<Vec<_>>();
    events.sort_by_key(|(index, _)| *index);
    events.into_iter().map(|(_, event)| event).collect()
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

fn document_path_uri(path: &str) -> String {
    let path = normalized_path(path);
    if path.starts_with('/') {
        format!("file://{path}")
    } else {
        format!("file:///{path}")
    }
}

pub(crate) fn document_uri_path(uri: &str) -> PathBuf {
    let path = uri.strip_prefix("file://").unwrap_or(uri);
    if cfg!(windows) {
        let path = path.replace('/', "\\");
        let path = path
            .strip_prefix("\\")
            .filter(|path| path.as_bytes().get(1) == Some(&b':'))
            .unwrap_or(&path);
        PathBuf::from(path)
    } else {
        PathBuf::from(path)
    }
}

pub(crate) fn normalized_path(path: impl AsRef<Path>) -> String {
    path.as_ref().display().to_string().replace('\\', "/")
}

fn source_version(version: i32) -> SourceVersion {
    u64::try_from(version)
        .ok()
        .map_or(SourceVersion::INITIAL, SourceVersion::new)
}

fn apply_document_changes(
    current_text: Option<&str>,
    changes: Vec<TextDocumentContentChangeEvent>,
) -> Result<String, String> {
    let mut text = current_text.map(str::to_owned);
    for change in changes {
        match change.range {
            Some(range) => {
                let Some(current) = text.as_mut() else {
                    return Err("ranged didChange requires an open document".to_owned());
                };
                apply_range_edit(current, range, &change.text)?;
            }
            None => {
                text = Some(change.text);
            }
        }
    }
    text.ok_or_else(|| "didChange requires at least one content change".to_owned())
}

pub(crate) fn apply_lsp_document_changes(
    current_text: Option<&str>,
    changes: Vec<lsp_types::TextDocumentContentChangeEvent>,
) -> Result<String, String> {
    apply_document_changes(
        current_text,
        changes
            .into_iter()
            .map(|change| TextDocumentContentChangeEvent {
                range: change.range.map(typed_range_to_local),
                range_length: change.range_length,
                text: change.text,
            })
            .collect(),
    )
}

fn typed_range_to_local(range: lsp_types::Range) -> LspRange {
    LspRange {
        start: typed_position_to_local(range.start),
        end: typed_position_to_local(range.end),
    }
}

fn typed_position_to_local(position: lsp_types::Position) -> LspPosition {
    LspPosition {
        line: position.line,
        character: position.character,
    }
}

fn apply_range_edit(text: &mut String, range: LspRange, replacement: &str) -> Result<(), String> {
    let line_index = line_index::LineIndex::new(text);
    let start = line_index.offset(range.start)?;
    let end = line_index.offset(range.end)?;
    if start > end {
        return Err("didChange range start must not be after the end".to_owned());
    }
    text.replace_range(start..end, replacement);
    Ok(())
}

fn project_diagnostics(project: &ProjectSources) -> Vec<ProjectDiagnostic> {
    let mut diagnostics = project.diagnostics().to_vec();
    diagnostics.extend(missing_import_diagnostics(project));
    diagnostics
}

fn publish_diagnostics_notification(
    uri: &str,
    diagnostics: Vec<lsp_types::Diagnostic>,
    error: Option<String>,
) -> String {
    let uri = lsp_types::Url::parse(uri).expect("diagnostic document URI should parse");
    let params = lsp_types::PublishDiagnosticsParams {
        uri,
        diagnostics,
        version: None,
    };
    let mut params =
        serde_json::to_value(params).expect("typed publishDiagnostics params should serialize");
    if let Some(error) = error
        && let Some(object) = params.as_object_mut()
    {
        object.insert("error".to_owned(), JsonValue::String(error));
    }
    json!({
        "jsonrpc": JSONRPC_VERSION,
        "method": "textDocument/publishDiagnostics",
        "params": params
    })
    .to_string()
}

pub(crate) fn with_work_done_progress(result: JsonRpcResult, title: &str) -> JsonRpcResult {
    let notifications = match result {
        JsonRpcResult::Notification(notification) => vec![notification],
        JsonRpcResult::Notifications(notifications) => notifications,
        other @ (JsonRpcResult::Response(_) | JsonRpcResult::None) => return other,
    };
    if notifications.is_empty() {
        return JsonRpcResult::None;
    }

    let mut wrapped = Vec::with_capacity(notifications.len() + 2);
    wrapped.push(work_done_progress_notification(json!({
        "kind": "begin",
        "title": title,
        "message": "updating open-file diagnostics"
    })));
    wrapped.extend(notifications);
    wrapped.push(work_done_progress_notification(json!({
        "kind": "end",
        "message": "workspace diagnostics updated"
    })));
    JsonRpcResult::Notifications(wrapped)
}

fn work_done_progress_notification(value: JsonValue) -> String {
    json!({
        "jsonrpc": JSONRPC_VERSION,
        "method": "$/progress",
        "params": {
            "token": WORKSPACE_DIAGNOSTICS_PROGRESS_TOKEN,
            "value": value
        }
    })
    .to_string()
}

#[cfg(test)]
mod tests;
