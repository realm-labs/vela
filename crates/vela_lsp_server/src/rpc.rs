use std::collections::BTreeMap;

use lsp_server::{Message, Response, ResponseError};
use lsp_types::NumberOrString;
use serde::Deserialize;
use serde_json::{Value as JsonValue, json};

pub(crate) const JSONRPC_VERSION: &str = "2.0";

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
pub(crate) struct JsonRpcMessage {
    pub(crate) jsonrpc: String,
    pub(crate) id: Option<RequestId>,
    #[serde(default)]
    pub(crate) method: Option<String>,
    #[serde(default)]
    pub(crate) params: JsonValue,
    #[serde(flatten)]
    pub(crate) extra: BTreeMap<String, JsonValue>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CancelRequestParams {
    pub(crate) id: RequestId,
}

pub(crate) type RequestId = lsp_server::RequestId;

#[derive(Debug, Clone, Copy)]
pub(crate) enum ErrorCode {
    ParseError,
    InvalidRequest,
    MethodNotFound,
    InvalidParams,
    InternalError,
    ServerNotInitialized,
    RequestCancelled,
    ContentModified,
}

impl ErrorCode {
    const fn value(self) -> i32 {
        match self {
            Self::ParseError => -32700,
            Self::InvalidRequest => -32600,
            Self::MethodNotFound => -32601,
            Self::InvalidParams => -32602,
            Self::InternalError => -32603,
            Self::ServerNotInitialized => -32002,
            Self::RequestCancelled => -32800,
            Self::ContentModified => -32801,
        }
    }
}

impl JsonRpcResult {
    pub(crate) fn ok(id: RequestId, result: JsonValue) -> Self {
        Self::Response(serialize_response(Response {
            id,
            result: Some(result),
            error: None,
        }))
    }

    pub(crate) fn error(
        id: Option<RequestId>,
        code: ErrorCode,
        message: impl Into<String>,
    ) -> Self {
        let message = message.into();
        if let Some(id) = id {
            return Self::Response(serialize_response(Response {
                id,
                result: None,
                error: Some(ResponseError {
                    code: code.value(),
                    message,
                    data: None,
                }),
            }));
        }

        Self::Response(
            json!({
                "jsonrpc": JSONRPC_VERSION,
                "id": null,
                "error": {
                    "code": code.value(),
                    "message": message
                }
            })
            .to_string(),
        )
    }
}

fn serialize_response(response: Response) -> String {
    let mut value = serde_json::to_value(Message::Response(response))
        .expect("typed LSP response should serialize");
    let object = value
        .as_object_mut()
        .expect("typed LSP response should serialize to an object");
    object.insert(
        "jsonrpc".to_owned(),
        JsonValue::String(JSONRPC_VERSION.to_owned()),
    );
    value.to_string()
}

pub(crate) fn request_id_from_lsp(id: lsp_server::RequestId) -> RequestId {
    id
}

pub(crate) fn request_id_from_lsp_number_or_string(id: NumberOrString) -> RequestId {
    match id {
        NumberOrString::Number(id) => RequestId::from(id),
        NumberOrString::String(id) => RequestId::from(id),
    }
}
