use std::collections::BTreeSet;

mod diagnostics;
mod documents;
mod project_state;
mod request_queue;
mod responses;

use crossbeam_channel::Sender;
use lsp_server::{Message, RequestId};
use lsp_types::{
    CallHierarchyIncomingCallsParams, CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams,
    CodeActionParams, CompletionParams, DidChangeConfigurationParams, DidChangeTextDocumentParams,
    DidChangeWatchedFilesParams, DidChangeWorkspaceFoldersParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DocumentFormattingParams, DocumentHighlightParams,
    DocumentOnTypeFormattingParams, DocumentRangeFormattingParams, DocumentSymbolParams,
    FoldingRangeParams, HoverParams, InlayHintParams, ReferenceParams, RenameParams,
    SelectionRangeParams, SemanticTokensDeltaParams, SemanticTokensParams,
    SemanticTokensRangeParams, SignatureHelpParams, TextDocumentPositionParams,
    WorkspaceSymbolParams,
};
use vela_language_service::{
    DocumentId, GenerationToken, LanguageServiceDatabases, LineIndex as ServiceLineIndex,
    WorkspaceConfig, WorkspaceGeneration, WorkspaceRoot, WorkspaceSnapshot,
};

use self::{
    diagnostics::{publish_diagnostics_notification, with_work_done_progress},
    documents::{apply_document_changes, source_version},
    project_state::ProjectState,
    request_queue::RequestQueue,
    responses::{
        error as response_error_messages, ok as response_ok_messages,
        ok_typed as response_ok_typed_messages,
    },
};
use crate::lsp::{from_proto, to_proto};
use crate::{
    ErrorCode, LaunchConfiguration,
    capabilities::initialize_result,
    completion::service_completion_resolve_payload,
    config::EditorConfiguration,
    config_change::ConfigChange,
    handlers::dispatch,
    lifecycle::{
        lsp_semantic_token_projection, lsp_supports_watched_file_registration,
        lsp_supports_work_done_progress, workspace_roots_from_lsp_initialize,
    },
    reload::{ReloadOperation, ReloadScheduler, ReloadWork},
    rpc::request_id_from_number_or_string,
    semantic_tokens::SemanticTokenProjection,
    task::{TaskOutcome, TaskResult, TaskScheduler, TaskSendSummary},
    transport::ResultSummary,
    watching,
};

pub(crate) struct GlobalState {
    sender: Sender<Message>,
    launch_configuration: LaunchConfiguration,
    request_queue: RequestQueue,
    reload_scheduler: ReloadScheduler,
    task_scheduler: TaskScheduler,
    project: ProjectState,
    client_supports_work_done_progress: bool,
    client_supports_watched_file_registration: bool,
    semantic_token_projection: SemanticTokenProjection,
    watched_files_registered: bool,
    watch_files_enabled: bool,
    initialized: bool,
    shutdown_requested: bool,
    exited: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) struct GlobalStateSnapshot {
    launch_configuration: LaunchConfiguration,
    workspace: WorkspaceSnapshot,
    databases: LanguageServiceDatabases,
    workspace_roots: BTreeSet<String>,
    open_documents: BTreeSet<DocumentId>,
    editor_config: Option<EditorConfiguration>,
    workspace_config: Option<WorkspaceConfig>,
    client_supports_work_done_progress: bool,
    client_supports_watched_file_registration: bool,
    semantic_token_projection: SemanticTokenProjection,
    watched_files_registered: bool,
    watch_files_enabled: bool,
    generation: WorkspaceGeneration,
    initialized: bool,
    shutdown_requested: bool,
}

#[allow(dead_code)]
impl GlobalStateSnapshot {
    pub(crate) const fn launch_configuration(&self) -> &LaunchConfiguration {
        &self.launch_configuration
    }

    pub(crate) const fn workspace(&self) -> &WorkspaceSnapshot {
        &self.workspace
    }

    pub(crate) const fn databases(&self) -> &LanguageServiceDatabases {
        &self.databases
    }

    pub(crate) const fn generation(&self) -> WorkspaceGeneration {
        self.generation
    }

    pub(crate) const fn workspace_roots(&self) -> &BTreeSet<String> {
        &self.workspace_roots
    }

    pub(crate) const fn open_documents(&self) -> &BTreeSet<DocumentId> {
        &self.open_documents
    }

    pub(crate) fn editor_config(&self) -> Option<&EditorConfiguration> {
        self.editor_config.as_ref()
    }

