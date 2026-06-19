use lsp_server::{Message, RequestId};
use lsp_types::NumberOrString;
use serde_json::Value as JsonValue;

pub(crate) const JSONRPC_VERSION: &str = "2.0";

#[derive(Debug, Clone, Copy)]
pub(crate) enum ErrorCode {
    InvalidRequest,
    MethodNotFound,
    InvalidParams,
    InternalError,
    ServerNotInitialized,
    RequestCancelled,
    ContentModified,
}

impl ErrorCode {
    pub(crate) const fn value(self) -> i32 {
        match self {
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

pub(crate) fn serialize_message(message: &Message) -> String {
    let mut value = serde_json::to_value(message).expect("typed LSP message should serialize");
    let object = value
        .as_object_mut()
        .expect("typed LSP message should serialize to an object");
    object.insert(
        "jsonrpc".to_owned(),
        JsonValue::String(JSONRPC_VERSION.to_owned()),
    );
    value.to_string()
}

pub(crate) fn request_id_from_number_or_string(id: NumberOrString) -> RequestId {
    match id {
        NumberOrString::Number(id) => RequestId::from(id),
        NumberOrString::String(id) => RequestId::from(id),
    }
}
