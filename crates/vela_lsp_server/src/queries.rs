use serde_json::Value as JsonValue;
use vela_language_service::DocumentId;

use crate::{
    ErrorCode, JsonRpcResult, LspServer, RequestId,
    completion::lsp_completion_list,
    definition::lsp_definition,
    error_response,
    folding::lsp_folding_ranges,
    hover::lsp_hover,
    protocol::DocumentSymbolParams,
    protocol::FoldingRangeParams,
    protocol::ReferencesParams,
    protocol::SelectionRangeParams,
    protocol::SemanticTokensParams,
    protocol::TextDocumentPositionParams,
    protocol::WorkspaceSymbolParams,
    protocol::service_position,
    references::{lsp_document_highlights, lsp_references},
    selection::lsp_selection_ranges,
    semantic_tokens::lsp_semantic_tokens,
    signature::lsp_signature_help,
    success_response,
    symbols::{lsp_document_symbols, lsp_workspace_symbols},
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

    pub(crate) fn definition(&mut self, id: Option<RequestId>, params: JsonValue) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<TextDocumentPositionParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid definition params: {error}"),
                ));
            }
        };

        let document_id = DocumentId::from(params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let definition = self
            .databases
            .definition(&document_id, service_position(params.position));

        JsonRpcResult::Response(success_response(
            id,
            definition.as_ref().map_or(JsonValue::Null, lsp_definition),
        ))
    }

    pub(crate) fn references(&mut self, id: Option<RequestId>, params: JsonValue) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<ReferencesParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid references params: {error}"),
                ));
            }
        };

        let document_id = DocumentId::from(params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let references = self.databases.references(
            &document_id,
            service_position(params.position),
            params.context.include_declaration,
        );

        JsonRpcResult::Response(success_response(id, lsp_references(&references)))
    }

    pub(crate) fn document_highlight(
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
                    format!("invalid documentHighlight params: {error}"),
                ));
            }
        };

        let document_id = DocumentId::from(params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let highlights = self
            .databases
            .document_highlights(&document_id, service_position(params.position));

        JsonRpcResult::Response(success_response(id, lsp_document_highlights(&highlights)))
    }

    pub(crate) fn document_symbol(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<DocumentSymbolParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid documentSymbol params: {error}"),
                ));
            }
        };

        let document_id = DocumentId::from(params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let symbols = self.databases.document_symbols(&document_id);

        JsonRpcResult::Response(success_response(id, lsp_document_symbols(&symbols)))
    }

    pub(crate) fn folding_range(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<FoldingRangeParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid foldingRange params: {error}"),
                ));
            }
        };

        let document_id = DocumentId::from(params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let ranges = self.databases.folding_ranges(&document_id);

        JsonRpcResult::Response(success_response(id, lsp_folding_ranges(&ranges)))
    }

    pub(crate) fn selection_range(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<SelectionRangeParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid selectionRange params: {error}"),
                ));
            }
        };

        let document_id = DocumentId::from(params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let positions = params
            .positions
            .into_iter()
            .map(service_position)
            .collect::<Vec<_>>();
        let ranges = self.databases.selection_ranges(&document_id, &positions);

        JsonRpcResult::Response(success_response(id, lsp_selection_ranges(&ranges)))
    }

    pub(crate) fn semantic_tokens_full(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<SemanticTokensParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid semanticTokens/full params: {error}"),
                ));
            }
        };

        let document_id = DocumentId::from(params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let tokens = self.databases.semantic_tokens(&document_id);

        JsonRpcResult::Response(success_response(id, lsp_semantic_tokens(&tokens)))
    }

    pub(crate) fn workspace_symbol(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<WorkspaceSymbolParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid workspace/symbol params: {error}"),
                ));
            }
        };

        self.refresh_databases_for_workspace_query();
        let symbols = self.databases.workspace_symbols(&params.query);

        JsonRpcResult::Response(success_response(id, lsp_workspace_symbols(&symbols)))
    }
}