    pub(crate) fn workspace_config(&self) -> Option<&WorkspaceConfig> {
        self.workspace_config.as_ref()
    }

    pub(crate) const fn client_supports_work_done_progress(&self) -> bool {
        self.client_supports_work_done_progress
    }

    pub(crate) const fn client_supports_watched_file_registration(&self) -> bool {
        self.client_supports_watched_file_registration
    }

    pub(crate) const fn semantic_token_projection(&self) -> &SemanticTokenProjection {
        &self.semantic_token_projection
    }

    pub(crate) const fn watched_files_registered(&self) -> bool {
        self.watched_files_registered
    }

    pub(crate) const fn watch_files_enabled(&self) -> bool {
        self.watch_files_enabled
    }

    pub(crate) const fn is_initialized(&self) -> bool {
        self.initialized
    }

    pub(crate) const fn is_shutdown_requested(&self) -> bool {
        self.shutdown_requested
    }

    pub(crate) fn completion(
        self,
        id: lsp_server::RequestId,
        params: CompletionParams,
    ) -> Vec<Message> {
        let document_id = from_proto::document_id(&params.text_document_position.text_document.uri);
        let text = snapshot_document_text(&self, &document_id);
        let input = match from_proto::completion_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return response_error_messages(
                    id,
                    ErrorCode::InvalidRequest,
                    format!("invalid completion position: {error}"),
                );
            }
        };
        let completions = self
            .databases
            .completion_items(&input.document_id, input.position);
        let line_index = ServiceLineIndex::new(&text);

        response_ok_typed_messages(
            id,
            to_proto::completion_response(&completions, &line_index),
            "typed completion response",
        )
    }

    pub(crate) fn completion_resolve(
        self,
        id: lsp_server::RequestId,
        params: lsp_types::CompletionItem,
    ) -> Vec<Message> {
        let params_value =
            serde_json::to_value(&params).expect("typed completion item should serialize");
        let payload = match service_completion_resolve_payload(&params_value) {
            Ok(payload) => payload,
            Err(error) => {
                return response_error_messages(
                    id,
                    ErrorCode::InvalidRequest,
                    format!("invalid completionItem/resolve payload: {error}"),
                );
            }
        };
        let documentation =
            payload.and_then(|payload| self.databases.completion_documentation(&payload));
        response_ok_typed_messages(
            id,
            to_proto::completion_item_resolved(params, documentation),
            "typed completion item",
        )
    }

    pub(crate) fn hover(self, id: lsp_server::RequestId, params: HoverParams) -> Vec<Message> {
        let document_id =
            from_proto::document_id(&params.text_document_position_params.text_document.uri);
        let text = snapshot_document_text(&self, &document_id);
        let input = match from_proto::hover_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return response_error_messages(
                    id,
                    ErrorCode::InvalidRequest,
                    format!("invalid hover position: {error}"),
                );
            }
        };
        let hover = self.databases.hover(&input.document_id, input.position);

        response_ok_typed_messages(
            id,
            hover.as_ref().map(to_proto::hover),
            "typed hover response",
        )
    }

    pub(crate) fn signature_help(
        self,
        id: lsp_server::RequestId,
        params: SignatureHelpParams,
    ) -> Vec<Message> {
        let document_id =
            from_proto::document_id(&params.text_document_position_params.text_document.uri);
        let text = snapshot_document_text(&self, &document_id);
        let input = match from_proto::signature_help_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return response_error_messages(
                    id,
                    ErrorCode::InvalidRequest,
                    format!("invalid signatureHelp position: {error}"),
                );
            }
        };
        let signatures = self
            .databases
            .signature_help(&input.document_id, input.position);

        response_ok_typed_messages(
            id,
            signatures.as_ref().map(to_proto::signature_help),
            "typed signatureHelp response",
        )
    }

    pub(crate) fn semantic_tokens_full(
        self,
        id: lsp_server::RequestId,
        params: SemanticTokensParams,
    ) -> Vec<Message> {
        let document_id = from_proto::semantic_tokens_params(&params);
        let tokens = self.databases.semantic_tokens(&document_id);

        response_ok_typed_messages(
            id,
            to_proto::semantic_tokens(&tokens, &self.semantic_token_projection),
            "typed semanticTokens/full response",
        )
    }

    pub(crate) fn semantic_tokens_full_delta(
        self,
        id: lsp_server::RequestId,
        params: SemanticTokensDeltaParams,
    ) -> Vec<Message> {
        let input = from_proto::semantic_tokens_delta_params(&params);
        let delta = self
            .databases
            .semantic_token_delta(&input.document_id, &input.previous_result_id);

        response_ok_typed_messages(
            id,
            to_proto::semantic_tokens_delta(&delta, &self.semantic_token_projection),
            "typed semanticTokens/full/delta response",
        )
    }

    pub(crate) fn semantic_tokens_range(
        self,
        id: lsp_server::RequestId,
        params: SemanticTokensRangeParams,
    ) -> Vec<Message> {
        let document_id = from_proto::document_id(&params.text_document.uri);
        let text = snapshot_document_text(&self, &document_id);
        let input = match from_proto::semantic_tokens_range_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return response_error_messages(
                    id,
                    ErrorCode::InvalidRequest,
                    format!("invalid semanticTokens/range params: {error}"),
                );
            }
        };
        let tokens = self
            .databases
            .semantic_tokens_in_range(&input.document_id, input.range);

        response_ok_typed_messages(
            id,
            to_proto::semantic_tokens_range(&tokens, &self.semantic_token_projection),
            "typed semanticTokens/range response",
        )
    }

    pub(crate) fn formatting(
        self,
        id: lsp_server::RequestId,
        params: DocumentFormattingParams,
    ) -> Vec<Message> {
        let document_id = from_proto::document_formatting_params(&params);
        let edits = self.databases.document_formatting(&document_id);

        response_ok_typed_messages(
            id,
            to_proto::text_edits(&edits),
            "typed formatting response",
        )
    }

    pub(crate) fn range_formatting(
        self,
        id: lsp_server::RequestId,
        params: DocumentRangeFormattingParams,
    ) -> Vec<Message> {
        let document_id = from_proto::document_id(&params.text_document.uri);
        let text = snapshot_document_text(&self, &document_id);
        let input = match from_proto::range_formatting_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return response_error_messages(
                    id,
                    ErrorCode::InvalidRequest,
                    format!("invalid rangeFormatting params: {error}"),
                );
            }
        };
        let edits = self
            .databases
            .range_formatting(&input.document_id, input.range);

        response_ok_typed_messages(
            id,
            to_proto::text_edits(&edits),
            "typed rangeFormatting response",
        )
    }

    pub(crate) fn on_type_formatting(
        self,
        id: lsp_server::RequestId,
        params: DocumentOnTypeFormattingParams,
    ) -> Vec<Message> {
        let document_id = from_proto::document_id(&params.text_document_position.text_document.uri);
        let text = snapshot_document_text(&self, &document_id);
        let input = match from_proto::on_type_formatting_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return response_error_messages(
                    id,
                    ErrorCode::InvalidRequest,
                    format!("invalid onTypeFormatting params: {error}"),
                );
            }
        };
        let edits =
            self.databases
                .on_type_formatting(&input.document_id, input.position, &input.trigger);

        response_ok_typed_messages(
            id,
            to_proto::text_edits(&edits),
            "typed onTypeFormatting response",
        )
    }

    pub(crate) fn definition(
        self,
        id: lsp_server::RequestId,
        params: lsp_types::GotoDefinitionParams,
    ) -> Vec<Message> {
        self.navigation_location(
            id,
            params,
            "definition",
            SnapshotNavigationLocationQuery::Definition,
        )
    }

    pub(crate) fn declaration(
        self,
        id: lsp_server::RequestId,
        params: lsp_types::request::GotoDeclarationParams,
    ) -> Vec<Message> {
        self.navigation_location(
            id,
            params,
            "declaration",
            SnapshotNavigationLocationQuery::Declaration,
        )
    }

    pub(crate) fn type_definition(
        self,
        id: lsp_server::RequestId,
        params: lsp_types::request::GotoTypeDefinitionParams,
    ) -> Vec<Message> {
        self.navigation_location(
            id,
            params,
            "typeDefinition",
            SnapshotNavigationLocationQuery::TypeDefinition,
        )
    }

    pub(crate) fn references(
        self,
        id: lsp_server::RequestId,
        params: ReferenceParams,
    ) -> Vec<Message> {
        let document_id = from_proto::document_id(&params.text_document_position.text_document.uri);
        let text = snapshot_document_text(&self, &document_id);
        let input = match from_proto::reference_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return response_error_messages(
                    id,
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

        response_ok_typed_messages(
            id,
            to_proto::reference_locations(&references),
            "typed references response",
        )
    }

    pub(crate) fn document_highlight(
        self,
        id: lsp_server::RequestId,
        params: DocumentHighlightParams,
    ) -> Vec<Message> {
        let document_id =
            from_proto::document_id(&params.text_document_position_params.text_document.uri);
        let text = snapshot_document_text(&self, &document_id);
        let input = match from_proto::document_highlight_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return response_error_messages(
                    id,
                    ErrorCode::InvalidRequest,
                    format!("invalid documentHighlight params: {error}"),
                );
            }
        };
        let highlights = self
            .databases
            .document_highlights(&input.document_id, input.position);

        response_ok_typed_messages(
            id,
            to_proto::document_highlights(&highlights),
            "typed documentHighlight response",
        )
    }

    pub(crate) fn document_symbol(
        self,
        id: lsp_server::RequestId,
        params: DocumentSymbolParams,
    ) -> Vec<Message> {
        let document_id = from_proto::document_symbol_params(&params);
        let symbols = self.databases.document_symbols(&document_id);

        response_ok_typed_messages(
            id,
            to_proto::document_symbols(&symbols),
            "typed documentSymbol response",
        )
    }

    pub(crate) fn workspace_symbol(
        self,
        id: lsp_server::RequestId,
        params: WorkspaceSymbolParams,
    ) -> Vec<Message> {
        let symbols = self
            .databases
            .workspace_symbols(from_proto::workspace_symbol_params(&params));

        response_ok_typed_messages(
            id,
            to_proto::workspace_symbols(&symbols),
            "typed workspace/symbol response",
        )
    }

    pub(crate) fn folding_range(
        self,
        id: lsp_server::RequestId,
        params: FoldingRangeParams,
    ) -> Vec<Message> {
        let document_id = from_proto::folding_range_params(&params);
        let ranges = self.databases.folding_ranges(&document_id);

        response_ok_typed_messages(
            id,
            to_proto::folding_ranges(&ranges),
            "typed foldingRange response",
        )
    }

    pub(crate) fn selection_range(
        self,
        id: lsp_server::RequestId,
        params: SelectionRangeParams,
    ) -> Vec<Message> {
        let document_id = from_proto::document_id(&params.text_document.uri);
        let text = snapshot_document_text(&self, &document_id);
        let input = match from_proto::selection_range_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return response_error_messages(
                    id,
                    ErrorCode::InvalidRequest,
                    format!("invalid selectionRange params: {error}"),
                );
            }
        };
        let ranges = self
            .databases
            .selection_ranges(&input.document_id, &input.positions);

        response_ok_typed_messages(
            id,
            to_proto::selection_ranges(&ranges),
            "typed selectionRange response",
        )
    }

    pub(crate) fn prepare_rename(
        self,
        id: lsp_server::RequestId,
        params: TextDocumentPositionParams,
    ) -> Vec<Message> {
        let document_id = from_proto::document_id(&params.text_document.uri);
        let text = snapshot_document_text(&self, &document_id);
        let input = match from_proto::prepare_rename_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return response_error_messages(
                    id,
                    ErrorCode::InvalidRequest,
                    format!("invalid prepareRename position: {error}"),
                );
            }
        };
        let prepare = self
            .databases
            .prepare_rename(&input.document_id, input.position);

        response_ok_typed_messages(
            id,
            prepare.as_ref().map(to_proto::prepare_rename),
            "typed prepareRename response",
        )
    }

    pub(crate) fn rename(self, id: lsp_server::RequestId, params: RenameParams) -> Vec<Message> {
        let document_id = from_proto::document_id(&params.text_document_position.text_document.uri);
        let text = snapshot_document_text(&self, &document_id);
        let input = match from_proto::rename_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return response_error_messages(
                    id,
                    ErrorCode::InvalidRequest,
                    format!("invalid rename position: {error}"),
                );
            }
        };
        let edit = self
            .databases
            .rename(&input.document_id, input.position, &params.new_name);

        response_ok_typed_messages(
            id,
            edit.as_ref().map(to_proto::workspace_edit),
            "typed rename response",
        )
    }

    pub(crate) fn prepare_call_hierarchy(
        self,
        id: lsp_server::RequestId,
        params: CallHierarchyPrepareParams,
    ) -> Vec<Message> {
        let document_id =
            from_proto::document_id(&params.text_document_position_params.text_document.uri);
        let text = snapshot_document_text(&self, &document_id);
        let input = match from_proto::prepare_call_hierarchy_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return response_error_messages(
                    id,
                    ErrorCode::InvalidRequest,
                    format!("invalid prepareCallHierarchy position: {error}"),
                );
            }
        };
        let items = self
            .databases
            .prepare_call_hierarchy(&input.document_id, input.position);

        response_ok_typed_messages(
            id,
            to_proto::call_hierarchy_items(&items),
            "typed prepareCallHierarchy response",
        )
    }

    pub(crate) fn incoming_calls(
        self,
        id: lsp_server::RequestId,
        params: CallHierarchyIncomingCallsParams,
    ) -> Vec<Message> {
        let document_id = from_proto::document_id(&params.item.uri);
        let text = snapshot_document_text(&self, &document_id);
        let item = match from_proto::call_hierarchy_item(&text, &params.item) {
            Ok(item) => item,
            Err(error) => {
                return response_error_messages(
                    id,
                    ErrorCode::InvalidRequest,
                    format!("invalid incomingCalls item range: {error}"),
                );
            }
        };
        let calls = self.databases.incoming_calls(&item);

        response_ok_typed_messages(
            id,
            to_proto::incoming_calls(&calls),
            "typed incomingCalls response",
        )
    }

    pub(crate) fn outgoing_calls(
        self,
        id: lsp_server::RequestId,
        params: CallHierarchyOutgoingCallsParams,
    ) -> Vec<Message> {
        let document_id = from_proto::document_id(&params.item.uri);
        let text = snapshot_document_text(&self, &document_id);
        let item = match from_proto::call_hierarchy_item(&text, &params.item) {
            Ok(item) => item,
            Err(error) => {
                return response_error_messages(
                    id,
                    ErrorCode::InvalidRequest,
                    format!("invalid outgoingCalls item range: {error}"),
                );
            }
        };
        let calls = self.databases.outgoing_calls(&item);

        response_ok_typed_messages(
            id,
            to_proto::outgoing_calls(&calls),
            "typed outgoingCalls response",
        )
    }

    pub(crate) fn code_action(
        self,
        id: lsp_server::RequestId,
        params: CodeActionParams,
    ) -> Vec<Message> {
        let document_id = from_proto::document_id(&params.text_document.uri);
        let text = snapshot_document_text(&self, &document_id);
        let input = match from_proto::code_action_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return response_error_messages(
                    id,
                    ErrorCode::InvalidRequest,
                    format!("invalid codeAction params: {error}"),
                );
            }
        };
        let actions = self.databases.code_actions(&input.document_id, input.range);

        response_ok_typed_messages(
            id,
            to_proto::code_actions(&actions),
            "typed codeAction response",
        )
    }

    pub(crate) fn inlay_hint(
        self,
        id: lsp_server::RequestId,
        params: InlayHintParams,
    ) -> Vec<Message> {
        let document_id = from_proto::document_id(&params.text_document.uri);
        let text = snapshot_document_text(&self, &document_id);
        let input = match from_proto::inlay_hint_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return response_error_messages(
                    id,
                    ErrorCode::InvalidRequest,
                    format!("invalid inlayHint params: {error}"),
                );
            }
        };
        let hints = self.databases.inlay_hints(&input.document_id, input.range);

        response_ok_typed_messages(
            id,
            to_proto::inlay_hints(&hints),
            "typed inlayHint response",
        )
    }

    fn navigation_location(
        self,
        id: lsp_server::RequestId,
        params: lsp_types::GotoDefinitionParams,
        method_name: &'static str,
        query: SnapshotNavigationLocationQuery,
    ) -> Vec<Message> {
        let document_id =
            from_proto::document_id(&params.text_document_position_params.text_document.uri);
        let text = snapshot_document_text(&self, &document_id);
        let input = match from_proto::goto_definition_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return response_error_messages(
                    id,
                    ErrorCode::InvalidRequest,
                    format!("invalid {method_name} position: {error}"),
                );
            }
        };
        let definition = match query {
            SnapshotNavigationLocationQuery::Definition => self
                .databases
                .definition(&input.document_id, input.position),
            SnapshotNavigationLocationQuery::Declaration => self
                .databases
                .declaration(&input.document_id, input.position),
            SnapshotNavigationLocationQuery::TypeDefinition => self
                .databases
                .type_definition(&input.document_id, input.position),
        };

        response_ok_typed_messages(
            id,
            definition.as_ref().map(to_proto::definition_location),
            "typed navigation response",
        )
    }
}

