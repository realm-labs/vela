use lsp_server::{Message, RequestId};
#[cfg(test)]
use lsp_server::{Response, ResponseError};
use lsp_types::NumberOrString;
#[cfg(test)]
use serde::Deserialize;
use serde_json::Value as JsonValue;

pub(crate) const JSONRPC_VERSION: &str = "2.0";

#[derive(Debug, Clone)]
#[cfg(test)]
pub enum JsonRpcResult {
    Response(Response),
    Notification(Message),
    Notifications(Vec<Message>),
    None,
}

#[cfg(test)]
impl PartialEq for JsonRpcResult {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Response(left), Self::Response(right)) => {
                serialize_message(&Message::Response(left.clone()))
                    == serialize_message(&Message::Response(right.clone()))
            }
            (Self::Notification(left), Self::Notification(right)) => {
                serialize_message(left) == serialize_message(right)
            }
            (Self::Notifications(left), Self::Notifications(right)) => {
                left.len() == right.len()
                    && left
                        .iter()
                        .zip(right)
                        .all(|(left, right)| serialize_message(left) == serialize_message(right))
            }
            (Self::None, Self::None) => true,
            _ => false,
        }
    }
}

#[cfg(test)]
impl Eq for JsonRpcResult {}

#[cfg(test)]
impl JsonRpcResult {
    #[must_use]
    pub fn into_response(self) -> Option<String> {
        match self {
            Self::Response(response) => Some(serialize_message(&Message::Response(response))),
            Self::Notification(_) | Self::Notifications(_) | Self::None => None,
        }
    }

    #[must_use]
    pub fn into_notification(self) -> Option<String> {
        match self {
            Self::Notification(notification) => Some(serialize_message(&notification)),
            Self::Notifications(mut notifications) if notifications.len() == 1 => notifications
                .pop()
                .map(|message| serialize_message(&message)),
            Self::Response(_) | Self::Notifications(_) | Self::None => None,
        }
    }

    #[must_use]
    pub fn into_notifications(self) -> Option<Vec<String>> {
        match self {
            Self::Notification(notification) => Some(vec![serialize_message(&notification)]),
            Self::Notifications(notifications) => {
                Some(notifications.iter().map(serialize_message).collect())
            }
            Self::Response(_) | Self::None => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn into_messages(self) -> anyhow::Result<Vec<Message>> {
        match self {
            Self::Response(response) => Ok(vec![Message::Response(response)]),
            Self::Notification(message) => Ok(vec![message]),
            Self::Notifications(messages) => Ok(messages),
            Self::None => Ok(Vec::new()),
        }
    }
}

#[cfg(test)]
pub(crate) fn typed_messages(result: JsonRpcResult) -> Vec<Message> {
    result
        .into_messages()
        .expect("JSON-RPC result should contain typed LSP messages")
}

#[cfg(test)]
pub(crate) fn result_from_messages(messages: Vec<Message>) -> JsonRpcResult {
    match messages.as_slice() {
        [] => JsonRpcResult::None,
        [Message::Response(response)] => JsonRpcResult::Response(response.clone()),
        [message @ Message::Notification(_)] | [message @ Message::Request(_)] => {
            JsonRpcResult::Notification(message.clone())
        }
        _ => JsonRpcResult::Notifications(messages),
    }
}

#[derive(Debug, Clone, Deserialize)]
#[cfg(test)]
pub(crate) struct CancelRequestParams {
    pub(crate) id: RequestId,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ErrorCode {
    #[cfg(test)]
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
    pub(crate) const fn value(self) -> i32 {
        match self {
            #[cfg(test)]
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

#[cfg(test)]
impl JsonRpcResult {
    pub(crate) fn ok(id: RequestId, result: JsonValue) -> Self {
        Self::Response(Response {
            id,
            result: Some(result),
            error: None,
        })
    }

    pub(crate) fn error(
        id: Option<RequestId>,
        code: ErrorCode,
        message: impl Into<String>,
    ) -> Self {
        if let Some(id) = id {
            let message = message.into();
            return Self::Response(Response {
                id,
                result: None,
                error: Some(ResponseError {
                    code: code.value(),
                    message,
                    data: None,
                }),
            });
        }

        Self::None
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
