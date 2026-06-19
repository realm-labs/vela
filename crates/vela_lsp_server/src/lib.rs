//! Native LSP protocol boundary for Vela editor tooling.

mod call_hierarchy;
mod capabilities;
mod client;
mod completion;
mod config;
mod config_change;
mod definition;
mod global_state;
mod handlers;
mod lifecycle;
mod line_index;
mod lsp;
pub mod main_loop;
mod protocol;
mod queries;
mod references;
mod reload;
mod rename;
mod rpc;
mod semantic_tokens;
pub mod stdio;
mod symbols;
mod tracing;
pub mod transport;
mod watching;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use protocol::{LspPosition, LspRange};
use serde::Deserialize;
use serde_json::{Value as JsonValue, json};
use vela_language_service::{
    DocumentDiagnostics, DocumentId, LanguageServiceDatabases, ProjectDiagnostic, ProjectSources,
    SchemaDiagnostic, ServiceDiagnostic, ServiceDiagnosticSeverity, SourceFileSnapshot,
    SourceVersion, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
    missing_import_diagnostics,
};

use crate::client::WorkspaceFolder;
use crate::config::EditorConfiguration;
use crate::config_change::ConfigChange;
pub use crate::rpc::JsonRpcResult;
pub(crate) use crate::rpc::{
    ErrorCode, JSONRPC_VERSION, JsonRpcMessage, RequestId, error_response, success_response,
};
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
    cancelled_requests: BTreeSet<RequestId>,
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

        let message = match serde_json::from_str::<JsonRpcMessage>(input) {
            Ok(message) => message,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    None,
                    ErrorCode::ParseError,
                    format!("failed to parse JSON-RPC message: {error}"),
                ));
            }
        };

        self.handle_message(message)
    }

    fn handle_message(&mut self, message: JsonRpcMessage) -> JsonRpcResult {
        if message.jsonrpc != JSONRPC_VERSION {
            return message.id.map_or(JsonRpcResult::None, |id| {
                JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    "unsupported JSON-RPC version",
                ))
            });
        }

        if let Some(id) = message.id.as_ref()
            && self.cancelled_requests.remove(id)
        {
            return JsonRpcResult::Response(error_response(
                message.id,
                ErrorCode::RequestCancelled,
                "request was cancelled before processing",
            ));
        }

        let Some(method) = message.method.as_deref() else {
            if message.extra.contains_key("result") || message.extra.contains_key("error") {
                return JsonRpcResult::None;
            }
            return message.id.map_or(JsonRpcResult::None, |id| {
                JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    "missing JSON-RPC method",
                ))
            });
        };

        if self.shutdown_requested && method != "exit" {
            return message.id.map_or(JsonRpcResult::None, |id| {
                JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    "server has shut down",
                ))
            });
        }

        if !self.initialized && !lifecycle::is_pre_initialize_method(method) {
            return message.id.map_or(JsonRpcResult::None, |id| {
                JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::ServerNotInitialized,
                    "server has not been initialized",
                ))
            });
        }

        match method {
            "$/cancelRequest" => self.cancel_request(message.id, message.params),
            "initialize" => self.initialize(message.id, message.params),
            "initialized" => self.initialized(message.id),
            "shutdown" => self.shutdown(message.id),
            "exit" => self.exit(message.id),
            "textDocument/didOpen" => self.did_open(message.id, message.params),
            "textDocument/didChange" => self.did_change(message.id, message.params),
            "textDocument/didClose" => self.did_close(message.id, message.params),
            "textDocument/completion" => self.completion(message.id, message.params),
            "completionItem/resolve" => self.completion_resolve(message.id, message.params),
            "textDocument/codeAction" => self.code_action(message.id, message.params),
            "textDocument/signatureHelp" => self.signature_help(message.id, message.params),
            "textDocument/hover" => self.hover(message.id, message.params),
            "textDocument/definition" => self.definition(message.id, message.params),
            "textDocument/declaration" => self.declaration(message.id, message.params),
            "textDocument/typeDefinition" => self.type_definition(message.id, message.params),
            "textDocument/references" => self.references(message.id, message.params),
            "textDocument/prepareRename" => self.prepare_rename(message.id, message.params),
            "textDocument/rename" => self.rename(message.id, message.params),
            "textDocument/prepareCallHierarchy" => {
                self.prepare_call_hierarchy(message.id, message.params)
            }
            "callHierarchy/incomingCalls" => self.incoming_calls(message.id, message.params),
            "callHierarchy/outgoingCalls" => self.outgoing_calls(message.id, message.params),
            "textDocument/documentHighlight" => self.document_highlight(message.id, message.params),
            "textDocument/documentSymbol" => self.document_symbol(message.id, message.params),
            "textDocument/foldingRange" => self.folding_range(message.id, message.params),
            "textDocument/formatting" => self.formatting(message.id, message.params),
            "textDocument/rangeFormatting" => self.range_formatting(message.id, message.params),
            "textDocument/onTypeFormatting" => self.on_type_formatting(message.id, message.params),
            "textDocument/selectionRange" => self.selection_range(message.id, message.params),
            "textDocument/semanticTokens/full" => {
                self.semantic_tokens_full(message.id, message.params)
            }
            "textDocument/semanticTokens/full/delta" => {
                self.semantic_tokens_full_delta(message.id, message.params)
            }
            "textDocument/semanticTokens/range" => {
                self.semantic_tokens_range(message.id, message.params)
            }
            "textDocument/inlayHint" => self.inlay_hint(message.id, message.params),
            "workspace/symbol" => self.workspace_symbol(message.id, message.params),
            "workspace/didChangeWatchedFiles" => {
                self.did_change_watched_files(message.id, message.params)
            }
            "workspace/didChangeConfiguration" => {
                self.did_change_configuration(message.id, message.params)
            }
            "workspace/didChangeWorkspaceFolders" => {
                self.did_change_workspace_folders(message.id, message.params)
            }
            method => self.method_not_found(message.id, method),
        }
    }

    fn did_open(&mut self, id: Option<RequestId>, params: JsonValue) -> JsonRpcResult {
        if let Some(id) = id {
            return JsonRpcResult::Response(error_response(
                Some(id),
                ErrorCode::InvalidRequest,
                "`textDocument/didOpen` must be sent as a notification",
            ));
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
            return JsonRpcResult::Response(error_response(
                Some(id),
                ErrorCode::InvalidRequest,
                "`textDocument/didChange` must be sent as a notification",
            ));
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
            return JsonRpcResult::Response(error_response(
                Some(id),
                ErrorCode::InvalidRequest,
                "`textDocument/didClose` must be sent as a notification",
            ));
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
            return JsonRpcResult::Response(error_response(
                Some(id),
                ErrorCode::InvalidRequest,
                "`workspace/didChangeWatchedFiles` must be sent as a notification",
            ));
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
            return JsonRpcResult::Response(error_response(
                Some(id),
                ErrorCode::InvalidRequest,
                "`workspace/didChangeWorkspaceFolders` must be sent as a notification",
            ));
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
                let mut diagnostics = lsp_diagnostics(&diagnostics);
                diagnostics.extend(lsp_project_diagnostics(&project_diagnostics, document_id));
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
        let mut diagnostics = lsp_diagnostics(&diagnostics);
        diagnostics.extend(lsp_project_diagnostics(
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
                    lsp_project_diagnostics(&self.config_diagnostics, document_id),
                    None,
                )
            })
            .collect()
    }

    fn schema_diagnostic_notifications(&self) -> Vec<String> {
        let diagnostics = lsp_schema_diagnostics(self.databases.schema_db().diagnostics());
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

fn lsp_diagnostics(diagnostics: &DocumentDiagnostics) -> Vec<JsonValue> {
    diagnostics
        .diagnostics()
        .iter()
        .map(lsp_diagnostic)
        .collect()
}

fn lsp_diagnostic(diagnostic: &ServiceDiagnostic) -> JsonValue {
    json!({
        "range": diagnostic.range().map_or_else(zero_range, lsp_range),
        "severity": lsp_severity(diagnostic.severity()),
        "code": diagnostic.code(),
        "source": "vela",
        "message": diagnostic.message(),
        "data": {
            "labels": diagnostic.labels().iter().map(|label| {
                json!({
                    "uri": label.document_id().as_str(),
                    "range": lsp_range(label.range()),
                    "message": label.message()
                })
            }).collect::<Vec<_>>(),
            "candidates": diagnostic.candidates().iter().map(|candidate| {
                json!({ "replacement": candidate.replacement() })
            }).collect::<Vec<_>>(),
            "repairHints": diagnostic.repair_hints().iter().map(|hint| {
                json!({
                    "uri": hint.document_id().as_str(),
                    "range": lsp_range(hint.range()),
                    "title": hint.title(),
                    "replacement": hint.replacement()
                })
            }).collect::<Vec<_>>()
        }
    })
}

fn project_diagnostics(project: &ProjectSources) -> Vec<ProjectDiagnostic> {
    let mut diagnostics = project.diagnostics().to_vec();
    diagnostics.extend(missing_import_diagnostics(project));
    diagnostics
}

fn lsp_project_diagnostics(
    diagnostics: &[ProjectDiagnostic],
    document_id: &DocumentId,
) -> Vec<JsonValue> {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.document_id() == Some(document_id))
        .map(|diagnostic| {
            json!({
                "range": zero_range(),
                "severity": 1,
                "code": "project::diagnostic",
                "source": "vela",
                "message": diagnostic.message(),
                "data": {
                    "labels": [],
                    "candidates": [],
                    "repairHints": []
                }
            })
        })
        .collect()
}