enum SnapshotNavigationLocationQuery {
    Definition,
    Declaration,
    TypeDefinition,
}

fn snapshot_document_text(snapshot: &GlobalStateSnapshot, document_id: &DocumentId) -> String {
    snapshot
        .workspace
        .document_text(document_id)
        .map(std::borrow::ToOwned::to_owned)
        .or_else(|| {
            snapshot
                .databases
                .source_db()
                .records()
                .get(document_id)
                .map(|source| source.text().to_owned())
        })
        .unwrap_or_default()
}

impl GlobalState {
    pub(crate) fn new(sender: Sender<Message>, launch_configuration: LaunchConfiguration) -> Self {
        let watch_files_enabled = launch_configuration.watch_files_enabled();
        let project = ProjectState::new(launch_configuration.clone());
        Self {
            sender,
            launch_configuration,
            request_queue: RequestQueue::default(),
            reload_scheduler: ReloadScheduler::default(),
            task_scheduler: TaskScheduler::default(),
            project,
            client_supports_work_done_progress: false,
            client_supports_watched_file_registration: false,
            semantic_token_projection: SemanticTokenProjection::default(),
            watched_files_registered: false,
            watch_files_enabled,
            initialized: false,
            shutdown_requested: false,
            exited: false,
        }
    }

    pub(crate) const fn launch_configuration(&self) -> &LaunchConfiguration {
        &self.launch_configuration
    }

