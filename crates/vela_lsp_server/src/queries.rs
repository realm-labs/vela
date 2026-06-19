use lsp_server::RequestId;
use serde_json::Value as JsonValue;
use vela_language_service::{DocumentId, LineIndex as ServiceLineIndex};

use crate::{
    ErrorCode, JsonRpcResult, LspServer,
    completion::service_completion_resolve_payload,
    lsp::{from_proto, to_proto},
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

impl LspServer {
    pub(crate) fn code_action(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<lsp_types::CodeActionParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid codeAction params: {error}"),
                );
            }
        };

        let document_id = from_proto::document_id(&params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let text = document_text(self, &document_id);
        let input = match from_proto::code_action_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid codeAction range: {error}"),
                );
            }
        };
        let actions = self.databases.code_actions(&input.document_id, input.range);

        JsonRpcResult::ok(
            id,
            serde_json::to_value(to_proto::code_actions(&actions))
                .expect("codeAction response should serialize"),
        )
    }

    pub(crate) fn completion(&mut self, id: Option<RequestId>, params: JsonValue) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<lsp_types::CompletionParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid completion params: {error}"),
                );
            }
        };

        let document_id = from_proto::document_id(&params.text_document_position.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let text = document_text(self, &document_id);
        let input = match from_proto::completion_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid completion position: {error}"),
                );
            }
        };
        let completions = self
            .databases
            .completion_items(&input.document_id, input.position);
        let line_index = ServiceLineIndex::new(&text);

        JsonRpcResult::ok(
            id,
            serde_json::to_value(to_proto::completion_response(&completions, &line_index))
                .expect("typed completion response should serialize"),
        )
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
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid completionItem/resolve payload: {error}"),
                );
            }
        };
        let params = match serde_json::from_value::<lsp_types::CompletionItem>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid completionItem/resolve params: {error}"),
                );
            }
        };
        let documentation =
            payload.and_then(|payload| self.databases.completion_documentation(&payload));
        JsonRpcResult::ok(
            id,
            serde_json::to_value(to_proto::completion_item_resolved(params, documentation))
                .expect("typed completion item should serialize"),
        )
    }

    pub(crate) fn signature_help(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<lsp_types::SignatureHelpParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid signatureHelp params: {error}"),
                );
            }
        };

        let document_id =
            from_proto::document_id(&params.text_document_position_params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let text = document_text(self, &document_id);
        let input = match from_proto::signature_help_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid signatureHelp position: {error}"),
                );
            }
        };
        let signatures = self
            .databases
            .signature_help(&input.document_id, input.position);

        JsonRpcResult::ok(
            id,
            signatures.as_ref().map_or(JsonValue::Null, |signatures| {
                serde_json::to_value(to_proto::signature_help(signatures))
                    .expect("typed signatureHelp response should serialize")
            }),
        )
    }

    pub(crate) fn hover(&mut self, id: Option<RequestId>, params: JsonValue) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<lsp_types::HoverParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid hover params: {error}"),
                );
            }
        };

        let document_id =
            from_proto::document_id(&params.text_document_position_params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let text = document_text(self, &document_id);
        let input = match from_proto::hover_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid hover position: {error}"),
                );
            }
        };
        let hover = self.databases.hover(&input.document_id, input.position);

        JsonRpcResult::ok(
            id,
            hover.as_ref().map_or(JsonValue::Null, |hover| {
                serde_json::to_value(to_proto::hover(hover))
                    .expect("typed hover response should serialize")
            }),
        )
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
        let params = match serde_json::from_value::<lsp_types::GotoDefinitionParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid {method_name} params: {error}"),
                );
            }
        };

        let document_id =
            from_proto::document_id(&params.text_document_position_params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let text = document_text(self, &document_id);
        let input = match from_proto::goto_definition_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid {method_name} position: {error}"),
                );
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

        JsonRpcResult::ok(
            id,
            definition.as_ref().map_or(JsonValue::Null, |definition| {
                serde_json::to_value(to_proto::definition_location(definition))
                    .expect("typed navigation response should serialize")
            }),
        )
    }

    pub(crate) fn references(&mut self, id: Option<RequestId>, params: JsonValue) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<lsp_types::ReferenceParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid references params: {error}"),
                );
            }
        };

        let document_id = from_proto::document_id(&params.text_document_position.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let text = document_text(self, &document_id);
        let input = match from_proto::reference_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid references position: {error}"),
                );
            }
        };
        let references = self.databases.references(
            &input.document_id,
            input.position,
            params.context.include_declaration,
        );

        JsonRpcResult::ok(
            id,
            serde_json::to_value(to_proto::reference_locations(&references))
                .expect("typed references response should serialize"),
        )
    }

    pub(crate) fn prepare_rename(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<lsp_types::TextDocumentPositionParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid prepareRename params: {error}"),
                );
            }
        };

        let document_id = from_proto::document_id(&params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let text = document_text(self, &document_id);
        let input = match from_proto::prepare_rename_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid prepareRename position: {error}"),
                );
            }
        };
        let prepare = self
            .databases
            .prepare_rename(&input.document_id, input.position);

        JsonRpcResult::ok(
            id,
            serde_json::to_value(prepare.as_ref().map(to_proto::prepare_rename))
                .expect("typed prepareRename response should serialize"),
        )
    }

    pub(crate) fn rename(&mut self, id: Option<RequestId>, params: JsonValue) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<lsp_types::RenameParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid rename params: {error}"),
                );
            }
        };

        let document_id = from_proto::document_id(&params.text_document_position.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let text = document_text(self, &document_id);
        let input = match from_proto::rename_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid rename position: {error}"),
                );
            }
        };
        let edit = self
            .databases
            .rename(&input.document_id, input.position, &params.new_name);

        JsonRpcResult::ok(
            id,
            serde_json::to_value(edit.as_ref().map(to_proto::workspace_edit))
                .expect("typed rename response should serialize"),
        )
    }

    pub(crate) fn prepare_call_hierarchy(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<lsp_types::CallHierarchyPrepareParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid prepareCallHierarchy params: {error}"),
                );
            }
        };

        let document_id =
            from_proto::document_id(&params.text_document_position_params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let text = document_text(self, &document_id);
        let input = match from_proto::prepare_call_hierarchy_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid prepareCallHierarchy position: {error}"),
                );
            }
        };
        let items = self
            .databases
            .prepare_call_hierarchy(&input.document_id, input.position);

        JsonRpcResult::ok(
            id,
            serde_json::to_value(to_proto::call_hierarchy_items(&items))
                .expect("typed prepareCallHierarchy response should serialize"),
        )
    }

    pub(crate) fn incoming_calls(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params =
            match serde_json::from_value::<lsp_types::CallHierarchyIncomingCallsParams>(params) {
                Ok(params) => params,
                Err(error) => {
                    return JsonRpcResult::error(
                        Some(id),
                        ErrorCode::InvalidRequest,
                        format!("invalid incomingCalls params: {error}"),
                    );
                }
            };

        let document_id = from_proto::document_id(&params.item.uri);
        self.refresh_databases_for_query(&document_id);
        let text = document_text(self, &document_id);
        let item = match from_proto::call_hierarchy_item(&text, &params.item) {
            Ok(item) => item,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid incomingCalls item range: {error}"),
                );
            }
        };
        let calls = self.databases.incoming_calls(&item);

        JsonRpcResult::ok(
            id,
            serde_json::to_value(to_proto::incoming_calls(&calls))
                .expect("typed incomingCalls response should serialize"),
        )
    }

    pub(crate) fn outgoing_calls(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params =
            match serde_json::from_value::<lsp_types::CallHierarchyOutgoingCallsParams>(params) {
                Ok(params) => params,
                Err(error) => {
                    return JsonRpcResult::error(
                        Some(id),
                        ErrorCode::InvalidRequest,
                        format!("invalid outgoingCalls params: {error}"),
                    );
                }
            };

        let document_id = from_proto::document_id(&params.item.uri);
        self.refresh_databases_for_query(&document_id);
        let text = document_text(self, &document_id);
        let item = match from_proto::call_hierarchy_item(&text, &params.item) {
            Ok(item) => item,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid outgoingCalls item range: {error}"),
                );
            }
        };
        let calls = self.databases.outgoing_calls(&item);

        JsonRpcResult::ok(
            id,
            serde_json::to_value(to_proto::outgoing_calls(&calls))
                .expect("typed outgoingCalls response should serialize"),
        )
    }

    pub(crate) fn document_highlight(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<lsp_types::DocumentHighlightParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid documentHighlight params: {error}"),
                );
            }
        };

        let document_id =
            from_proto::document_id(&params.text_document_position_params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let text = document_text(self, &document_id);
        let input = match from_proto::document_highlight_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid documentHighlight position: {error}"),
                );
            }
        };
        let highlights = self
            .databases
            .document_highlights(&input.document_id, input.position);

        JsonRpcResult::ok(
            id,
            serde_json::to_value(to_proto::document_highlights(&highlights))
                .expect("typed documentHighlight response should serialize"),
        )
    }

    pub(crate) fn document_symbol(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<lsp_types::DocumentSymbolParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid documentSymbol params: {error}"),
                );
            }
        };

        let document_id = from_proto::document_symbol_params(&params);
        self.refresh_databases_for_query(&document_id);
        let symbols = self.databases.document_symbols(&document_id);

        JsonRpcResult::ok(
            id,
            serde_json::to_value(to_proto::document_symbols(&symbols))
                .expect("typed documentSymbol response should serialize"),
        )
    }

    pub(crate) fn folding_range(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<lsp_types::FoldingRangeParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid foldingRange params: {error}"),
                );
            }
        };

        let document_id = from_proto::folding_range_params(&params);
        self.refresh_databases_for_query(&document_id);
        let ranges = self.databases.folding_ranges(&document_id);

        JsonRpcResult::ok(
            id,
            serde_json::to_value(to_proto::folding_ranges(&ranges))
                .expect("typed foldingRange response should serialize"),
        )
    }

    pub(crate) fn formatting(&mut self, id: Option<RequestId>, params: JsonValue) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<lsp_types::DocumentFormattingParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid formatting params: {error}"),
                );
            }
        };

        let document_id = from_proto::document_formatting_params(&params);
        self.refresh_databases_for_query(&document_id);
        let edits = self.databases.document_formatting(&document_id);

        JsonRpcResult::ok(
            id,
            serde_json::to_value(to_proto::text_edits(&edits))
                .expect("typed formatting response should serialize"),
        )
    }

    pub(crate) fn range_formatting(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params =
            match serde_json::from_value::<lsp_types::DocumentRangeFormattingParams>(params) {
                Ok(params) => params,
                Err(error) => {
                    return JsonRpcResult::error(
                        Some(id),
                        ErrorCode::InvalidRequest,
                        format!("invalid rangeFormatting params: {error}"),
                    );
                }
            };

        let document_id = from_proto::document_id(&params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let text = document_text(self, &document_id);
        let input = match from_proto::range_formatting_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid rangeFormatting range: {error}"),
                );
            }
        };
        let edits = self
            .databases
            .range_formatting(&input.document_id, input.range);

        JsonRpcResult::ok(
            id,
            serde_json::to_value(to_proto::text_edits(&edits))
                .expect("typed rangeFormatting response should serialize"),
        )
    }

    pub(crate) fn on_type_formatting(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params =
            match serde_json::from_value::<lsp_types::DocumentOnTypeFormattingParams>(params) {
                Ok(params) => params,
                Err(error) => {
                    return JsonRpcResult::error(
                        Some(id),
                        ErrorCode::InvalidRequest,
                        format!("invalid onTypeFormatting params: {error}"),
                    );
                }
            };

        let document_id = from_proto::document_id(&params.text_document_position.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let text = document_text(self, &document_id);
        let input = match from_proto::on_type_formatting_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid onTypeFormatting position: {error}"),
                );
            }
        };
        let edits =
            self.databases
                .on_type_formatting(&input.document_id, input.position, &input.trigger);

        JsonRpcResult::ok(
            id,
            serde_json::to_value(to_proto::text_edits(&edits))
                .expect("typed onTypeFormatting response should serialize"),
        )
    }

    pub(crate) fn selection_range(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<lsp_types::SelectionRangeParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid selectionRange params: {error}"),
                );
            }
        };

        let document_id = from_proto::document_id(&params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let text = document_text(self, &document_id);
        let input = match from_proto::selection_range_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid selectionRange position: {error}"),
                );
            }
        };
        let ranges = self
            .databases
            .selection_ranges(&input.document_id, &input.positions);

        JsonRpcResult::ok(
            id,
            serde_json::to_value(to_proto::selection_ranges(&ranges))
                .expect("typed selectionRange response should serialize"),
        )
    }

    pub(crate) fn semantic_tokens_full(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<lsp_types::SemanticTokensParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid semanticTokens/full params: {error}"),
                );
            }
        };

        let document_id = from_proto::semantic_tokens_params(&params);
        self.refresh_databases_for_query(&document_id);
        let tokens = self.databases.semantic_tokens(&document_id);

        JsonRpcResult::ok(
            id,
            serde_json::to_value(to_proto::semantic_tokens(
                &tokens,
                &self.semantic_token_projection,
            ))
            .expect("semanticTokens/full response should serialize"),
        )
    }

    pub(crate) fn semantic_tokens_full_delta(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<lsp_types::SemanticTokensDeltaParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid semanticTokens/full/delta params: {error}"),
                );
            }
        };

        let input = from_proto::semantic_tokens_delta_params(&params);
        let document_id = input.document_id;
        self.refresh_databases_for_query(&document_id);
        let delta = self
            .databases
            .semantic_token_delta(&document_id, &input.previous_result_id);

        JsonRpcResult::ok(
            id,
            serde_json::to_value(to_proto::semantic_tokens_delta(
                &delta,
                &self.semantic_token_projection,
            ))
            .expect("semanticTokens/full/delta response should serialize"),
        )
    }

    pub(crate) fn semantic_tokens_range(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<lsp_types::SemanticTokensRangeParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid semanticTokens/range params: {error}"),
                );
            }
        };

        let document_id = from_proto::document_id(&params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let text = document_text(self, &document_id);
        let input = match from_proto::semantic_tokens_range_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid semanticTokens/range range: {error}"),
                );
            }
        };
        let tokens = self
            .databases
            .semantic_tokens_in_range(&input.document_id, input.range);

        JsonRpcResult::ok(
            id,
            serde_json::to_value(to_proto::semantic_tokens_range(
                &tokens,
                &self.semantic_token_projection,
            ))
            .expect("semanticTokens/range response should serialize"),
        )
    }

    pub(crate) fn inlay_hint(&mut self, id: Option<RequestId>, params: JsonValue) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<lsp_types::InlayHintParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid inlayHint params: {error}"),
                );
            }
        };

        let document_id = from_proto::document_id(&params.text_document.uri);
        self.refresh_databases_for_query(&document_id);
        let text = document_text(self, &document_id);
        let input = match from_proto::inlay_hint_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid inlayHint range: {error}"),
                );
            }
        };
        let hints = self.databases.inlay_hints(&input.document_id, input.range);

        JsonRpcResult::ok(
            id,
            serde_json::to_value(to_proto::inlay_hints(&hints))
                .expect("inlayHint response should serialize"),
        )
    }

    pub(crate) fn workspace_symbol(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        let params = match serde_json::from_value::<lsp_types::WorkspaceSymbolParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid workspace/symbol params: {error}"),
                );
            }
        };

        self.refresh_databases_for_workspace_query();
        let symbols = self
            .databases
            .workspace_symbols(from_proto::workspace_symbol_params(&params));

        JsonRpcResult::ok(
            id,
            serde_json::to_value(to_proto::workspace_symbols(&symbols))
                .expect("typed workspace/symbol response should serialize"),
        )
    }
}