fn lsp_schema_diagnostics(diagnostics: &[SchemaDiagnostic]) -> Vec<JsonValue> {
    diagnostics
        .iter()
        .map(|diagnostic| {
            json!({
                "range": zero_range(),
                "severity": 1,
                "code": "schema::diagnostic",
                "source": "vela",
                "message": diagnostic.message(),
                "data": {
                    "labels": [],
                    "candidates": [],
                    "repairHints": []
                }
            })
        })
        .collect()
}

fn lsp_range(range: vela_language_service::DiagnosticRange) -> JsonValue {
    json!({
        "start": {
            "line": range.start().line,
            "character": range.start().character
        },
        "end": {
            "line": range.end().line,
            "character": range.end().character
        }
    })
}

fn zero_range() -> JsonValue {
    json!({
        "start": { "line": 0, "character": 0 },
        "end": { "line": 0, "character": 0 }
    })
}

fn lsp_severity(severity: ServiceDiagnosticSeverity) -> u8 {
    match severity {
        ServiceDiagnosticSeverity::Error => 1,
        ServiceDiagnosticSeverity::Warning => 2,
        ServiceDiagnosticSeverity::Note => 3,
        ServiceDiagnosticSeverity::Help => 4,
    }
}

fn publish_diagnostics_notification(
    uri: &str,
    diagnostics: Vec<JsonValue>,
    error: Option<String>,
) -> String {
    let mut params = json!({
        "uri": uri,
        "diagnostics": diagnostics
    });
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