    #[allow(dead_code)]
    pub(crate) fn snapshot(&self) -> GlobalStateSnapshot {
        GlobalStateSnapshot {
            launch_configuration: self.launch_configuration.clone(),
            workspace: self.project.workspace_snapshot(),
            databases: self.project.databases.clone(),
            workspace_roots: self.project.workspace_roots.clone(),
            open_documents: self.project.open_documents.clone(),
            editor_config: self.project.editor_config.clone(),
            workspace_config: self.project.config.clone(),
            client_supports_work_done_progress: self.client_supports_work_done_progress,
            client_supports_watched_file_registration: self
                .client_supports_watched_file_registration,
            semantic_token_projection: self.semantic_token_projection.clone(),
            watched_files_registered: self.watched_files_registered,
            watch_files_enabled: self.watch_files_enabled,
            generation: self.project.databases.generation(),
            initialized: self.initialized,
            shutdown_requested: self.shutdown_requested,
        }
    }

    pub(crate) fn handle_message(&mut self, message: &Message) -> anyhow::Result<Vec<Message>> {
        let request_id = RequestQueue::request_id(message);
        if let Some(id) = request_id.as_ref() {
            self.request_queue.start(id.clone());
        }
        let messages = dispatch::dispatch_message(self, message);
        if let Some(id) = request_id {
            self.request_queue.finish(&id);
        }
        Ok(messages)
    }

