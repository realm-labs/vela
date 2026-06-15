//! Native LSP protocol boundary for Vela editor tooling.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use vela_language_service::{
    DocumentDiagnostics, DocumentId, LanguageServiceDatabases, ServiceDiagnostic,
    ServiceDiagnosticSeverity, SourceFileSnapshot, SourceVersion, Workspace, WorkspaceConfig,
    WorkspaceRoot, assemble_project_sources,
};

const JSONRPC_VERSION: &str = "2.0";
const FILE_CHANGE_DELETED: u8 = 3;
const CONFIG_FILE: &str = "vela.toml";
const SOURCE_EXTENSION: &str = ".vela";

#[derive(Debug, Default)]
pub struct LspServer {
    workspace: Workspace,
    databases: LanguageServiceDatabases,
    config: Option<WorkspaceConfig>,
    disk_sources: BTreeMap<DocumentId, SourceFileSnapshot>,
    open_documents: BTreeSet<DocumentId>,
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

        match message.method.as_str() {
            "initialize" => self.initialize(message.id, message.params),
            "initialized" => self.initialized(message.id),
            "shutdown" => self.shutdown(message.id),
            "exit" => self.exit(message.id),
            "textDocument/didOpen" => self.did_open(message.id, message.params),
            "textDocument/didChange" => self.did_change(message.id, message.params),
            "workspace/didChangeWatchedFiles" => {
                self.did_change_watched_files(message.id, message.params)
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
        self.config = workspace_config_from_initialize(&params);
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

        let Some(change) = params.content_changes.into_iter().last() else {
            return JsonRpcResult::Notification(publish_diagnostics_notification(
                &params.text_document.uri,
                Vec::new(),
                Some("didChange requires a full replacement text change".to_owned()),
            ));
        };
        if change.range.is_some() {
            return JsonRpcResult::Notification(publish_diagnostics_notification(
                &params.text_document.uri,
                Vec::new(),
                Some("incremental didChange ranges are not implemented".to_owned()),
            ));
        }

        let uri = params.text_document.uri;
        let document_id = DocumentId::from(uri.clone());
        let version = source_version(params.text_document.version);
        self.workspace
            .change_document(document_id.clone(), change.text, version);
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

        JsonRpcResult::None
    }

    fn upsert_watched_file(&mut self, uri: &str) {
        let Some(text) = read_document_uri(uri) else {
            return;
        };
        if is_config_uri(uri) {
            let result = WorkspaceConfig::from_vela_toml(uri, &text);
            self.config = Some(result.config);
        } else if is_source_uri(uri) {
            let document_id = DocumentId::from(uri.to_owned());
            self.disk_sources.insert(
                document_id.clone(),
                SourceFileSnapshot::new(document_id, text),
            );
        }
    }

    fn remove_watched_file(&mut self, uri: &str) {
        if is_config_uri(uri) {
            self.config = None;
        } else if is_source_uri(uri) {
            self.disk_sources.remove(&DocumentId::from(uri.to_owned()));
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
        let project = assemble_project_sources(&config, &files, &self.workspace.snapshot());
        self.databases
            .update_with_open_documents(&project, &self.open_documents);
        let diagnostics = self.databases.diagnostics_for_document(document_id);

        JsonRpcResult::Notification(publish_diagnostics_notification(
            uri,
            lsp_diagnostics(&diagnostics),
            None,
        ))
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
    None,
}

impl JsonRpcResult {
    #[must_use]
    pub fn into_response(self) -> Option<String> {
        match self {
            Self::Response(response) => Some(response),
            Self::Notification(_) | Self::None => None,
        }
    }

    #[must_use]
    pub fn into_notification(self) -> Option<String> {
        match self {
            Self::Notification(notification) => Some(notification),
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

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InitializeParams {
    root_uri: Option<String>,
    workspace_folders: Option<Vec<WorkspaceFolder>>,
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
    range: Option<JsonValue>,
    text: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DidChangeWatchedFilesParams {
    changes: Vec<FileEvent>,
}

#[derive(Debug, Clone, Deserialize)]
struct FileEvent {
    uri: String,
    #[serde(rename = "type")]
    kind: u8,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
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
}

impl ErrorCode {
    const fn value(self) -> i32 {
        match self {
            Self::ParseError => -32700,
            Self::InvalidRequest => -32600,
            Self::MethodNotFound => -32601,
        }
    }
}

fn initialize_result() -> JsonValue {
    json!({
        "capabilities": {
            "textDocumentSync": {
                "openClose": true,
                "change": 1,
                "save": false
            }
        },
        "serverInfo": {
            "name": "vela_lsp_server",
            "version": env!("CARGO_PKG_VERSION")
        }
    })
}

fn workspace_config_from_initialize(params: &InitializeParams) -> Option<WorkspaceConfig> {
    let roots = params
        .workspace_folders
        .iter()
        .flatten()
        .map(|folder| WorkspaceRoot::from(folder.uri.clone()))
        .chain(params.root_uri.iter().cloned().map(WorkspaceRoot::from))
        .collect::<Vec<_>>();
    (!roots.is_empty()).then(|| WorkspaceConfig::workspace(roots))
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

fn source_version(version: i32) -> SourceVersion {
    u64::try_from(version)
        .ok()
        .map_or(SourceVersion::INITIAL, SourceVersion::new)
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
mod tests {
    use serde_json::Value as JsonValue;

    use crate::{JsonRpcResult, LspServer};

    fn request(id: i64, method: &str, params: JsonValue) -> String {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        })
        .to_string()
    }

    fn notification(method: &str, params: JsonValue) -> String {
        serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        })
        .to_string()
    }

    fn response_value(result: JsonRpcResult) -> JsonValue {
        let Some(response) = result.into_response() else {
            panic!("request should return a JSON-RPC response");
        };
        json_value(&response)
    }

    fn notification_value(result: JsonRpcResult) -> JsonValue {
        let Some(notification) = result.into_notification() else {
            panic!("notification should return a JSON-RPC notification");
        };
        json_value(&notification)
    }

    fn json_value(source: &str) -> JsonValue {
        match serde_json::from_str(source) {
            Ok(value) => value,
            Err(error) => panic!("message should be valid JSON: {error}"),
        }
    }

    mod lifecycle {
        use super::{JsonRpcResult, JsonValue, LspServer, notification, request, response_value};

        #[test]
        fn lsp_initialize_reports_capabilities() {
            let mut server = LspServer::new();
            let response = response_value(server.handle_json(&request(
                1,
                "initialize",
                serde_json::json!({
                    "processId": null,
                    "capabilities": {}
                }),
            )));

            assert!(server.is_initialized());
            assert_eq!(response["jsonrpc"], "2.0");
            assert_eq!(response["id"], 1);
            assert_eq!(response["result"]["serverInfo"]["name"], "vela_lsp_server");
            assert_eq!(
                response["result"]["capabilities"]["textDocumentSync"]["openClose"],
                true
            );
            assert_eq!(
                response["result"]["capabilities"]["textDocumentSync"]["change"],
                1
            );
            assert!(response["result"]["capabilities"]["completionProvider"].is_null());
            assert!(response["result"]["capabilities"]["hoverProvider"].is_null());
            assert!(response["result"]["capabilities"]["definitionProvider"].is_null());
        }

        #[test]
        fn lsp_initialized_notification_has_no_response() {
            let mut server = LspServer::new();
            let result = server.handle_json(&notification("initialized", serde_json::json!({})));

            assert!(server.is_initialized());
            assert_eq!(result, JsonRpcResult::None);
        }

        #[test]
        fn lsp_shutdown_exits_without_background_tasks() {
            let mut server = LspServer::new();
            let response =
                response_value(server.handle_json(&request(2, "shutdown", JsonValue::Null)));
            let exit = server.handle_json(&notification("exit", JsonValue::Null));

            assert_eq!(response["result"], JsonValue::Null);
            assert!(server.is_shutdown_requested());
            assert!(server.is_exited());
            assert_eq!(exit, JsonRpcResult::None);
        }
    }

    mod document_sync {
        use super::{LspServer, notification, notification_value, request, response_value};

        #[test]
        fn lsp_did_open_publishes_diagnostics() {
            let mut server = LspServer::new();
            let notification = notification_value(server.handle_json(&notification(
                "textDocument/didOpen",
                serde_json::json!({
                    "textDocument": {
                        "uri": "file:///workspace/main.vela",
                        "languageId": "vela",
                        "version": 1,
                        "text": "pub fn main(scores: Array<i64>) { return scores.frist() }"
                    }
                }),
            )));

            assert_eq!(notification["jsonrpc"], "2.0");
            assert_eq!(notification["method"], "textDocument/publishDiagnostics");
            assert_eq!(notification["params"]["uri"], "file:///workspace/main.vela");
            let Some(diagnostics) = notification["params"]["diagnostics"].as_array() else {
                panic!("publishDiagnostics should contain a diagnostic array");
            };
            assert_eq!(diagnostics.len(), 1);
            let diagnostic = &diagnostics[0];
            assert_eq!(diagnostic["severity"], 1);
            assert_eq!(diagnostic["source"], "vela");
            assert_eq!(diagnostic["code"], "analysis::unknown_method");
            assert!(
                diagnostic["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("unknown method `frist`"))
            );

            let Some(candidates) = diagnostic["data"]["candidates"].as_array() else {
                panic!("diagnostic should preserve candidate metadata");
            };
            assert!(
                candidates
                    .iter()
                    .any(|candidate| candidate["replacement"] == "first")
            );
            let Some(repair_hints) = diagnostic["data"]["repairHints"].as_array() else {
                panic!("diagnostic should preserve repair hints");
            };
            assert!(repair_hints.is_empty());
        }

        #[test]
        fn lsp_did_change_replaces_document_text() {
            let mut server = LspServer::new();
            let open = notification_value(server.handle_json(&notification(
                "textDocument/didOpen",
                serde_json::json!({
                    "textDocument": {
                        "uri": "file:///workspace/main.vela",
                        "languageId": "vela",
                        "version": 1,
                        "text": "pub fn main(scores: Array<i64>) { return scores.frist() }"
                    }
                }),
            )));
            let Some(open_diagnostics) = open["params"]["diagnostics"].as_array() else {
                panic!("didOpen should publish diagnostics");
            };
            assert_eq!(open_diagnostics.len(), 1);

            let change = notification_value(server.handle_json(&notification(
                "textDocument/didChange",
                serde_json::json!({
                    "textDocument": {
                        "uri": "file:///workspace/main.vela",
                        "version": 2
                    },
                    "contentChanges": [
                        {
                            "text": "pub fn main(scores: Array<i64>) { return scores.first() }"
                        }
                    ]
                }),
            )));

            assert_eq!(change["jsonrpc"], "2.0");
            assert_eq!(change["method"], "textDocument/publishDiagnostics");
            assert_eq!(change["params"]["uri"], "file:///workspace/main.vela");
            let Some(change_diagnostics) = change["params"]["diagnostics"].as_array() else {
                panic!("didChange should publish diagnostics");
            };
            assert!(change_diagnostics.is_empty());
        }

        #[test]
        fn lsp_initialize_uses_workspace_root_for_document_sync() {
            let mut server = LspServer::new();
            let response = response_value(server.handle_json(&request(
                1,
                "initialize",
                serde_json::json!({
                    "processId": null,
                    "rootUri": "file:///workspace/scripts",
                    "capabilities": {}
                }),
            )));
            assert_eq!(response["result"]["serverInfo"]["name"], "vela_lsp_server");

            let helper = notification_value(server.handle_json(&notification(
                "textDocument/didOpen",
                serde_json::json!({
                    "textDocument": {
                        "uri": "file:///workspace/scripts/game/helper.vela",
                        "languageId": "vela",
                        "version": 1,
                        "text": "pub fn grant() { return 1 }"
                    }
                }),
            )));
            let Some(helper_diagnostics) = helper["params"]["diagnostics"].as_array() else {
                panic!("helper didOpen should publish diagnostics");
            };
            assert!(helper_diagnostics.is_empty(), "{helper_diagnostics:?}");

            let main = notification_value(server.handle_json(&notification(
                "textDocument/didOpen",
                serde_json::json!({
                    "textDocument": {
                        "uri": "file:///workspace/scripts/game/main.vela",
                        "languageId": "vela",
                        "version": 1,
                        "text": "use game::helper::grant\npub fn main() { return grant() }"
                    }
                }),
            )));

            let Some(main_diagnostics) = main["params"]["diagnostics"].as_array() else {
                panic!("main didOpen should publish diagnostics");
            };
            assert!(
                main_diagnostics
                    .iter()
                    .all(|diagnostic| diagnostic["code"] != "hir::unresolved_module"
                        && diagnostic["code"] != "hir::unresolved_import"),
                "{main_diagnostics:?}"
            );
        }
    }

    mod file_watching {
        use std::fs;
        use std::path::{Path, PathBuf};
        use std::time::{SystemTime, UNIX_EPOCH};

        use super::{
            JsonRpcResult, LspServer, notification, notification_value, request, response_value,
        };

        fn temp_workspace() -> PathBuf {
            let suffix = match SystemTime::now().duration_since(UNIX_EPOCH) {
                Ok(duration) => duration.as_nanos(),
                Err(error) => panic!("system time should be after UNIX_EPOCH: {error}"),
            };
            let root = std::env::temp_dir().join(format!(
                "vela_lsp_server_{}_{}",
                std::process::id(),
                suffix
            ));
            if let Err(error) = fs::create_dir_all(root.join("scripts").join("game")) {
                panic!("temporary workspace should be creatable: {error}");
            }
            root
        }

        fn file_uri(path: &Path) -> String {
            let path = path.display().to_string().replace('\\', "/");
            if path.starts_with('/') {
                format!("file://{path}")
            } else {
                format!("file:///{path}")
            }
        }

        #[test]
        fn file_create_adds_module() {
            let root = temp_workspace();
            let config_path = root.join("vela.toml");
            let helper_path = root.join("scripts").join("game").join("helper.vela");
            if let Err(error) = fs::write(
                &config_path,
                r#"
                    [workspace]
                    roots = ["scripts"]
                "#,
            ) {
                panic!("vela.toml should be writable: {error}");
            }
            if let Err(error) = fs::write(&helper_path, "pub fn grant() { return 1 }") {
                panic!("helper source should be writable: {error}");
            }

            let mut server = LspServer::new();
            let response = response_value(server.handle_json(&request(
                1,
                "initialize",
                serde_json::json!({
                    "processId": null,
                    "rootUri": file_uri(&root),
                    "capabilities": {}
                }),
            )));
            assert_eq!(response["result"]["serverInfo"]["name"], "vela_lsp_server");

            let watched = server.handle_json(&notification(
                "workspace/didChangeWatchedFiles",
                serde_json::json!({
                    "changes": [
                        { "uri": file_uri(&config_path), "type": 1 },
                        { "uri": file_uri(&helper_path), "type": 1 }
                    ]
                }),
            ));
            assert_eq!(watched, JsonRpcResult::None);

            let main_uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
            let main = notification_value(server.handle_json(&notification(
                "textDocument/didOpen",
                serde_json::json!({
                    "textDocument": {
                        "uri": main_uri,
                        "languageId": "vela",
                        "version": 1,
                        "text": "use game::helper::grant\npub fn main() { return grant() }"
                    }
                }),
            )));

            let Some(diagnostics) = main["params"]["diagnostics"].as_array() else {
                panic!("didOpen should publish diagnostics");
            };
            assert!(
                diagnostics
                    .iter()
                    .all(|diagnostic| diagnostic["code"] != "hir::unresolved_module"
                        && diagnostic["code"] != "hir::unresolved_import"),
                "{diagnostics:?}"
            );

            if let Err(error) = fs::remove_dir_all(&root) {
                panic!("temporary workspace should be removable: {error}");
            }
        }
    }
}
