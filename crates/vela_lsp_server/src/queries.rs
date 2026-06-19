use serde_json::Value as JsonValue;
use vela_language_service::{DiagnosticRange, DocumentId, LineIndex as ServiceLineIndex, Position};

use crate::{
    ErrorCode, JsonRpcResult, LspServer, RequestId,
    call_hierarchy::{
        lsp_call_hierarchy_items, lsp_incoming_calls, lsp_outgoing_calls,
        service_call_hierarchy_item,
    },
    code_action::lsp_code_actions,
    completion::{lsp_completion_resolved_item, service_completion_resolve_payload},
    definition::lsp_definition,
    error_response,
    folding::lsp_folding_ranges,
    formatting::lsp_text_edits,
    hover::lsp_hover,
    inlay::lsp_inlay_hints,
    lsp::{from_proto, to_proto},
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
    protocol::SemanticTokensRangeParams,
    protocol::TextDocumentPositionParams,
    protocol::WorkspaceSymbolParams,
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

fn document_text(server: &LspServer, document_id: &DocumentId) -> String {
    server
        .databases
        .source_db()
        .records()
        .get(document_id)
        .map_or_else(String::new, |source| source.text().to_owned())
}

fn service_position_for_request(
    id: &RequestId,
    method_name: &str,
    document_text: &str,
    position: crate::protocol::LspPosition,
) -> Result<Position, JsonRpcResult> {
    crate::line_index::LineIndex::new(document_text)
        .service_position(position)
        .map_err(|error| {
            JsonRpcResult::Response(error_response(
                Some(id.clone()),
                ErrorCode::InvalidRequest,
                format!("invalid {method_name} position: {error}"),
            ))
        })
}

fn service_range_for_request(
    id: &RequestId,
    method_name: &str,
    document_text: &str,
    range: crate::protocol::LspRange,
) -> Result<DiagnosticRange, JsonRpcResult> {
    crate::line_index::LineIndex::new(document_text)
        .service_range(range)
        .map_err(|error| {
            JsonRpcResult::Response(error_response(
                Some(id.clone()),
                ErrorCode::InvalidRequest,
                format!("invalid {method_name} range: {error}"),
            ))
        })
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
        let text = document_text(self, &document_id);
        let range = match service_range_for_request(&id, "codeAction", &text, params.range) {
            Ok(range) => range,
            Err(response) => return response,
        };
        let actions = self.databases.code_actions(&document_id, range);

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
        let text = document_text(self, &document_id);
        let position = match service_position_for_request(&id, "completion", &text, params.position)
        {
            Ok(position) => position,
            Err(response) => return response,
        };
        let completions = self.databases.completion_items(&document_id, position);
        let line_index = ServiceLineIndex::new(&text);

        JsonRpcResult::Response(success_response(
            id,
            serde_json::to_value(to_proto::completion_response(&completions, &line_index))
                .expect("typed completion response should serialize"),
        ))
    }

    pub(crate) fn completion_typed(
        &mut self,
        id: RequestId,
        params: lsp_types::CompletionParams,
    ) -> JsonRpcResult {
        let document_id = from_proto::document_id(&params.text_document_position.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let text = document_text(self, &document_id);
        let input = match from_proto::completion_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid completion position: {error}"),
                ));
            }
        };
        let completions = self
            .databases
            .completion_items(&input.document_id, input.position);
        let line_index = ServiceLineIndex::new(&text);

        JsonRpcResult::Response(success_response(
            id,
            serde_json::to_value(to_proto::completion_response(&completions, &line_index))
                .expect("typed completion response should serialize"),
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
        let payload = match service_completion_resolve_payload(&params) {
            Ok(payload) => payload,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid completionItem/resolve payload: {error}"),
                ));
            }
        };
        let documentation =
            payload.and_then(|payload| self.databases.completion_documentation(&payload));
        JsonRpcResult::Response(success_response(
            id,
            lsp_completion_resolved_item(params, documentation),
        ))
    }

    pub(crate) fn completion_resolve_typed(
        &mut self,
        id: RequestId,
        params: lsp_types::CompletionItem,
    ) -> JsonRpcResult {
        let params_value =
            serde_json::to_value(&params).expect("typed completion item should serialize");
        let payload = match service_completion_resolve_payload(&params_value) {
            Ok(payload) => payload,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid completionItem/resolve payload: {error}"),
                ));
            }
        };
        let documentation =
            payload.and_then(|payload| self.databases.completion_documentation(&payload));
        JsonRpcResult::Response(success_response(
            id,
            serde_json::to_value(to_proto::completion_item_resolved(params, documentation))
                .expect("typed completion item should serialize"),
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
        let text = document_text(self, &document_id);
        let position =
            match service_position_for_request(&id, "signatureHelp", &text, params.position) {
                Ok(position) => position,
                Err(response) => return response,
            };
        let signatures = self.databases.signature_help(&document_id, position);

        JsonRpcResult::Response(success_response(
            id,
            signatures
                .as_ref()
                .map_or(JsonValue::Null, lsp_signature_help),
        ))
    }

    pub(crate) fn signature_help_typed(
        &mut self,
        id: RequestId,
        params: lsp_types::SignatureHelpParams,
    ) -> JsonRpcResult {
        let document_id =
            from_proto::document_id(&params.text_document_position_params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let text = document_text(self, &document_id);
        let input = match from_proto::signature_help_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid signatureHelp position: {error}"),
                ));
            }
        };
        let signatures = self
            .databases
            .signature_help(&input.document_id, input.position);

        JsonRpcResult::Response(success_response(
            id,
            serde_json::to_value(signatures.as_ref().map(to_proto::signature_help))
                .expect("typed signatureHelp response should serialize"),
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
        let text = document_text(self, &document_id);
        let position = match service_position_for_request(&id, "hover", &text, params.position) {
            Ok(position) => position,
            Err(response) => return response,
        };
        let hover = self.databases.hover(&document_id, position);

        JsonRpcResult::Response(success_response(
            id,
            hover.as_ref().map_or(JsonValue::Null, lsp_hover),
        ))
    }

    pub(crate) fn hover_typed(
        &mut self,
        id: RequestId,
        params: lsp_types::HoverParams,
    ) -> JsonRpcResult {
        let document_id =
            from_proto::document_id(&params.text_document_position_params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let text = document_text(self, &document_id);
        let input = match from_proto::hover_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid hover position: {error}"),
                ));
            }
        };
        let hover = self.databases.hover(&input.document_id, input.position);

        JsonRpcResult::Response(success_response(
            id,
            serde_json::to_value(hover.as_ref().map(to_proto::hover))
                .expect("typed hover response should serialize"),
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

    pub(crate) fn definition_typed(
        &mut self,
        id: RequestId,
        params: lsp_types::GotoDefinitionParams,
    ) -> JsonRpcResult {
        self.navigation_location_typed(
            id,
            params,
            "definition",
            NavigationLocationQuery::Definition,
        )
    }

    pub(crate) fn declaration_typed(
        &mut self,
        id: RequestId,
        params: lsp_types::request::GotoDeclarationParams,
    ) -> JsonRpcResult {
        self.navigation_location_typed(
            id,
            params,
            "declaration",
            NavigationLocationQuery::Declaration,
        )
    }

    pub(crate) fn type_definition_typed(
        &mut self,
        id: RequestId,
        params: lsp_types::request::GotoTypeDefinitionParams,
    ) -> JsonRpcResult {
        self.navigation_location_typed(
            id,
            params,
            "typeDefinition",
            NavigationLocationQuery::TypeDefinition,
        )
    }

    fn navigation_location_typed(
        &mut self,
        id: RequestId,
        params: lsp_types::GotoDefinitionParams,
        method_name: &'static str,
        query: NavigationLocationQuery,
    ) -> JsonRpcResult {
        let document_id =
            from_proto::document_id(&params.text_document_position_params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let text = document_text(self, &document_id);
        let input = match from_proto::goto_definition_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid {method_name} position: {error}"),
                ));
            }
        };
        let definition = match query {
            NavigationLocationQuery::Definition => self
                .databases
                .definition(&input.document_id, input.position),
            NavigationLocationQuery::Declaration => self
                .databases
                .declaration(&input.document_id, input.position),
            NavigationLocationQuery::TypeDefinition => self
                .databases
                .type_definition(&input.document_id, input.position),
        };

        JsonRpcResult::Response(success_response(
            id,
            serde_json::to_value(definition.as_ref().map(to_proto::definition_location))
                .expect("typed navigation response should serialize"),
        ))
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
        let text = document_text(self, &document_id);
        let position = match service_position_for_request(&id, method_name, &text, params.position)
        {
            Ok(position) => position,
            Err(response) => return response,
        };
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
        let text = document_text(self, &document_id);
        let position = match service_position_for_request(&id, "references", &text, params.position)
        {
            Ok(position) => position,
            Err(response) => return response,
        };
        let references =
            self.databases
                .references(&document_id, position, params.context.include_declaration);

        JsonRpcResult::Response(success_response(id, lsp_references(&references)))
    }

    pub(crate) fn references_typed(
        &mut self,
        id: RequestId,
        params: lsp_types::ReferenceParams,
    ) -> JsonRpcResult {
        let document_id = from_proto::document_id(&params.text_document_position.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let text = document_text(self, &document_id);
        let input = match from_proto::reference_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid references position: {error}"),
                ));
            }
        };
        let references = self.databases.references(
            &input.document_id,
            input.position,
            params.context.include_declaration,
        );

        JsonRpcResult::Response(success_response(
            id,
            serde_json::to_value(to_proto::reference_locations(&references))
                .expect("typed references response should serialize"),
        ))
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
        let text = document_text(self, &document_id);
        let position =
            match service_position_for_request(&id, "prepareRename", &text, params.position) {
                Ok(position) => position,
                Err(response) => return response,
            };
        let prepare = self.databases.prepare_rename(&document_id, position);

        JsonRpcResult::Response(success_response(
            id,
            prepare.as_ref().map_or(JsonValue::Null, lsp_prepare_rename),
        ))
    }

    pub(crate) fn prepare_rename_typed(
        &mut self,
        id: RequestId,
        params: lsp_types::TextDocumentPositionParams,
    ) -> JsonRpcResult {
        let document_id = from_proto::document_id(&params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let text = document_text(self, &document_id);
        let input = match from_proto::prepare_rename_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid prepareRename position: {error}"),
                ));
            }
        };
        let prepare = self
            .databases
            .prepare_rename(&input.document_id, input.position);

        JsonRpcResult::Response(success_response(
            id,
            serde_json::to_value(prepare.as_ref().map(to_proto::prepare_rename))
                .expect("typed prepareRename response should serialize"),
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
        let text = document_text(self, &document_id);
        let position = match service_position_for_request(&id, "rename", &text, params.position) {
            Ok(position) => position,
            Err(response) => return response,
        };
        let edit = self
            .databases
            .rename(&document_id, position, &params.new_name);

        JsonRpcResult::Response(success_response(
            id,
            edit.as_ref().map_or(JsonValue::Null, lsp_workspace_edit),
        ))
    }

    pub(crate) fn rename_typed(
        &mut self,
        id: RequestId,
        params: lsp_types::RenameParams,
    ) -> JsonRpcResult {
        let document_id = from_proto::document_id(&params.text_document_position.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let text = document_text(self, &document_id);
        let input = match from_proto::rename_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid rename position: {error}"),
                ));
            }
        };
        let edit = self
            .databases
            .rename(&input.document_id, input.position, &params.new_name);

        JsonRpcResult::Response(success_response(
            id,
            serde_json::to_value(edit.as_ref().map(to_proto::workspace_edit))
                .expect("typed rename response should serialize"),
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
        let text = document_text(self, &document_id);
        let position =
            match service_position_for_request(&id, "prepareCallHierarchy", &text, params.position)
            {
                Ok(position) => position,
                Err(response) => return response,
            };
        let items = self
            .databases
            .prepare_call_hierarchy(&document_id, position);

        JsonRpcResult::Response(success_response(id, lsp_call_hierarchy_items(&items)))
    }

    pub(crate) fn prepare_call_hierarchy_typed(
        &mut self,
        id: RequestId,
        params: lsp_types::CallHierarchyPrepareParams,
    ) -> JsonRpcResult {
        let document_id =
            from_proto::document_id(&params.text_document_position_params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let text = document_text(self, &document_id);
        let input = match from_proto::prepare_call_hierarchy_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid prepareCallHierarchy position: {error}"),
                ));
            }
        };
        let items = self
            .databases
            .prepare_call_hierarchy(&input.document_id, input.position);

        JsonRpcResult::Response(success_response(
            id,
            serde_json::to_value(to_proto::call_hierarchy_items(&items))
                .expect("typed prepareCallHierarchy response should serialize"),
        ))
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
        let text = document_text(self, &document_id);
        let item = match service_call_hierarchy_item(&params.item, &text) {
            Ok(item) => item,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid incomingCalls item range: {error}"),
                ));
            }
        };
        let calls = self.databases.incoming_calls(&item);

        JsonRpcResult::Response(success_response(id, lsp_incoming_calls(&calls)))
    }

    pub(crate) fn incoming_calls_typed(
        &mut self,
        id: RequestId,
        params: lsp_types::CallHierarchyIncomingCallsParams,
    ) -> JsonRpcResult {
        let document_id = from_proto::document_id(&params.item.uri);
        self.refresh_databases_for_query(&document_id);
        let text = document_text(self, &document_id);
        let item = match from_proto::call_hierarchy_item(&text, &params.item) {
            Ok(item) => item,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid incomingCalls item range: {error}"),
                ));
            }
        };
        let calls = self.databases.incoming_calls(&item);

        JsonRpcResult::Response(success_response(
            id,
            serde_json::to_value(to_proto::incoming_calls(&calls))
                .expect("typed incomingCalls response should serialize"),
        ))
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
        let text = document_text(self, &document_id);
        let item = match service_call_hierarchy_item(&params.item, &text) {
            Ok(item) => item,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid outgoingCalls item range: {error}"),
                ));
            }
        };
        let calls = self.databases.outgoing_calls(&item);

        JsonRpcResult::Response(success_response(id, lsp_outgoing_calls(&calls)))
    }

    pub(crate) fn outgoing_calls_typed(
        &mut self,
        id: RequestId,
        params: lsp_types::CallHierarchyOutgoingCallsParams,
    ) -> JsonRpcResult {
        let document_id = from_proto::document_id(&params.item.uri);
        self.refresh_databases_for_query(&document_id);
        let text = document_text(self, &document_id);
        let item = match from_proto::call_hierarchy_item(&text, &params.item) {
            Ok(item) => item,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid outgoingCalls item range: {error}"),
                ));
            }
        };
        let calls = self.databases.outgoing_calls(&item);

        JsonRpcResult::Response(success_response(
            id,
            serde_json::to_value(to_proto::outgoing_calls(&calls))
                .expect("typed outgoingCalls response should serialize"),
        ))
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
        let text = document_text(self, &document_id);
        let position =
            match service_position_for_request(&id, "documentHighlight", &text, params.position) {
                Ok(position) => position,
                Err(response) => return response,
            };
        let highlights = self.databases.document_highlights(&document_id, position);

        JsonRpcResult::Response(success_response(id, lsp_document_highlights(&highlights)))
    }

    pub(crate) fn document_highlight_typed(
        &mut self,
        id: RequestId,
        params: lsp_types::DocumentHighlightParams,
    ) -> JsonRpcResult {
        let document_id =
            from_proto::document_id(&params.text_document_position_params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let text = document_text(self, &document_id);
        let input = match from_proto::document_highlight_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid documentHighlight params: {error}"),
                ));
            }
        };
        let highlights = self
            .databases
            .document_highlights(&input.document_id, input.position);

        JsonRpcResult::Response(success_response(
            id,
            serde_json::to_value(to_proto::document_highlights(&highlights))
                .expect("typed documentHighlight response should serialize"),
        ))
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

    pub(crate) fn document_symbol_typed(
        &mut self,
        id: RequestId,
        params: lsp_types::DocumentSymbolParams,
    ) -> JsonRpcResult {
        let document_id = from_proto::document_symbol_params(&params);
        self.refresh_databases_for_query(&document_id);
        let symbols = self.databases.document_symbols(&document_id);

        JsonRpcResult::Response(success_response(
            id,
            serde_json::to_value(to_proto::document_symbols(&symbols))
                .expect("typed documentSymbol response should serialize"),
        ))
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
        let text = document_text(self, &document_id);
        let range = match service_range_for_request(&id, "rangeFormatting", &text, params.range) {
            Ok(range) => range,
            Err(response) => return response,
        };
        let edits = self.databases.range_formatting(&document_id, range);

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
        let text = document_text(self, &document_id);
        let position =
            match service_position_for_request(&id, "onTypeFormatting", &text, params.position) {
                Ok(position) => position,
                Err(response) => return response,
            };
        let edits = self
            .databases
            .on_type_formatting(&document_id, position, &params.ch);

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
        let text = document_text(self, &document_id);
        let positions = match params
            .positions
            .into_iter()
            .map(|position| service_position_for_request(&id, "selectionRange", &text, position))
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(positions) => positions,
            Err(response) => return response,
        };
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

        JsonRpcResult::Response(success_response(
            id,
            lsp_semantic_tokens(&tokens, &self.semantic_token_projection),
        ))
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

        JsonRpcResult::Response(success_response(
            id,
            lsp_semantic_token_delta(&delta, &self.semantic_token_projection),
        ))
    }

    pub(crate) fn semantic_tokens_range(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<SemanticTokensRangeParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid semanticTokens/range params: {error}"),
                ));
            }
        };

        let document_id = DocumentId::from(params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let text = document_text(self, &document_id);
        let range =
            match service_range_for_request(&id, "semanticTokens/range", &text, params.range) {
                Ok(range) => range,
                Err(response) => return response,
            };
        let tokens = self.databases.semantic_tokens_in_range(&document_id, range);

        JsonRpcResult::Response(success_response(
            id,
            lsp_semantic_tokens(&tokens, &self.semantic_token_projection),
        ))
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
        let text = document_text(self, &document_id);
        let range = match service_range_for_request(&id, "inlayHint", &text, params.range) {
            Ok(range) => range,
            Err(response) => return response,
        };
        let hints = self.databases.inlay_hints(&document_id, range);

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