    pub(crate) fn send_messages(&self, messages: Vec<Message>) -> anyhow::Result<ResultSummary> {
        let summary = ResultSummary::from_messages(&messages);
        for message in messages {
            self.sender.send(message)?;
        }
        Ok(summary)
    }

    pub(crate) fn send_task_result(
        &mut self,
        result: TaskResult,
    ) -> anyhow::Result<TaskSendSummary> {
        let _lane = result.lane();
        let _method = result.method();
        let retry = result.retry().cloned();
        let is_cancelled = result
            .generation_token()
            .is_some_and(GenerationToken::is_cancelled);
        let is_stale = result.generation_token().is_some_and(|generation| {
            generation.generation() != self.project.databases.generation()
        });
        let request_id = result.request_id().cloned();
        if let Some(request_id) = request_id.as_ref() {
            self.request_queue.finish_in_flight(request_id);
        }
        if is_cancelled {
            let summary = if let Some(request_id) = request_id {
                self.send_messages(dispatch::request_cancelled(request_id))?
            } else {
                self.send_messages(Vec::new())?
            };
            return Ok(TaskSendSummary::new(summary, TaskOutcome::Cancelled));
        }
        if is_stale {
            if let Some(retry) = retry.and_then(|retry| retry.next_attempt()) {
                dispatch::retry_stale_request(self, retry);
                let summary = self.send_messages(Vec::new())?;
                return Ok(TaskSendSummary::new(summary, TaskOutcome::Retried));
            }
            let summary = if let Some(request_id) = request_id {
                self.send_messages(dispatch::content_modified(request_id))?
            } else {
                self.send_messages(Vec::new())?
            };
            return Ok(TaskSendSummary::new(summary, TaskOutcome::StaleDiscarded));
        }
        let summary = self.send_messages(result.into_messages())?;
        Ok(TaskSendSummary::new(summary, TaskOutcome::Completed))
    }

