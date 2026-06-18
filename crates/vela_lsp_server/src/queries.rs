use serde_json::Value as JsonValue;
use vela_language_service::{DocumentId, LineIndex};

use crate::{
    ErrorCode, JsonRpcResult, LspServer, RequestId,
    call_hierarchy::{
        lsp_call_hierarchy_items, lsp_incoming_calls, lsp_outgoing_calls,
        service_call_hierarchy_item,
    },
    code_action::lsp_code_actions,
    completion::{
        lsp_completion_list, lsp_completion_resolved_item, service_completion_resolve_payload,
    },
    definition::lsp_definition,
    error_response,
    folding::lsp_folding_ranges,
    formatting::lsp_text_edits,
    hover::lsp_hover,
    inlay::lsp_inlay_hints,
    protocol::CallHierarchyIncomingCallsParams,
    protocol::CallHierarchyOutgoingCallsParams,
    protocol::CallHierarchyPrepareParams,
    protocol::CodeActionParams,
    protocol::DocumentFormattingParams,
    protocol::DocumentOnTypeFormattingParams,
    protocol::DocumentRangeFormattingParams,
    protocol::DocumentSymbolParams,
    protocol::FoldingRangeParams,
    protocol::InlayHintParams,
    protocol::PrepareRenameParams,
    protocol::ReferencesParams,
    protocol::RenameParams,
    protocol::SelectionRangeParams,
    protocol::SemanticTokensDeltaParams,
    protocol::SemanticTokensParams,
    protocol::TextDocumentPositionParams,
    protocol::WorkspaceSymbolParams,
    protocol::service_position,
    protocol::service_range,
    references::{lsp_document_highlights, lsp_references},
    rename::{lsp_prepare_rename, lsp_workspace_edit},
    selection::lsp_selection_ranges,
    semantic_tokens::{lsp_semantic_token_delta, lsp_semantic_tokens},
    signature::lsp_signature_help,
    success_response,
    symbols::{lsp_document_symbols, lsp_workspace_symbols},
};

enum NavigationLocationQuery {
    Definition,
    Declaration,
    TypeDefinition,
}

