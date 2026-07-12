use lsp_server::{Message, Response, ResponseError};

use crate::ErrorCode;

pub(super) fn ok(id: lsp_server::RequestId, result: serde_json::Value) -> Vec<Message> {
    vec![Message::Response(Response {
        id,
        result: Some(result),
        error: None,
    })]
}

pub(super) fn ok_typed<T>(
    id: lsp_server::RequestId,
    result: T,
    context: &'static str,
) -> Vec<Message>
where
    T: serde::Serialize,
{
    let result = serde_json::to_value(result)
        .unwrap_or_else(|error| panic!("{context} should serialize: {error}"));
    ok(id, result)
}

pub(super) fn error(
    id: lsp_server::RequestId,
    code: ErrorCode,
    message: impl Into<String>,
) -> Vec<Message> {
    vec![Message::Response(Response {
        id,
        result: None,
        error: Some(ResponseError {
            code: code.value(),
            message: message.into(),
            data: None,
        }),
    })]
}