    pub(crate) const fn task_scheduler(&self) -> &TaskScheduler {
        &self.task_scheduler
    }

    pub(crate) fn register_in_flight_cancellation(&mut self, id: RequestId) -> GenerationToken {
        let (token, handle) = self
            .project
            .databases
            .begin_cancellable_background_request();
        self.request_queue.start_in_flight(id, handle);
        token
    }

    pub(crate) const fn is_exited(&self) -> bool {
        self.exited
    }

    pub(crate) const fn is_initialized(&self) -> bool {
        self.initialized
    }

    pub(crate) const fn is_shutdown_requested(&self) -> bool {
        self.shutdown_requested
    }

    pub(crate) fn take_cancelled_request(&mut self, id: &RequestId) -> bool {
        self.request_queue.take_cancelled(id)
    }

    pub(crate) fn apply_config_change(&mut self, change: ConfigChange) {
        let watch_files_enabled = change.watch_files_enabled();
        self.project.apply_config_change(change);
        if let Some(enabled) = watch_files_enabled {
            self.watch_files_enabled = enabled;
        }
    }

    pub(crate) fn initialize(
        &mut self,
        id: lsp_server::RequestId,
        params: lsp_types::InitializeParams,
    ) -> Vec<Message> {
        if self.initialized {
            return response_error_messages(
                id,
                ErrorCode::InvalidRequest,
                "server is already initialized",
            );
        }

        let editor_config = match params
            .initialization_options
            .clone()
            .map(serde_json::from_value)
            .transpose()
        {
            Ok(editor_config) => editor_config,
            Err(error) => {
                return response_error_messages(
                    id,
                    ErrorCode::InvalidParams,
                    format!("invalid initialize params: {error}"),
                );
            }
        };

        self.initialized = true;
        self.apply_config_change(ConfigChange::from_initialize(
            workspace_roots_from_lsp_initialize(&params),
            editor_config,
        ));
        self.project.refresh_databases();
        self.client_supports_work_done_progress = lsp_supports_work_done_progress(&params);
        self.client_supports_watched_file_registration =
            lsp_supports_watched_file_registration(&params);
        self.semantic_token_projection = lsp_semantic_token_projection(&params);
        response_ok_messages(id, initialize_result(&self.semantic_token_projection))
    }

