use serde_json::Value as JsonValue;
use vela_language_service::DocumentId;

use crate::{
    ErrorCode, JsonRpcResult, LspServer, RequestId, completion::lsp_completion_list,
    error_response, hover::lsp_hover, protocol::TextDocumentPositionParams,
    protocol::service_position, signature::lsp_signature_help, success_response,
};

impl LspServer {
    pub(crate) fn completion(&mut self, id: Option<RequestId>, params: JsonValue) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<TextDocumentPositionParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid completion params: {error}"),
                ));
            }
        };

        let document_id = DocumentId::from(params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let completions = self
            .databases
            .completion_items(&document_id, service_position(params.position));

        JsonRpcResult::Response(success_response(id, lsp_completion_list(&completions)))
    }

    pub(crate) fn signature_help(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<TextDocumentPositionParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid signatureHelp params: {error}"),
                ));
            }
        };

        let document_id = DocumentId::from(params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let signatures = self
            .databases
            .signature_help(&document_id, service_position(params.position));

        JsonRpcResult::Response(success_response(
            id,
            signatures
                .as_ref()
                .map_or(JsonValue::Null, lsp_signature_help),
        ))
    }

    pub(crate) fn hover(&mut self, id: Option<RequestId>, params: JsonValue) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<TextDocumentPositionParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid hover params: {error}"),
                ));
            }
        };

        let document_id = DocumentId::from(params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let hover = self
            .databases
            .hover(&document_id, service_position(params.position));

        JsonRpcResult::Response(success_response(
            id,
            hover.as_ref().map_or(JsonValue::Null, lsp_hover),
        ))
    }
}
