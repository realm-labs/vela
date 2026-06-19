use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
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

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(untagged)]
pub(crate) enum RequestId {
    Number(i64),
    String(String),
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ErrorCode {
    ParseError,
    InvalidRequest,
    MethodNotFound,
    ServerNotInitialized,
    RequestCancelled,
}

impl ErrorCode {
    const fn value(self) -> i32 {
        match self {
            Self::ParseError => -32700,
            Self::InvalidRequest => -32600,
            Self::MethodNotFound => -32601,
            Self::ServerNotInitialized => -32002,
            Self::RequestCancelled => -32800,
        }
    }
}

pub(crate) fn success_response(id: RequestId, result: JsonValue) -> String {
    json!({
        "jsonrpc": JSONRPC_VERSION,
        "id": id,
        "result": result
    })
    .to_string()
}

pub(crate) fn error_response(
    id: Option<RequestId>,
    code: ErrorCode,
    message: impl Into<String>,
) -> String {
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