    pub(crate) fn shutdown(&mut self, id: lsp_server::RequestId, _params: ()) -> Vec<Message> {
        self.shutdown_requested = true;
        response_ok_messages(id, serde_json::Value::Null)
    }

    pub(crate) fn initialized(&mut self, _params: lsp_types::InitializedParams) -> Vec<Message> {
        self.register_watched_files_after_initialized()
    }

    pub(crate) fn exit(&mut self, _params: ()) -> Vec<Message> {
        self.exited = true;
        Vec::new()
    }

    pub(crate) fn cancel_request(&mut self, params: lsp_types::CancelParams) -> Vec<Message> {
        self.request_queue
            .cancel(request_id_from_number_or_string(params.id));
        Vec::new()
    }

    pub(crate) fn did_change_configuration(
        &mut self,
        params: DidChangeConfigurationParams,
    ) -> Vec<Message> {
        let editor_config = match EditorConfiguration::from_settings(params.settings) {
            Ok(config) => config,
            Err(error) => {
                return vec![publish_diagnostics_notification(
                    "",
                    Vec::new(),
                    Some(format!("invalid didChangeConfiguration settings: {error}")),
                )];
            }
        };

        self.apply_config_change(ConfigChange::from_editor_settings(editor_config));
        self.project.refresh_databases();
        self.project.publish_open_diagnostics()
    }

    pub(crate) fn did_change_workspace_folders(
        &mut self,
        params: DidChangeWorkspaceFoldersParams,
    ) -> Vec<Message> {
        let mut workspace_roots = self.project.workspace_roots.clone();
        for folder in params.event.removed {
            let root = WorkspaceRoot::from(folder.uri.to_string());
            workspace_roots.remove(root.path());
        }
        for folder in params.event.added {
            let root = WorkspaceRoot::from(folder.uri.to_string());
            workspace_roots.insert(root.path().to_owned());
        }
        self.reload_scheduler
            .schedule_workspace_roots(workspace_roots);
        for work in self.reload_scheduler.drain() {
            self.apply_reload_work(work);
        }
        self.project.refresh_databases();
        self.publish_workspace_diagnostics()
    }

