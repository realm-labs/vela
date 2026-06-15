//! Native LSP protocol boundary for Vela editor tooling.

use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};

const JSONRPC_VERSION: &str = "2.0";

#[derive(Debug, Clone, Default)]
pub struct LspServer {
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
            "initialize" => self.initialize(message.id),
            "initialized" => self.initialized(message.id),
            "shutdown" => self.shutdown(message.id),
            "exit" => self.exit(message.id),
            method => self.method_not_found(message.id, method),
        }
    }

    fn initialize(&mut self, id: Option<RequestId>) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        self.initialized = true;
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
    None,
}

impl JsonRpcResult {
    #[must_use]
    pub fn into_response(self) -> Option<String> {
        match self {
            Self::Response(response) => Some(response),
            Self::None => None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct JsonRpcMessage {
    jsonrpc: String,
    id: Option<RequestId>,
    method: String,
    #[serde(default)]
    _params: JsonValue,
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
    mod lifecycle {
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
            match serde_json::from_str(&response) {
                Ok(value) => value,
                Err(error) => panic!("response should be valid JSON: {error}"),
            }
        }

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
}