impl LspServer {
    pub(crate) fn code_action(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<CodeActionParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid codeAction params: {error}"),
                ));
            }
        };

        let document_id = DocumentId::from(params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let actions = self
            .databases
            .code_actions(&document_id, crate::protocol::service_range(params.range));

        JsonRpcResult::Response(success_response(id, lsp_code_actions(&actions)))
    }

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
        let line_index = self
            .databases
            .source_db()
            .records()
            .get(&document_id)
            .map_or_else(
                || LineIndex::new(""),
                |source| LineIndex::new(source.text()),
            );

        JsonRpcResult::Response(success_response(
            id,
            lsp_completion_list(&completions, &line_index),
        ))
    }

    pub(crate) fn completion_resolve(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let documentation = service_completion_resolve_payload(&params)
            .and_then(|payload| self.databases.completion_documentation(&payload));
        JsonRpcResult::Response(success_response(
            id,
            lsp_completion_resolved_item(params, documentation),
        ))
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
        self.navigation_location(
            id,
            params,
            "definition",
            NavigationLocationQuery::Definition,
        )
    }

    pub(crate) fn declaration(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        self.navigation_location(
            id,
            params,
            "declaration",
            NavigationLocationQuery::Declaration,
        )
    }

    pub(crate) fn type_definition(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        self.navigation_location(
            id,
            params,
            "typeDefinition",
            NavigationLocationQuery::TypeDefinition,
        )
    }

    fn navigation_location(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
        method_name: &'static str,
        query: NavigationLocationQuery,
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
                    format!("invalid {method_name} params: {error}"),
                ));
            }
        };

        let document_id = DocumentId::from(params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let position = service_position(params.position);
        let definition = match query {
            NavigationLocationQuery::Definition => {
                self.databases.definition(&document_id, position)
            }
            NavigationLocationQuery::Declaration => {
                self.databases.declaration(&document_id, position)
            }
            NavigationLocationQuery::TypeDefinition => {
                self.databases.type_definition(&document_id, position)
            }
        };

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

    pub(crate) fn prepare_rename(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<PrepareRenameParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid prepareRename params: {error}"),
                ));
            }
        };

        let document_id = DocumentId::from(params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let prepare = self
            .databases
            .prepare_rename(&document_id, service_position(params.position));

        JsonRpcResult::Response(success_response(
            id,
            prepare.as_ref().map_or(JsonValue::Null, lsp_prepare_rename),
        ))
    }

    pub(crate) fn rename(&mut self, id: Option<RequestId>, params: JsonValue) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<RenameParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid rename params: {error}"),
                ));
            }
        };

        let document_id = DocumentId::from(params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let edit = self.databases.rename(
            &document_id,
            service_position(params.position),
            &params.new_name,
        );

        JsonRpcResult::Response(success_response(
            id,
            edit.as_ref().map_or(JsonValue::Null, lsp_workspace_edit),
        ))
    }

    pub(crate) fn prepare_call_hierarchy(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<CallHierarchyPrepareParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid prepareCallHierarchy params: {error}"),
                ));
            }
        };

        let document_id = DocumentId::from(params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let items = self
            .databases
            .prepare_call_hierarchy(&document_id, service_position(params.position));

        JsonRpcResult::Response(success_response(id, lsp_call_hierarchy_items(&items)))
    }

    pub(crate) fn incoming_calls(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<CallHierarchyIncomingCallsParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid incomingCalls params: {error}"),
                ));
            }
        };

        let document_id = DocumentId::from(params.item.uri.clone());
        self.refresh_databases_for_query(&document_id);
        let item = service_call_hierarchy_item(&params.item);
        let calls = self.databases.incoming_calls(&item);

        JsonRpcResult::Response(success_response(id, lsp_incoming_calls(&calls)))
    }

    pub(crate) fn outgoing_calls(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<CallHierarchyOutgoingCallsParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid outgoingCalls params: {error}"),
                ));
            }
        };

        let document_id = DocumentId::from(params.item.uri.clone());
        self.refresh_databases_for_query(&document_id);
        let item = service_call_hierarchy_item(&params.item);
        let calls = self.databases.outgoing_calls(&item);

        JsonRpcResult::Response(success_response(id, lsp_outgoing_calls(&calls)))
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

    pub(crate) fn formatting(&mut self, id: Option<RequestId>, params: JsonValue) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<DocumentFormattingParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid formatting params: {error}"),
                ));
            }
        };

        let document_id = DocumentId::from(params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let edits = self.databases.document_formatting(&document_id);

        JsonRpcResult::Response(success_response(id, lsp_text_edits(&edits)))
    }

    pub(crate) fn range_formatting(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<DocumentRangeFormattingParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid rangeFormatting params: {error}"),
                ));
            }
        };

        let document_id = DocumentId::from(params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let edits = self
            .databases
            .range_formatting(&document_id, service_range(params.range));

        JsonRpcResult::Response(success_response(id, lsp_text_edits(&edits)))
    }

    pub(crate) fn on_type_formatting(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<DocumentOnTypeFormattingParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid onTypeFormatting params: {error}"),
                ));
            }
        };

        let document_id = DocumentId::from(params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let edits = self.databases.on_type_formatting(
            &document_id,
            service_position(params.position),
            &params.ch,
        );

        JsonRpcResult::Response(success_response(id, lsp_text_edits(&edits)))
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

    pub(crate) fn semantic_tokens_full_delta(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<SemanticTokensDeltaParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid semanticTokens/full/delta params: {error}"),
                ));
            }
        };

        let document_id = DocumentId::from(params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let delta = self
            .databases
            .semantic_token_delta(&document_id, &params.previous_result_id);

        JsonRpcResult::Response(success_response(id, lsp_semantic_token_delta(&delta)))
    }

    pub(crate) fn inlay_hint(&mut self, id: Option<RequestId>, params: JsonValue) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<InlayHintParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid inlayHint params: {error}"),
                ));
            }
        };

        let document_id = DocumentId::from(params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let hints = self
            .databases
            .inlay_hints(&document_id, service_range(params.range));

        JsonRpcResult::Response(success_response(id, lsp_inlay_hints(&hints)))
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