    pub(crate) fn did_change_watched_files(
        &mut self,
        params: DidChangeWatchedFilesParams,
    ) -> Vec<Message> {
        let schema_path = self.project.schema_path().map(str::to_owned);
        self.reload_scheduler.schedule_watched_files(
            params.changes,
            schema_path.as_deref(),
            &self.project.open_documents,
        );
        for work in self.reload_scheduler.drain() {
            self.apply_reload_work(work);
        }
        self.project.refresh_databases_after_watched_changes();
        self.publish_workspace_diagnostics()
    }

    pub(crate) fn did_open(&mut self, params: DidOpenTextDocumentParams) -> Vec<Message> {
        let uri = params.text_document.uri.to_string();
        let document_id = DocumentId::from(uri.clone());
        let version = source_version(params.text_document.version);
        self.project.workspace.open_document(
            document_id.clone(),
            params.text_document.text,
            version,
        );
        self.project.open_documents.insert(document_id.clone());
        self.project.refresh_document_databases(&document_id);
        let message = self
            .project
            .publish_document_diagnostics(&uri, &document_id);
        vec![message]
    }

    pub(crate) fn did_change(&mut self, params: DidChangeTextDocumentParams) -> Vec<Message> {
        if params.content_changes.is_empty() {
            return vec![publish_diagnostics_notification(
                params.text_document.uri.as_str(),
                Vec::new(),
                Some("didChange requires at least one content change".to_owned()),
            )];
        }

        let uri = params.text_document.uri.to_string();
        let document_id = DocumentId::from(uri.clone());
        let version = source_version(params.text_document.version);
        let current_text = self
            .project
            .workspace
            .document_text(&document_id)
            .map(std::borrow::ToOwned::to_owned);
        let changes = params.content_changes;
        let text = match apply_document_changes(current_text.as_deref(), changes) {
            Ok(text) => text,
            Err(error) => {
                return vec![publish_diagnostics_notification(
                    &uri,
                    Vec::new(),
                    Some(error),
                )];
            }
        };

        self.project
            .workspace
            .change_document(document_id.clone(), text, version);
        self.project.open_documents.insert(document_id.clone());
        self.project.refresh_document_databases(&document_id);
        let message = self
            .project
            .publish_document_diagnostics(&uri, &document_id);
        vec![message]
    }

    pub(crate) fn did_close(&mut self, params: DidCloseTextDocumentParams) -> Vec<Message> {
        let uri = params.text_document.uri.to_string();
        let document_id = DocumentId::from(uri.clone());
        self.project.workspace.close_document(&document_id);
        self.project.open_documents.remove(&document_id);
        self.project.refresh_databases();

        if self.project.disk_sources.contains_key(&document_id) {
            vec![
                self.project
                    .publish_document_diagnostics(&uri, &document_id),
            ]
        } else {
            vec![publish_diagnostics_notification(&uri, Vec::new(), None)]
        }
    }

    fn register_watched_files_after_initialized(&mut self) -> Vec<Message> {
        if self.client_supports_watched_file_registration
            && self.watch_files_enabled
            && !self.watched_files_registered
            && let Some(registration) = watching::registration_request(
                self.project.config.as_ref(),
                &self.project.workspace_roots,
            )
        {
            self.watched_files_registered = true;
            return vec![registration];
        }
        Vec::new()
    }

    fn publish_workspace_diagnostics(&mut self) -> Vec<Message> {
        let has_open_documents = !self.project.open_documents.is_empty();
        let messages = self.project.publish_open_diagnostics();
        if has_open_documents && self.client_supports_work_done_progress {
            with_work_done_progress(messages, "Vela workspace diagnostics")
        } else {
            messages
        }
    }

    fn apply_reload_work(&mut self, work: ReloadWork) {
        match work {
            ReloadWork::WatchedFile { uri, operation, .. } => {
                let config_change = match operation {
                    ReloadOperation::Upsert => self.project.upsert_watched_file(&uri),
                    ReloadOperation::Remove => self.project.remove_watched_file(&uri),
                };
                if let Some(config_change) = config_change {
                    self.apply_config_change(config_change);
                }
            }
            ReloadWork::WorkspaceRoots { roots, .. } => {
                self.apply_config_change(ConfigChange::from_workspace_roots(roots));
            }
        }
    }
}

#[cfg(test)]
mod tests;
