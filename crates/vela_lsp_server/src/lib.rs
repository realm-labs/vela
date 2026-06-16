//! Native LSP protocol boundary for Vela editor tooling.

mod call_hierarchy;
mod capabilities;
mod code_action;
mod completion;
mod definition;
mod folding;
mod formatting;
mod hover;
mod inlay;
mod protocol;
mod queries;
mod references;
mod rename;
mod selection;
mod semantic_tokens;
mod signature;
pub mod stdio;
mod symbols;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use protocol::{LspPosition, LspRange};
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use vela_language_service::{
    DocumentDiagnostics, DocumentId, LanguageServiceDatabases, ProjectDiagnostic, ProjectSources,
    SchemaDiagnostic, ServiceDiagnostic, ServiceDiagnosticSeverity, SourceFileSnapshot,
    SourceVersion, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
    missing_import_diagnostics,
};

use crate::capabilities::initialize_result;

const JSONRPC_VERSION: &str = "2.0";
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
    cancelled_requests: BTreeSet<RequestId>,
    disk_sources: BTreeMap<DocumentId, SourceFileSnapshot>,
    open_documents: BTreeSet<DocumentId>,
    client_supports_work_done_progress: bool,
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

        match message.method.as_str() {
            "$/cancelRequest" => self.cancel_request(message.id, message.params),
            "initialize" => self.initialize(message.id, message.params),
            "initialized" => self.initialized(message.id),
            "shutdown" => self.shutdown(message.id),
            "exit" => self.exit(message.id),
            "textDocument/didOpen" => self.did_open(message.id, message.params),
            "textDocument/didChange" => self.did_change(message.id, message.params),
            "textDocument/completion" => self.completion(message.id, message.params),
            "textDocument/codeAction" => self.code_action(message.id, message.params),
            "textDocument/signatureHelp" => self.signature_help(message.id, message.params),
            "textDocument/hover" => self.hover(message.id, message.params),
            "textDocument/definition" => self.definition(message.id, message.params),
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
            "textDocument/inlayHint" => self.inlay_hint(message.id, message.params),
            "workspace/symbol" => self.workspace_symbol(message.id, message.params),
            "workspace/didChangeWatchedFiles" => {
                self.did_change_watched_files(message.id, message.params)
            }
            "workspace/didChangeWorkspaceFolders" => {
                self.did_change_workspace_folders(message.id, message.params)
            }
            method => self.method_not_found(message.id, method),
        }
    }

    fn initialize(&mut self, id: Option<RequestId>, params: JsonValue) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        self.initialized = true;
        let params = serde_json::from_value::<InitializeParams>(params).unwrap_or_default();
        self.workspace_roots = workspace_roots_from_initialize(&params);
        self.config = workspace_config_from_roots(&self.workspace_roots);
        self.client_supports_work_done_progress = params.capabilities.supports_work_done_progress();
        JsonRpcResult::Response(success_response(id, initialize_result()))
    }

    fn initialized(&mut self, id: Option<RequestId>) -> JsonRpcResult {
        self.initialized = true;
        id.map_or(JsonRpcResult::None, |id| {
            JsonRpcResult::Response(error_response(
                Some(id),
                ErrorCode::InvalidRequest,
                "`initialized` must be sent as a notification",
            ))
        })
    }

    fn shutdown(&mut self, id: Option<RequestId>) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        self.shutdown_requested = true;
        JsonRpcResult::Response(success_response(id, JsonValue::Null))
    }

    fn exit(&mut self, id: Option<RequestId>) -> JsonRpcResult {
        self.exited = true;
        id.map_or(JsonRpcResult::None, |id| {
            JsonRpcResult::Response(error_response(
                Some(id),
                ErrorCode::InvalidRequest,
                "`exit` must be sent as a notification",
            ))
        })
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

        for change in params.changes {
            if change.kind == FILE_CHANGE_DELETED {
                self.remove_watched_file(&change.uri);
            } else {
                self.upsert_watched_file(&change.uri);
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

    fn cancel_request(&mut self, id: Option<RequestId>, params: JsonValue) -> JsonRpcResult {
        if let Some(id) = id {
            return JsonRpcResult::Response(error_response(
                Some(id),
                ErrorCode::InvalidRequest,
                "`$/cancelRequest` must be sent as a notification",
            ));
        }

        let Ok(params) = serde_json::from_value::<CancelRequestParams>(params) else {
            return JsonRpcResult::None;
        };
        self.cancelled_requests.insert(params.id);
        JsonRpcResult::None
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

        for folder in params.event.removed {
            let root = WorkspaceRoot::from(folder.uri);
            self.workspace_roots.remove(root.path());
        }
        for folder in params.event.added {
            let root = WorkspaceRoot::from(folder.uri);
            self.workspace_roots.insert(root.path().to_owned());
        }
        if !self.has_config_file {
            self.config = workspace_config_from_roots(&self.workspace_roots);
            self.reload_schema_from_config();
        }

        let has_open_documents = !self.open_documents.is_empty();
        let result = self.publish_open_diagnostics();
        if has_open_documents && self.client_supports_work_done_progress {
            with_work_done_progress(result, "Vela workspace diagnostics")
        } else {
            result
        }
    }

    fn upsert_watched_file(&mut self, uri: &str) {
        if is_config_uri(uri) {
            let Some(text) = read_document_uri(uri) else {
                return;
            };
            let document_id = DocumentId::from(uri.to_owned());
            let result = WorkspaceConfig::from_vela_toml(uri, &text);
            if !result.diagnostics.is_empty() || self.config_documents.contains(&document_id) {
                self.config_documents.insert(document_id);
            }
            self.has_config_file = true;
            self.config = Some(result.config);
            self.config_diagnostics = result.diagnostics;
            self.reload_schema_from_config();
        } else if self.is_schema_uri(uri) {
            self.upsert_schema_artifact(uri);
        } else if is_source_uri(uri) {
            let Some(text) = read_document_uri(uri) else {
                return;
            };
            let document_id = DocumentId::from(uri.to_owned());
            self.disk_sources.insert(
                document_id.clone(),
                SourceFileSnapshot::new(document_id, text),
            );
        }
    }

    fn remove_watched_file(&mut self, uri: &str) {
        if is_config_uri(uri) {
            self.has_config_file = false;
            self.config = workspace_config_from_roots(&self.workspace_roots);
            self.config_diagnostics.clear();
            self.config_documents
                .insert(DocumentId::from(uri.to_owned()));
            self.reload_schema_from_config();
        } else if self.is_schema_uri(uri) {
            self.mark_schema_artifact_missing();
        } else if is_source_uri(uri) {
            self.disk_sources.remove(&DocumentId::from(uri.to_owned()));
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

    fn schema_path(&self) -> Option<&str> {
        self.config
            .as_ref()
            .and_then(|config| config.schema().path())
    }

    fn publish_open_diagnostics(&mut self) -> JsonRpcResult {
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

    fn publish_current_diagnostics(
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

    fn method_not_found(&self, id: Option<RequestId>, method: &str) -> JsonRpcResult {
        id.map_or(JsonRpcResult::None, |id| {
            JsonRpcResult::Response(error_response(
                Some(id),
                ErrorCode::MethodNotFound,
                format!("method `{method}` is not implemented"),
            ))
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonRpcResult {
    Response(String),
    Notification(String),
    Notifications(Vec<String>),
    None,
}

impl JsonRpcResult {
    #[must_use]
    pub fn into_response(self) -> Option<String> {
        match self {
            Self::Response(response) => Some(response),
            Self::Notification(_) | Self::Notifications(_) | Self::None => None,
        }
    }

    #[must_use]
    pub fn into_notification(self) -> Option<String> {
        match self {
            Self::Notification(notification) => Some(notification),
            Self::Notifications(mut notifications) if notifications.len() == 1 => {
                notifications.pop()
            }
            Self::Response(_) | Self::Notifications(_) | Self::None => None,
        }
    }

    #[must_use]
    pub fn into_notifications(self) -> Option<Vec<String>> {
        match self {
            Self::Notification(notification) => Some(vec![notification]),
            Self::Notifications(notifications) => Some(notifications),
            Self::Response(_) | Self::None => None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct JsonRpcMessage {
    jsonrpc: String,
    id: Option<RequestId>,
    method: String,
    #[serde(default)]
    params: JsonValue,
}

#[derive(Debug, Clone, Deserialize)]
struct CancelRequestParams {
    id: RequestId,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InitializeParams {
    root_uri: Option<String>,
    workspace_folders: Option<Vec<WorkspaceFolder>>,
    #[serde(default)]
    capabilities: ClientCapabilities,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClientCapabilities {
    window: Option<WindowClientCapabilities>,
}

impl ClientCapabilities {
    fn supports_work_done_progress(&self) -> bool {
        self.window
            .as_ref()
            .is_some_and(|window| window.work_done_progress)
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WindowClientCapabilities {
    work_done_progress: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceFolder {
    uri: String,
    #[allow(dead_code)]
    name: Option<String>,
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

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(untagged)]
enum RequestId {
    Number(i64),
    String(String),
}

#[derive(Debug, Clone, Copy)]
enum ErrorCode {
    ParseError,
    InvalidRequest,
    MethodNotFound,
    RequestCancelled,
}

impl ErrorCode {
    const fn value(self) -> i32 {
        match self {
            Self::ParseError => -32700,
            Self::InvalidRequest => -32600,
            Self::MethodNotFound => -32601,
            Self::RequestCancelled => -32800,
        }
    }
}

fn workspace_roots_from_initialize(params: &InitializeParams) -> BTreeSet<String> {
    params
        .workspace_folders
        .iter()
        .flatten()
        .map(|folder| WorkspaceRoot::from(folder.uri.clone()))
        .chain(params.root_uri.iter().cloned().map(WorkspaceRoot::from))
        .map(|root| root.path().to_owned())
        .collect()
}

fn workspace_config_from_roots(roots: &BTreeSet<String>) -> Option<WorkspaceConfig> {
    (!roots.is_empty())
        .then(|| WorkspaceConfig::workspace(roots.iter().cloned().map(WorkspaceRoot::from)))
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

fn document_uri_path(uri: &str) -> PathBuf {
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

fn normalized_path(path: impl AsRef<Path>) -> String {
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

fn apply_range_edit(text: &mut String, range: LspRange, replacement: &str) -> Result<(), String> {
    let start = lsp_position_offset(text, range.start)?;
    let end = lsp_position_offset(text, range.end)?;
    if start > end {
        return Err("didChange range start must not be after the end".to_owned());
    }
    text.replace_range(start..end, replacement);
    Ok(())
}

fn lsp_position_offset(text: &str, position: LspPosition) -> Result<usize, String> {
    let line = usize::try_from(position.line)
        .map_err(|_| "didChange range line is too large".to_owned())?;
    let character = usize::try_from(position.character)
        .map_err(|_| "didChange range character is too large".to_owned())?;
    let (line_start, line_end) = line_bounds(text, line)?;
    utf16_character_offset(&text[line_start..line_end], character).map(|offset| line_start + offset)
}

fn line_bounds(text: &str, target_line: usize) -> Result<(usize, usize), String> {
    let mut line = 0usize;
    let mut line_start = 0usize;
    for (offset, byte) in text.bytes().enumerate() {
        if byte != b'\n' {
            continue;
        }
        if line == target_line {
            return Ok((line_start, trim_carriage_return(text, line_start, offset)));
        }
        line = line.saturating_add(1);
        line_start = offset + 1;
    }
    if line == target_line {
        Ok((line_start, text.len()))
    } else {
        Err("didChange range line is outside the document".to_owned())
    }
}

fn trim_carriage_return(text: &str, line_start: usize, line_end: usize) -> usize {
    if line_end > line_start && text.as_bytes()[line_end - 1] == b'\r' {
        line_end - 1
    } else {
        line_end
    }
}

fn utf16_character_offset(line_text: &str, character: usize) -> Result<usize, String> {
    let mut utf16_units = 0usize;
    for (offset, ch) in line_text.char_indices() {
        if utf16_units == character {
            return Ok(offset);
        }
        let next_units = utf16_units + ch.len_utf16();
        if character < next_units {
            return Err("didChange range splits a UTF-16 character".to_owned());
        }
        utf16_units = next_units;
    }
    if utf16_units == character {
        Ok(line_text.len())
    } else {
        Err("didChange range character is outside the line".to_owned())
    }
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

fn with_work_done_progress(result: JsonRpcResult, title: &str) -> JsonRpcResult {
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

fn success_response(id: RequestId, result: JsonValue) -> String {
    json!({
        "jsonrpc": JSONRPC_VERSION,
        "id": id,
        "result": result
    })
    .to_string()
}

fn error_response(id: Option<RequestId>, code: ErrorCode, message: impl Into<String>) -> String {
    json!({
        "jsonrpc": JSONRPC_VERSION,
        "id": id,
        "error": {
            "code": code.value(),
            "message": message.into()
        }
    })
    .to_string()
}

#[cfg(test)]
mod tests;
