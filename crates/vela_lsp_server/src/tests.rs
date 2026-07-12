use lsp_server::{Message, Notification, Request, RequestId};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value as JsonValue;

use self::support::TestServer;

fn request<R>(server: &mut TestServer, id: i32, params: JsonValue) -> Vec<Message>
where
    R: lsp_types::request::Request,
    R::Params: DeserializeOwned + Serialize,
{
    let params = serde_json::from_value(params)
        .unwrap_or_else(|error| panic!("{} fixture params should be typed: {error}", R::METHOD));
    server.request_messages::<R>(id, params)
}

fn notify<N>(server: &mut TestServer, params: JsonValue) -> Vec<Message>
where
    N: lsp_types::notification::Notification,
    N::Params: DeserializeOwned + Serialize,
{
    let params = serde_json::from_value(params)
        .unwrap_or_else(|error| panic!("{} fixture params should be typed: {error}", N::METHOD));
    server.notify::<N>(params)
}

fn protocol_request(
    server: &mut TestServer,
    id: i32,
    method: &str,
    params: JsonValue,
) -> Vec<Message> {
    server.send_protocol_message(Message::Request(Request {
        id: RequestId::from(id),
        method: method.to_owned(),
        params,
    }))
}

fn protocol_notification(server: &mut TestServer, method: &str, params: JsonValue) -> Vec<Message> {
    server.send_protocol_message(Message::Notification(Notification {
        method: method.to_owned(),
        params,
    }))
}

fn navigation_request(
    server: &mut TestServer,
    id: i32,
    method: &str,
    params: JsonValue,
) -> Vec<Message> {
    match method {
        "textDocument/definition" => {
            request::<lsp_types::request::GotoDefinition>(server, id, params)
        }
        "textDocument/declaration" => {
            request::<lsp_types::request::GotoDeclaration>(server, id, params)
        }
        "textDocument/typeDefinition" => {
            request::<lsp_types::request::GotoTypeDefinition>(server, id, params)
        }
        method => panic!("unsupported navigation fixture method: {method}"),
    }
}

fn response_value(messages: Vec<Message>) -> JsonValue {
    let [Message::Response(response)] = messages.as_slice() else {
        panic!("request should return a JSON-RPC response");
    };
    message_value(&Message::Response(response.clone()))
}

fn notification_value(mut messages: Vec<Message>) -> JsonValue {
    if messages.len() != 1 {
        panic!("notification should return a JSON-RPC notification");
    }
    let Some(notification) = messages.pop() else {
        panic!("notification should return a JSON-RPC notification");
    };
    if !matches!(notification, Message::Notification(_)) {
        panic!("notification should return a JSON-RPC notification");
    }
    message_value(&notification)
}

fn notification_values(messages: Vec<Message>) -> Vec<JsonValue> {
    if messages
        .iter()
        .any(|message| !matches!(message, Message::Notification(_)))
    {
        panic!("result should contain JSON-RPC notifications");
    }
    messages.iter().map(message_value).collect()
}

fn assert_no_messages(messages: Vec<Message>) {
    assert!(
        messages.is_empty(),
        "expected no LSP messages: {messages:?}"
    );
}

fn message_value(message: &Message) -> JsonValue {
    json_value(&crate::rpc::serialize_message(message))
}

fn json_value(source: &str) -> JsonValue {
    match serde_json::from_str(source) {
        Ok(value) => value,
        Err(error) => panic!("message should be valid JSON: {error}"),
    }
}

fn publish_diagnostics_notifications(notifications: &[JsonValue]) -> Vec<&JsonValue> {
    notifications
        .iter()
        .filter(|notification| notification["method"] == "textDocument/publishDiagnostics")
        .collect()
}

fn assert_workspace_progress(notifications: &[JsonValue]) {
    let Some(begin) = notifications.first() else {
        panic!("workspace progress should include a begin notification");
    };
    let Some(end) = notifications.last() else {
        panic!("workspace progress should include an end notification");
    };

    assert_eq!(begin["method"], "$/progress");
    assert_eq!(begin["params"]["token"], "vela/workspace-diagnostics");
    assert_eq!(begin["params"]["value"]["kind"], "begin");
    assert_eq!(
        begin["params"]["value"]["title"],
        "Vela workspace diagnostics"
    );

    assert_eq!(end["method"], "$/progress");
    assert_eq!(end["params"]["token"], "vela/workspace-diagnostics");
    assert_eq!(end["params"]["value"]["kind"], "end");
}

mod code_action;
mod completion_map;
mod completion_member;
mod completion_resolve;
mod completion_struct;
mod completion_type;
mod document_sync;
mod file_watching;
mod formatting;
mod incremental;
mod inlay;
mod inlay_suppression;
mod lifecycle;
mod rename_source_return;
mod support;

mod call_hierarchy;
mod close_overlay;
mod completion;
mod definition;
mod file_watching_coalescing;
mod folding;
mod hover;
mod references;
mod rename;
mod rename_collisions;
mod rename_schema;
mod schema_reload;
mod selection;
mod semantic_tokens;
mod semantic_tokens_degradation;
mod semantic_tokens_schema;
mod semantic_tokens_schema_trait;
mod semantic_tokens_source;
mod signature;
mod symbols;
mod workspace_folders;
