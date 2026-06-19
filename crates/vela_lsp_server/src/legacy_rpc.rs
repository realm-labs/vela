use lsp_server::{Message, Notification, Request, RequestId, Response, ResponseError};
use serde::Deserialize;

use crate::{
    JsonValue,
    rpc::{ErrorCode, serialize_message},
};

#[derive(Debug, Clone)]
pub enum JsonRpcResult {
    Response(Response),
    Notification(Message),
    Notifications(Vec<Message>),
    None,
}

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

impl Eq for JsonRpcResult {}

impl JsonRpcResult {
    #[must_use]
    pub fn into_response(self) -> Option<String> {
        match self {
            Self::Response(response) => Some(serialize_message(&Message::Response(response))),
            Self::Notification(_) | Self::Notifications(_) | Self::None => None,
        }
    }

    #[must_use]
    pub fn into_notification_message(self) -> Option<Message> {
        match self {
            Self::Notification(notification) => Some(notification),
            Self::Notifications(mut notifications) if notifications.len() == 1 => {
                notifications.pop()
            }
            Self::Response(_) | Self::Notifications(_) | Self::None => None,
        }
    }

    #[must_use]
    pub fn into_notification_messages(self) -> Option<Vec<Message>> {
        match self {
            Self::Notification(notification) => Some(vec![notification]),
            Self::Notifications(notifications) => Some(notifications),
            Self::Response(_) | Self::None => None,
        }
    }

    pub(crate) fn into_messages(self) -> anyhow::Result<Vec<Message>> {
        match self {
            Self::Response(response) => Ok(vec![Message::Response(response)]),
            Self::Notification(message) => Ok(vec![message]),
            Self::Notifications(messages) => Ok(messages),
            Self::None => Ok(Vec::new()),
        }
    }

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

pub(crate) fn typed_messages(result: JsonRpcResult) -> Vec<Message> {
    result
        .into_messages()
        .expect("JSON-RPC result should contain typed LSP messages")
}

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

pub(crate) fn message_from_json_rpc(value: JsonValue) -> anyhow::Result<Message> {
    if value.get("method").is_some() {
        let method = value
            .get("method")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| anyhow::anyhow!("JSON-RPC message is missing method"))?
            .to_owned();
        let params = value.get("params").cloned().unwrap_or(JsonValue::Null);
        if let Some(id) = value.get("id") {
            return Ok(Message::Request(Request {
                id: request_id_from_json(id)?,
                method,
                params,
            }));
        }
        return Ok(Message::Notification(Notification { method, params }));
    }

    let id = value
        .get("id")
        .ok_or_else(|| anyhow::anyhow!("JSON-RPC response is missing id"))
        .and_then(request_id_from_json)?;
    let result = value.get("result").cloned();
    let error = value
        .get("error")
        .map(response_error_from_json)
        .transpose()?;
    Ok(Message::Response(Response { id, result, error }))
}

pub(crate) fn request_id_from_json(value: &JsonValue) -> anyhow::Result<RequestId> {
    if let Some(id) = value.as_i64() {
        let id = i32::try_from(id)?;
        return Ok(RequestId::from(id));
    }
    if let Some(id) = value.as_str() {
        return Ok(RequestId::from(id.to_owned()));
    }
    anyhow::bail!("unsupported JSON-RPC response id `{value}`")
}

fn response_error_from_json(value: &JsonValue) -> anyhow::Result<ResponseError> {
    let Some(object) = value.as_object() else {
        anyhow::bail!("JSON-RPC response error must be an object");
    };
    let code = object
        .get("code")
        .and_then(JsonValue::as_i64)
        .ok_or_else(|| anyhow::anyhow!("JSON-RPC response error is missing code"))?;
    let code = i32::try_from(code)?;
    let message = object
        .get("message")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| anyhow::anyhow!("JSON-RPC response error is missing message"))?
        .to_owned();
    let data = object.get("data").cloned();
    Ok(ResponseError {
        code,
        message,
        data,
    })
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CancelRequestParams {
    pub(crate) id: RequestId,
}
