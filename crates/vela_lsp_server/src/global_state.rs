use std::collections::BTreeSet;

use crossbeam_channel::Sender;
use lsp_server::Message;
use lsp_types::{
    CallHierarchyIncomingCallsParams, CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams,
    CodeActionParams, CompletionParams, DidChangeConfigurationParams, DidChangeTextDocumentParams,
    DidChangeWatchedFilesParams, DidChangeWorkspaceFoldersParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, DocumentFormattingParams,
    DocumentHighlightParams, DocumentOnTypeFormattingParams, DocumentRangeFormattingParams,
    DocumentSymbolParams, FoldingRangeParams, HoverParams, InlayHintParams, ReferenceParams,
    RenameParams, SelectionRangeParams, SemanticTokensDeltaParams, SemanticTokensParams,
    SemanticTokensRangeParams, SignatureHelpParams, TextDocumentPositionParams,
    WorkspaceSymbolParams,
};
use vela_language_service::{
    DocumentId, LanguageServiceDatabases, LineIndex as ServiceLineIndex, WorkspaceConfig,
    WorkspaceGeneration, WorkspaceRoot, WorkspaceSnapshot,
};

use crate::lsp::{from_proto, to_proto};
use crate::{
    ErrorCode, JsonRpcResult, LaunchConfiguration, LspServer, RequestId,
    apply_lsp_document_changes,
    capabilities::initialize_result,
    completion::service_completion_resolve_payload,
    config::EditorConfiguration,
    config_change::ConfigChange,
    error_response,
    handlers::dispatch,
    lifecycle::{
        lsp_semantic_token_projection, lsp_supports_watched_file_registration,
        lsp_supports_work_done_progress, workspace_roots_from_lsp_initialize,
    },
    publish_diagnostics_notification,
    reload::{ReloadOperation, ReloadScheduler, ReloadWork},
    rpc::{request_id_from_lsp, request_id_from_lsp_number_or_string},
    semantic_tokens::SemanticTokenProjection,
    source_version, success_response,
    task::{TaskResult, TaskScheduler},
    transport::{ResultSummary, messages_from_result},
    watching, with_work_done_progress,
};

pub(crate) struct GlobalState {
    sender: Sender<Message>,
    launch_configuration: LaunchConfiguration,
    request_queue: RequestQueue,
    reload_scheduler: ReloadScheduler,
    task_scheduler: TaskScheduler,
    server: LspServer,
    workspace_snapshot: WorkspaceSnapshot,
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
    ) -> JsonRpcResult {
        let id = request_id_from_lsp(id);
        let document_id = from_proto::document_id(&params.text_document_position.text_document.uri);
        let text = snapshot_document_text(&self, &document_id);
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
        self,
        id: lsp_server::RequestId,
        params: lsp_types::CompletionItem,
    ) -> JsonRpcResult {
        let id = request_id_from_lsp(id);
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

    pub(crate) fn hover(self, id: lsp_server::RequestId, params: HoverParams) -> JsonRpcResult {
        let id = request_id_from_lsp(id);
        let document_id =
            from_proto::document_id(&params.text_document_position_params.text_document.uri);
        let text = snapshot_document_text(&self, &document_id);
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

    pub(crate) fn signature_help(
        self,
        id: lsp_server::RequestId,
        params: SignatureHelpParams,
    ) -> JsonRpcResult {
        let id = request_id_from_lsp(id);
        let document_id =
            from_proto::document_id(&params.text_document_position_params.text_document.uri);
        let text = snapshot_document_text(&self, &document_id);
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

    pub(crate) fn semantic_tokens_full(
        self,
        id: lsp_server::RequestId,
        params: SemanticTokensParams,
    ) -> JsonRpcResult {
        let id = request_id_from_lsp(id);
        let document_id = from_proto::semantic_tokens_params(&params);
        let tokens = self.databases.semantic_tokens(&document_id);

        JsonRpcResult::Response(success_response(
            id,
            serde_json::to_value(to_proto::semantic_tokens(
                &tokens,
                &self.semantic_token_projection,
            ))
            .expect("typed semanticTokens/full response should serialize"),
        ))
    }

    pub(crate) fn semantic_tokens_full_delta(
        self,
        id: lsp_server::RequestId,
        params: SemanticTokensDeltaParams,
    ) -> JsonRpcResult {
        let id = request_id_from_lsp(id);
        let input = from_proto::semantic_tokens_delta_params(&params);
        let delta = self
            .databases
            .semantic_token_delta(&input.document_id, &input.previous_result_id);

        JsonRpcResult::Response(success_response(
            id,
            serde_json::to_value(to_proto::semantic_tokens_delta(
                &delta,
                &self.semantic_token_projection,
            ))
            .expect("typed semanticTokens/full/delta response should serialize"),
        ))
    }

    pub(crate) fn semantic_tokens_range(
        self,
        id: lsp_server::RequestId,
        params: SemanticTokensRangeParams,
    ) -> JsonRpcResult {
        let id = request_id_from_lsp(id);
        let document_id = from_proto::document_id(&params.text_document.uri);
        let text = snapshot_document_text(&self, &document_id);
        let input = match from_proto::semantic_tokens_range_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid semanticTokens/range params: {error}"),
                ));
            }
        };
        let tokens = self
            .databases
            .semantic_tokens_in_range(&input.document_id, input.range);

        JsonRpcResult::Response(success_response(
            id,
            serde_json::to_value(to_proto::semantic_tokens_range(
                &tokens,
                &self.semantic_token_projection,
            ))
            .expect("typed semanticTokens/range response should serialize"),
        ))
    }

    pub(crate) fn formatting(
        self,
        id: lsp_server::RequestId,
        params: DocumentFormattingParams,
    ) -> JsonRpcResult {
        let id = request_id_from_lsp(id);
        let document_id = from_proto::document_formatting_params(&params);
        let edits = self.databases.document_formatting(&document_id);

        JsonRpcResult::Response(success_response(
            id,
            serde_json::to_value(to_proto::text_edits(&edits))
                .expect("typed formatting response should serialize"),
        ))
    }

    pub(crate) fn range_formatting(
        self,
        id: lsp_server::RequestId,
        params: DocumentRangeFormattingParams,
    ) -> JsonRpcResult {
        let id = request_id_from_lsp(id);
        let document_id = from_proto::document_id(&params.text_document.uri);
        let text = snapshot_document_text(&self, &document_id);
        let input = match from_proto::range_formatting_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid rangeFormatting params: {error}"),
                ));
            }
        };
        let edits = self
            .databases
            .range_formatting(&input.document_id, input.range);

        JsonRpcResult::Response(success_response(
            id,
            serde_json::to_value(to_proto::text_edits(&edits))
                .expect("typed rangeFormatting response should serialize"),
        ))
    }

    pub(crate) fn on_type_formatting(
        self,
        id: lsp_server::RequestId,
        params: DocumentOnTypeFormattingParams,
    ) -> JsonRpcResult {
        let id = request_id_from_lsp(id);
        let document_id = from_proto::document_id(&params.text_document_position.text_document.uri);
        let text = snapshot_document_text(&self, &document_id);
        let input = match from_proto::on_type_formatting_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid onTypeFormatting params: {error}"),
                ));
            }
        };
        let edits =
            self.databases
                .on_type_formatting(&input.document_id, input.position, &input.trigger);

        JsonRpcResult::Response(success_response(
            id,
            serde_json::to_value(to_proto::text_edits(&edits))
                .expect("typed onTypeFormatting response should serialize"),
        ))
    }

    pub(crate) fn definition(
        self,
        id: lsp_server::RequestId,
        params: lsp_types::GotoDefinitionParams,
    ) -> JsonRpcResult {
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
    ) -> JsonRpcResult {
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
    ) -> JsonRpcResult {
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
    ) -> JsonRpcResult {
        let id = request_id_from_lsp(id);
        let document_id = from_proto::document_id(&params.text_document_position.text_document.uri);
        let text = snapshot_document_text(&self, &document_id);
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

    pub(crate) fn document_highlight(
        self,
        id: lsp_server::RequestId,
        params: DocumentHighlightParams,
    ) -> JsonRpcResult {
        let id = request_id_from_lsp(id);
        let document_id =
            from_proto::document_id(&params.text_document_position_params.text_document.uri);
        let text = snapshot_document_text(&self, &document_id);
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
        self,
        id: lsp_server::RequestId,
        params: DocumentSymbolParams,
    ) -> JsonRpcResult {
        let id = request_id_from_lsp(id);
        let document_id = from_proto::document_symbol_params(&params);
        let symbols = self.databases.document_symbols(&document_id);

        JsonRpcResult::Response(success_response(
            id,
            serde_json::to_value(to_proto::document_symbols(&symbols))
                .expect("typed documentSymbol response should serialize"),
        ))
    }

    pub(crate) fn workspace_symbol(
        self,
        id: lsp_server::RequestId,
        params: WorkspaceSymbolParams,
    ) -> JsonRpcResult {
        let id = request_id_from_lsp(id);
        let symbols = self
            .databases
            .workspace_symbols(from_proto::workspace_symbol_params(&params));

        JsonRpcResult::Response(success_response(
            id,
            serde_json::to_value(to_proto::workspace_symbols(&symbols))
                .expect("typed workspace/symbol response should serialize"),
        ))
    }

    pub(crate) fn folding_range(
        self,
        id: lsp_server::RequestId,
        params: FoldingRangeParams,
    ) -> JsonRpcResult {
        let id = request_id_from_lsp(id);
        let document_id = from_proto::folding_range_params(&params);
        let ranges = self.databases.folding_ranges(&document_id);

        JsonRpcResult::Response(success_response(
            id,
            serde_json::to_value(to_proto::folding_ranges(&ranges))
                .expect("typed foldingRange response should serialize"),
        ))
    }

    pub(crate) fn selection_range(
        self,
        id: lsp_server::RequestId,
        params: SelectionRangeParams,
    ) -> JsonRpcResult {
        let id = request_id_from_lsp(id);
        let document_id = from_proto::document_id(&params.text_document.uri);
        let text = snapshot_document_text(&self, &document_id);
        let input = match from_proto::selection_range_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid selectionRange params: {error}"),
                ));
            }
        };
        let ranges = self
            .databases
            .selection_ranges(&input.document_id, &input.positions);

        JsonRpcResult::Response(success_response(
            id,
            serde_json::to_value(to_proto::selection_ranges(&ranges))
                .expect("typed selectionRange response should serialize"),
        ))
    }

    pub(crate) fn prepare_rename(
        self,
        id: lsp_server::RequestId,
        params: TextDocumentPositionParams,
    ) -> JsonRpcResult {
        let id = request_id_from_lsp(id);
        let document_id = from_proto::document_id(&params.text_document.uri);
        let text = snapshot_document_text(&self, &document_id);
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

    pub(crate) fn rename(self, id: lsp_server::RequestId, params: RenameParams) -> JsonRpcResult {
        let id = request_id_from_lsp(id);
        let document_id = from_proto::document_id(&params.text_document_position.text_document.uri);
        let text = snapshot_document_text(&self, &document_id);
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

    fn navigation_location(
        self,
        id: lsp_server::RequestId,
        params: lsp_types::GotoDefinitionParams,
        method_name: &'static str,
        query: SnapshotNavigationLocationQuery,
    ) -> JsonRpcResult {
        let id = request_id_from_lsp(id);
        let document_id =
            from_proto::document_id(&params.text_document_position_params.text_document.uri);
        let text = snapshot_document_text(&self, &document_id);
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

        JsonRpcResult::Response(success_response(
            id,
            serde_json::to_value(definition.as_ref().map(to_proto::definition_location))
                .expect("typed navigation response should serialize"),
        ))
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
        let server = LspServer::with_launch_configuration(launch_configuration.clone());
        let workspace_snapshot = server.workspace.snapshot();
        let databases = server.databases.clone();
        let workspace_roots = server.workspace_roots.clone();
        let open_documents = server.open_documents.clone();
        let editor_config = server.editor_config.clone();
        let workspace_config = server.config.clone();
        Self {
            sender,
            launch_configuration,
            request_queue: RequestQueue::default(),
            reload_scheduler: ReloadScheduler::default(),
            task_scheduler: TaskScheduler::default(),
            server,
            workspace_snapshot,
            databases,
            workspace_roots,
            open_documents,
            editor_config,
            workspace_config,
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
            workspace: self.workspace_snapshot.clone(),
            databases: self.databases.clone(),
            workspace_roots: self.workspace_roots.clone(),
            open_documents: self.open_documents.clone(),
            editor_config: self.editor_config.clone(),
            workspace_config: self.workspace_config.clone(),
            client_supports_work_done_progress: self.client_supports_work_done_progress,
            client_supports_watched_file_registration: self
                .client_supports_watched_file_registration,
            semantic_token_projection: self.semantic_token_projection.clone(),
            watched_files_registered: self.watched_files_registered,
            watch_files_enabled: self.watch_files_enabled,
            generation: self.databases.generation(),
            initialized: self.initialized,
            shutdown_requested: self.shutdown_requested,
        }
    }

    pub(crate) fn handle_message(&mut self, message: &Message, input: &str) -> JsonRpcResult {
        let request_id = RequestQueue::request_id(message);
        if let Some(id) = request_id.as_ref() {
            self.request_queue.start(id.clone());
        }
        let result = dispatch::dispatch_message(self, message, input);
        if let Some(id) = request_id {
            self.request_queue.finish(&id);
        }
        result
    }

    pub(crate) fn send_result(&self, result: JsonRpcResult) -> anyhow::Result<ResultSummary> {
        let summary = ResultSummary::from_result(&result);
        for message in messages_from_result(result)? {
            self.sender.send(message)?;
        }
        Ok(summary)
    }

    pub(crate) fn send_task_result(&self, result: TaskResult) -> anyhow::Result<ResultSummary> {
        let _lane = result.lane();
        self.send_result(result.into_result())
    }

    pub(crate) const fn task_scheduler(&self) -> &TaskScheduler {
        &self.task_scheduler
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
        self.server.apply_config_change(change);
        self.sync_workspace_analysis_from_legacy_server();
        self.workspace_roots = self.server.workspace_roots.clone();
        self.editor_config = self.server.editor_config.clone();
        self.workspace_config = self.server.config.clone();
        if let Some(enabled) = watch_files_enabled {
            self.watch_files_enabled = enabled;
        }
    }

    pub(crate) fn initialize(
        &mut self,
        id: lsp_server::RequestId,
        params: lsp_types::InitializeParams,
    ) -> JsonRpcResult {
        let id = request_id_from_lsp(id);
        if self.initialized {
            return JsonRpcResult::Response(error_response(
                Some(id),
                ErrorCode::InvalidRequest,
                "server is already initialized",
            ));
        }

        let editor_config = match params
            .initialization_options
            .clone()
            .map(serde_json::from_value)
            .transpose()
        {
            Ok(editor_config) => editor_config,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidParams,
                    format!("invalid initialize params: {error}"),
                ));
            }
        };

        self.initialized = true;
        self.server.initialized = true;
        self.apply_config_change(ConfigChange::from_initialize(
            workspace_roots_from_lsp_initialize(&params),
            editor_config,
        ));
        self.client_supports_work_done_progress = lsp_supports_work_done_progress(&params);
        self.client_supports_watched_file_registration =
            lsp_supports_watched_file_registration(&params);
        self.semantic_token_projection = lsp_semantic_token_projection(&params);
        self.sync_client_capabilities_to_legacy_server();
        JsonRpcResult::Response(success_response(
            id,
            initialize_result(&self.semantic_token_projection),
        ))
    }

    pub(crate) fn shutdown(&mut self, id: lsp_server::RequestId, _params: ()) -> JsonRpcResult {
        let id = request_id_from_lsp(id);
        self.shutdown_requested = true;
        self.server.shutdown_requested = true;
        JsonRpcResult::Response(success_response(id, serde_json::Value::Null))
    }

    pub(crate) fn initialized(&mut self, _params: lsp_types::InitializedParams) -> JsonRpcResult {
        self.register_watched_files_after_initialized()
    }

    pub(crate) fn exit(&mut self, _params: ()) -> JsonRpcResult {
        self.exited = true;
        self.server.exited = true;
        JsonRpcResult::None
    }

    pub(crate) fn cancel_request(&mut self, params: lsp_types::CancelParams) -> JsonRpcResult {
        self.request_queue
            .cancel(request_id_from_lsp_number_or_string(params.id));
        JsonRpcResult::None
    }

    pub(crate) fn code_action(
        &mut self,
        id: lsp_server::RequestId,
        params: CodeActionParams,
    ) -> JsonRpcResult {
        let id = request_id_from_lsp(id);
        let result = self.server.code_action_typed(id, params);
        self.sync_workspace_analysis_from_legacy_server();
        result
    }

    pub(crate) fn inlay_hint(
        &mut self,
        id: lsp_server::RequestId,
        params: InlayHintParams,
    ) -> JsonRpcResult {
        let id = request_id_from_lsp(id);
        let result = self.server.inlay_hint_typed(id, params);
        self.sync_workspace_analysis_from_legacy_server();
        result
    }

    pub(crate) fn prepare_call_hierarchy(
        &mut self,
        id: lsp_server::RequestId,
        params: CallHierarchyPrepareParams,
    ) -> JsonRpcResult {
        let id = request_id_from_lsp(id);
        let result = self.server.prepare_call_hierarchy_typed(id, params);
        self.sync_workspace_analysis_from_legacy_server();
        result
    }

    pub(crate) fn incoming_calls(
        &mut self,
        id: lsp_server::RequestId,
        params: CallHierarchyIncomingCallsParams,
    ) -> JsonRpcResult {
        let id = request_id_from_lsp(id);
        let result = self.server.incoming_calls_typed(id, params);
        self.sync_workspace_analysis_from_legacy_server();
        result
    }

    pub(crate) fn outgoing_calls(
        &mut self,
        id: lsp_server::RequestId,
        params: CallHierarchyOutgoingCallsParams,
    ) -> JsonRpcResult {
        let id = request_id_from_lsp(id);
        let result = self.server.outgoing_calls_typed(id, params);
        self.sync_workspace_analysis_from_legacy_server();
        result
    }

    pub(crate) fn did_change_configuration(
        &mut self,
        params: DidChangeConfigurationParams,
    ) -> JsonRpcResult {
        let editor_config = match EditorConfiguration::from_settings(params.settings) {
            Ok(config) => config,
            Err(error) => {
                return JsonRpcResult::Notification(publish_diagnostics_notification(
                    "",
                    Vec::new(),
                    Some(format!("invalid didChangeConfiguration settings: {error}")),
                ));
            }
        };

        self.apply_config_change(ConfigChange::from_editor_settings(editor_config));
        let result = self.server.publish_open_diagnostics();
        self.sync_workspace_analysis_from_legacy_server();
        result
    }

    pub(crate) fn did_change_workspace_folders(
        &mut self,
        params: DidChangeWorkspaceFoldersParams,
    ) -> JsonRpcResult {
        let mut workspace_roots = self.workspace_roots.clone();
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
        self.sync_workspace_analysis_from_legacy_server();
        self.publish_workspace_diagnostics()
    }

    pub(crate) fn did_change_watched_files(
        &mut self,
        params: DidChangeWatchedFilesParams,
    ) -> JsonRpcResult {
        let schema_path = self.schema_path().map(str::to_owned);
        self.reload_scheduler.schedule_watched_files(
            params.changes,
            schema_path.as_deref(),
            &self.open_documents,
        );
        for work in self.reload_scheduler.drain() {
            self.apply_reload_work(work);
        }
        self.publish_workspace_diagnostics()
    }

    pub(crate) fn did_save(&mut self, _params: DidSaveTextDocumentParams) -> JsonRpcResult {
        JsonRpcResult::None
    }

    pub(crate) fn did_open(&mut self, params: DidOpenTextDocumentParams) -> JsonRpcResult {
        let uri = params.text_document.uri.to_string();
        let document_id = DocumentId::from(uri.clone());
        let version = source_version(params.text_document.version);
        self.server.workspace.open_document(
            document_id.clone(),
            params.text_document.text,
            version,
        );
        self.server.open_documents.insert(document_id.clone());
        self.open_documents.insert(document_id.clone());

        let result = self.server.publish_current_diagnostics(&uri, &document_id);
        self.sync_workspace_analysis_from_legacy_server();
        result
    }

    pub(crate) fn did_change(&mut self, params: DidChangeTextDocumentParams) -> JsonRpcResult {
        if params.content_changes.is_empty() {
            return JsonRpcResult::Notification(publish_diagnostics_notification(
                params.text_document.uri.as_str(),
                Vec::new(),
                Some("didChange requires at least one content change".to_owned()),
            ));
        }

        let uri = params.text_document.uri.to_string();
        let document_id = DocumentId::from(uri.clone());
        let version = source_version(params.text_document.version);
        let current_text = self
            .server
            .workspace
            .document_text(&document_id)
            .map(std::borrow::ToOwned::to_owned);
        let changes = params.content_changes;
        let text = match apply_lsp_document_changes(current_text.as_deref(), changes) {
            Ok(text) => text,
            Err(error) => {
                return JsonRpcResult::Notification(publish_diagnostics_notification(
                    &uri,
                    Vec::new(),
                    Some(error),
                ));
            }
        };

        self.server
            .workspace
            .change_document(document_id.clone(), text, version);
        self.server.open_documents.insert(document_id.clone());
        self.open_documents.insert(document_id.clone());

        let result = self.server.publish_current_diagnostics(&uri, &document_id);
        self.sync_workspace_analysis_from_legacy_server();
        result
    }

    pub(crate) fn did_close(&mut self, params: DidCloseTextDocumentParams) -> JsonRpcResult {
        let uri = params.text_document.uri.to_string();
        let document_id = DocumentId::from(uri.clone());
        self.server.workspace.close_document(&document_id);
        self.server.open_documents.remove(&document_id);
        self.open_documents.remove(&document_id);

        let result = if self.server.disk_sources.contains_key(&document_id) {
            self.server.publish_current_diagnostics(&uri, &document_id)
        } else {
            JsonRpcResult::Notification(publish_diagnostics_notification(&uri, Vec::new(), None))
        };
        self.sync_workspace_analysis_from_legacy_server();
        result
    }

    pub(crate) fn handle_legacy_json(&mut self, input: &str) -> JsonRpcResult {
        let result = self.server.handle_json(input);
        self.sync_from_legacy_server();
        result
    }

    fn sync_from_legacy_server(&mut self) {
        self.initialized |= self.server.initialized;
        self.shutdown_requested |= self.server.shutdown_requested;
        self.exited |= self.server.exited;
        self.client_supports_work_done_progress |= self.server.client_supports_work_done_progress;
        self.client_supports_watched_file_registration |=
            self.server.client_supports_watched_file_registration;
        self.semantic_token_projection = self.server.semantic_token_projection.clone();
        self.watched_files_registered |= self.server.watched_files_registered;
        self.watch_files_enabled = !self.server.file_watching_disabled;
        self.sync_workspace_analysis_from_legacy_server();
        self.workspace_roots = self.server.workspace_roots.clone();
        self.open_documents = self.server.open_documents.clone();
        self.editor_config = self.server.editor_config.clone();
        self.workspace_config = self.server.config.clone();
    }

    fn sync_client_capabilities_to_legacy_server(&mut self) {
        self.server.client_supports_work_done_progress = self.client_supports_work_done_progress;
        self.server.client_supports_watched_file_registration =
            self.client_supports_watched_file_registration;
        self.server.semantic_token_projection = self.semantic_token_projection.clone();
    }

    fn sync_workspace_analysis_from_legacy_server(&mut self) {
        self.workspace_snapshot = self.server.workspace.snapshot();
        self.databases = self.server.databases.clone();
    }

    fn register_watched_files_after_initialized(&mut self) -> JsonRpcResult {
        if self.client_supports_watched_file_registration
            && self.watch_files_enabled
            && !self.watched_files_registered
            && let Some(registration) = watching::registration_request(
                self.workspace_config.as_ref(),
                &self.workspace_roots,
            )
        {
            self.watched_files_registered = true;
            self.server.watched_files_registered = true;
            return JsonRpcResult::Notification(registration);
        }
        JsonRpcResult::None
    }

    fn schema_path(&self) -> Option<&str> {
        self.workspace_config
            .as_ref()
            .and_then(|config| config.schema().path())
    }

    fn publish_workspace_diagnostics(&mut self) -> JsonRpcResult {
        let has_open_documents = !self.open_documents.is_empty();
        let result = self.server.publish_open_diagnostics();
        self.sync_workspace_analysis_from_legacy_server();
        if has_open_documents && self.client_supports_work_done_progress {
            with_work_done_progress(result, "Vela workspace diagnostics")
        } else {
            result
        }
    }

    fn apply_reload_work(&mut self, work: ReloadWork) {
        match work {
            ReloadWork::WatchedFile { uri, operation, .. } => {
                let config_change = match operation {
                    ReloadOperation::Upsert => self.server.upsert_watched_file(&uri),
                    ReloadOperation::Remove => self.server.remove_watched_file(&uri),
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

#[derive(Debug, Default)]
struct RequestQueue {
    incoming: BTreeSet<RequestId>,
    cancelled: BTreeSet<RequestId>,
}

impl RequestQueue {
    fn request_id(message: &Message) -> Option<RequestId> {
        match message {
            Message::Request(request) => Some(request_id_from_lsp(request.id.clone())),
            Message::Response(_) | Message::Notification(_) => None,
        }
    }

    fn start(&mut self, id: RequestId) {
        self.incoming.insert(id);
    }

    fn finish(&mut self, id: &RequestId) {
        self.incoming.remove(id);
    }

    fn cancel(&mut self, id: RequestId) {
        self.cancelled.insert(id);
    }

    fn take_cancelled(&mut self, id: &RequestId) -> bool {
        self.cancelled.remove(id)
    }
}

#[cfg(test)]
mod tests {
    use crossbeam_channel::unbounded;
    use vela_language_service::{
        DocumentId, SchemaConfig, SourceVersion, WorkspaceConfig, WorkspaceRoot,
    };

    use super::*;

    #[test]
    fn snapshot_captures_read_only_global_state() {
        let (sender, _receiver) = unbounded();
        let mut launch_configuration = LaunchConfiguration::new();
        launch_configuration.add_workspace_root("/workspace/scripts");
        let mut state = GlobalState::new(sender, launch_configuration);
        let document = DocumentId::from("file:///workspace/scripts/main.vela");

        state
            .server
            .workspace_roots
            .insert("/workspace/scripts".to_owned());
        state
            .workspace_roots
            .insert("/workspace/scripts".to_owned());
        state.server.open_documents.insert(document.clone());
        state.open_documents.insert(document.clone());
        state.server.workspace.open_document(
            document.clone(),
            "fn main() { 1 }",
            SourceVersion::new(3),
        );
        state.sync_from_legacy_server();
        state.client_supports_work_done_progress = true;
        state.client_supports_watched_file_registration = true;
        state.editor_config = Some(
            EditorConfiguration::from_settings(serde_json::json!({
                "workspace": {
                    "roots": ["/workspace/scripts"]
                }
            }))
            .expect("editor config should deserialize"),
        );
        state.workspace_config = Some(workspace_config_with_schema(
            "/workspace/scripts",
            "/workspace/target/vela/schema.json",
        ));
        state.semantic_token_projection = SemanticTokenProjection::for_client(
            Some(&["type".to_owned(), "function".to_owned()]),
            Some(&["declaration".to_owned()]),
        );
        state.watched_files_registered = true;
        state.watch_files_enabled = false;
        state.initialized = true;
        state.server.initialized = true;

        let snapshot = state.snapshot();
        state.server.workspace.change_document(
            document.clone(),
            "fn main() { 2 }",
            SourceVersion::new(4),
        );
        state.server.open_documents.clear();
        state.open_documents.clear();
        state.editor_config = None;
        state.workspace_config = None;
        state.client_supports_work_done_progress = false;
        state.client_supports_watched_file_registration = false;
        state.semantic_token_projection = SemanticTokenProjection::default();
        state.watched_files_registered = false;
        state.watch_files_enabled = true;
        state.server.shutdown_requested = true;

        assert_eq!(
            snapshot.launch_configuration().workspace_roots(),
            ["/workspace/scripts"]
        );
        assert_eq!(
            snapshot.workspace().document_text(&document),
            Some("fn main() { 1 }")
        );
        assert_eq!(snapshot.generation(), snapshot.databases().generation());
        assert!(snapshot.workspace_roots().contains("/workspace/scripts"));
        assert!(snapshot.open_documents().contains(&document));
        assert!(snapshot.editor_config().is_some());
        assert_eq!(
            snapshot
                .workspace_config()
                .and_then(|config| config.schema().path()),
            Some("/workspace/target/vela/schema.json")
        );
        assert!(snapshot.client_supports_work_done_progress());
        assert!(snapshot.client_supports_watched_file_registration());
        assert_ne!(
            snapshot.semantic_token_projection(),
            &SemanticTokenProjection::default()
        );
        assert!(snapshot.watched_files_registered());
        assert!(!snapshot.watch_files_enabled());
        assert!(snapshot.is_initialized());
        assert!(!snapshot.is_shutdown_requested());
    }

    #[test]
    fn snapshot_uses_global_workspace_and_database_mirrors() {
        let (sender, _receiver) = unbounded();
        let mut state = GlobalState::new(sender, LaunchConfiguration::new());
        let document = DocumentId::from("file:///workspace/scripts/main.vela");
        state.server.workspace.open_document(
            document.clone(),
            "fn main() { 1 }",
            SourceVersion::new(1),
        );
        state
            .server
            .databases
            .mark_schema_missing("/schema/one.json");
        state.sync_from_legacy_server();

        state.server.workspace.change_document(
            document.clone(),
            "fn main() { 2 }",
            SourceVersion::new(2),
        );
        state.server.databases.clear_schema();

        let snapshot = state.snapshot();

        assert_eq!(
            snapshot.workspace().document_text(&document),
            Some("fn main() { 1 }")
        );
        assert!(!snapshot.databases().schema_db().diagnostics().is_empty());
    }

    #[test]
    fn client_capabilities_are_owned_by_global_state() {
        let (sender, _receiver) = unbounded();
        let mut state = GlobalState::new(sender, LaunchConfiguration::new());
        let params = lsp_types::InitializeParams {
            process_id: None,
            capabilities: serde_json::from_value(serde_json::json!({
                "window": {
                    "workDoneProgress": true
                },
                "workspace": {
                    "didChangeWatchedFiles": {
                        "dynamicRegistration": true
                    }
                },
                "textDocument": {
                    "semanticTokens": {
                        "dynamicRegistration": false,
                        "requests": {
                            "range": true,
                            "full": {
                                "delta": true
                            }
                        },
                        "tokenTypes": ["type", "function"],
                        "tokenModifiers": ["declaration"],
                        "formats": ["relative"]
                    }
                }
            }))
            .expect("client capabilities should deserialize"),
            ..lsp_types::InitializeParams::default()
        };
        let expected_projection = lsp_semantic_token_projection(&params);

        let initialize = state.initialize(lsp_server::RequestId::from(1), params);

        assert!(initialize.into_response().is_some());
        assert!(state.client_supports_work_done_progress);
        assert!(state.client_supports_watched_file_registration);
        assert_eq!(state.semantic_token_projection, expected_projection);
        assert_eq!(
            state.server.client_supports_work_done_progress,
            state.client_supports_work_done_progress
        );
        assert_eq!(
            state.server.client_supports_watched_file_registration,
            state.client_supports_watched_file_registration
        );
        assert_eq!(
            state.server.semantic_token_projection,
            state.semantic_token_projection
        );
    }

    #[test]
    fn typed_initialized_uses_global_watcher_capability() {
        let (sender, _receiver) = unbounded();
        let mut state = GlobalState::new(sender, LaunchConfiguration::new());
        state
            .server
            .workspace_roots
            .insert("/workspace/scripts".to_owned());
        state
            .workspace_roots
            .insert("/workspace/scripts".to_owned());
        state.client_supports_watched_file_registration = true;
        state.server.client_supports_watched_file_registration = false;

        let first = state.initialized(lsp_types::InitializedParams {});
        let second = state.initialized(lsp_types::InitializedParams {});

        let JsonRpcResult::Notification(registration) = first else {
            panic!("expected watched-file registration notification");
        };
        let registration: serde_json::Value =
            serde_json::from_str(&registration).expect("registration should be JSON");
        assert_eq!(
            registration["method"],
            serde_json::json!("client/registerCapability")
        );
        assert!(state.watched_files_registered);
        assert!(state.server.watched_files_registered);
        assert_eq!(second, JsonRpcResult::None);
    }

    #[test]
    fn typed_initialized_uses_global_watch_setting() {
        let (sender, _receiver) = unbounded();
        let mut launch_configuration = LaunchConfiguration::new();
        launch_configuration.set_watch_files_enabled(false);
        let mut state = GlobalState::new(sender, launch_configuration);
        state
            .server
            .workspace_roots
            .insert("/workspace/scripts".to_owned());
        state
            .workspace_roots
            .insert("/workspace/scripts".to_owned());
        state.client_supports_watched_file_registration = true;
        state.server.file_watching_disabled = false;

        let result = state.initialized(lsp_types::InitializedParams {});

        assert_eq!(result, JsonRpcResult::None);
        assert!(!state.watch_files_enabled);
        assert!(!state.watched_files_registered);
        assert!(!state.server.watched_files_registered);
    }

    #[test]
    fn typed_initialized_uses_global_workspace_config() {
        let (sender, _receiver) = unbounded();
        let mut state = GlobalState::new(sender, LaunchConfiguration::new());
        state.workspace_config = Some(workspace_config_with_schema(
            "/workspace/scripts",
            "/workspace/target/vela/schema.json",
        ));
        state.server.config = None;
        state.client_supports_watched_file_registration = true;

        let result = state.initialized(lsp_types::InitializedParams {});

        let JsonRpcResult::Notification(registration) = result else {
            panic!("expected watched-file registration notification");
        };
        let registration: serde_json::Value =
            serde_json::from_str(&registration).expect("registration should be JSON");
        let watchers = registration["params"]["registrations"][0]["registerOptions"]["watchers"]
            .as_array()
            .expect("watchers should be an array");
        assert!(watchers.iter().any(|watcher| {
            watcher["globPattern"] == serde_json::json!("/workspace/target/vela/schema.json")
        }));
        assert!(state.watched_files_registered);
    }

    #[test]
    fn typed_workspace_folder_changes_use_global_roots() {
        let (sender, _receiver) = unbounded();
        let mut state = GlobalState::new(sender, LaunchConfiguration::new());
        state
            .workspace_roots
            .insert("/workspace/scripts".to_owned());
        state
            .server
            .workspace_roots
            .insert("/legacy/only".to_owned());

        let result =
            state.did_change_workspace_folders(lsp_types::DidChangeWorkspaceFoldersParams {
                event: lsp_types::WorkspaceFoldersChangeEvent {
                    added: vec![lsp_types::WorkspaceFolder {
                        uri: lsp_types::Url::parse("file:///workspace/tools")
                            .expect("workspace folder URI should parse"),
                        name: "tools".to_owned(),
                    }],
                    removed: vec![lsp_types::WorkspaceFolder {
                        uri: lsp_types::Url::parse("file:///workspace/scripts")
                            .expect("workspace folder URI should parse"),
                        name: "scripts".to_owned(),
                    }],
                },
            });

        assert_eq!(result, JsonRpcResult::None);
        assert!(!state.workspace_roots.contains("/workspace/scripts"));
        assert!(state.workspace_roots.contains("/workspace/tools"));
        assert!(!state.server.workspace_roots.contains("/legacy/only"));
        assert_eq!(state.server.workspace_roots, state.workspace_roots);
    }

    #[test]
    fn typed_configuration_updates_global_editor_config() {
        let (sender, _receiver) = unbounded();
        let mut state = GlobalState::new(sender, LaunchConfiguration::new());

        let result = state.did_change_configuration(lsp_types::DidChangeConfigurationParams {
            settings: serde_json::json!({
                "vela": {
                    "workspace": {
                        "roots": ["/workspace/scripts"]
                    }
                }
            }),
        });

        assert_eq!(result, JsonRpcResult::None);
        assert!(state.editor_config.is_some());
        assert!(state.workspace_config.is_some());
        assert_eq!(
            state.editor_config.is_some(),
            state.server.editor_config.is_some()
        );
        assert_eq!(
            state.workspace_config.as_ref().map(WorkspaceConfig::roots),
            state.server.config.as_ref().map(WorkspaceConfig::roots)
        );
    }

    #[test]
    fn schema_path_is_owned_by_global_workspace_config() {
        let (sender, _receiver) = unbounded();
        let mut state = GlobalState::new(sender, LaunchConfiguration::new());
        state.workspace_config = Some(workspace_config_with_schema(
            "/workspace/scripts",
            "/workspace/target/vela/schema.json",
        ));
        state.server.config = Some(workspace_config_with_schema(
            "/legacy/scripts",
            "/legacy/target/vela/schema.json",
        ));

        assert_eq!(
            state.schema_path(),
            Some("/workspace/target/vela/schema.json")
        );
    }

    #[test]
    fn typed_did_save_is_no_response_no_op() {
        let (sender, _receiver) = unbounded();
        let mut state = GlobalState::new(sender, LaunchConfiguration::new());
        let document = DocumentId::from("file:///workspace/scripts/main.vela");
        state.open_documents.insert(document.clone());
        state.server.open_documents.clear();

        let result = state.did_save(lsp_types::DidSaveTextDocumentParams {
            text_document: lsp_types::TextDocumentIdentifier {
                uri: lsp_types::Url::parse(document.as_str())
                    .expect("document URI should parse as URL"),
            },
            text: Some("fn main() {}".to_owned()),
        });

        assert_eq!(result, JsonRpcResult::None);
        assert!(state.open_documents.contains(&document));
        assert!(state.server.open_documents.is_empty());
    }

    #[test]
    fn typed_did_open_updates_global_workspace_and_diagnostics() {
        let (sender, _receiver) = unbounded();
        let mut state = GlobalState::new(sender, LaunchConfiguration::new());
        let document = DocumentId::from("file:///workspace/scripts/main.vela");

        let result = state.did_open(lsp_types::DidOpenTextDocumentParams {
            text_document: lsp_types::TextDocumentItem {
                uri: lsp_types::Url::parse(document.as_str())
                    .expect("document URI should parse as URL"),
                language_id: "vela".to_owned(),
                version: 3,
                text: "fn main() {}".to_owned(),
            },
        });

        assert!(matches!(
            result,
            JsonRpcResult::Notification(_) | JsonRpcResult::Notifications(_)
        ));
        assert!(state.open_documents.contains(&document));
        assert_eq!(state.open_documents, state.server.open_documents);
        assert_eq!(
            state.snapshot().workspace().document_text(&document),
            Some("fn main() {}")
        );
        assert_eq!(
            state.snapshot().generation(),
            state.snapshot().databases().generation()
        );
    }

    #[test]
    fn typed_did_change_applies_incremental_edit_and_syncs_snapshot() {
        let (sender, _receiver) = unbounded();
        let mut state = GlobalState::new(sender, LaunchConfiguration::new());
        let document = DocumentId::from("file:///workspace/scripts/main.vela");
        state
            .server
            .workspace
            .open_document(document.clone(), "one\ntwo", SourceVersion::new(1));
        state.server.open_documents.insert(document.clone());
        state.sync_from_legacy_server();

        let result = state.did_change(lsp_types::DidChangeTextDocumentParams {
            text_document: lsp_types::VersionedTextDocumentIdentifier {
                uri: lsp_types::Url::parse(document.as_str())
                    .expect("document URI should parse as URL"),
                version: 2,
            },
            content_changes: vec![lsp_types::TextDocumentContentChangeEvent {
                range: Some(lsp_types::Range {
                    start: lsp_types::Position {
                        line: 1,
                        character: 0,
                    },
                    end: lsp_types::Position {
                        line: 1,
                        character: 3,
                    },
                }),
                range_length: None,
                text: "three".to_owned(),
            }],
        });

        assert!(matches!(
            result,
            JsonRpcResult::Notification(_) | JsonRpcResult::Notifications(_)
        ));
        assert_eq!(
            state.snapshot().workspace().document_text(&document),
            Some("one\nthree")
        );
        assert!(state.open_documents.contains(&document));
        assert_eq!(state.open_documents, state.server.open_documents);
    }

    #[test]
    fn typed_did_close_removes_open_overlay_and_clears_scratch_diagnostics() {
        let (sender, _receiver) = unbounded();
        let mut state = GlobalState::new(sender, LaunchConfiguration::new());
        let document = DocumentId::from("file:///workspace/scripts/main.vela");
        state.server.workspace.open_document(
            document.clone(),
            "fn main() {}",
            SourceVersion::new(1),
        );
        state.server.open_documents.insert(document.clone());
        state.sync_from_legacy_server();

        let result = state.did_close(lsp_types::DidCloseTextDocumentParams {
            text_document: lsp_types::TextDocumentIdentifier {
                uri: lsp_types::Url::parse(document.as_str())
                    .expect("document URI should parse as URL"),
            },
        });

        assert!(matches!(
            result,
            JsonRpcResult::Notification(_) | JsonRpcResult::Notifications(_)
        ));
        assert!(!state.open_documents.contains(&document));
        assert_eq!(state.open_documents, state.server.open_documents);
        assert_eq!(state.snapshot().workspace().document_text(&document), None);
    }

    #[test]
    fn legacy_document_sync_updates_global_open_documents() {
        let (sender, _receiver) = unbounded();
        let mut state = GlobalState::new(sender, LaunchConfiguration::new());
        state.initialized = true;
        state.server.initialized = true;
        let document = DocumentId::from("file:///workspace/scripts/main.vela");

        let result = state.handle_legacy_json(
            &serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": document.as_str(),
                        "languageId": "vela",
                        "version": 1,
                        "text": "fn main() {}"
                    }
                }
            })
            .to_string(),
        );

        assert!(matches!(
            result,
            JsonRpcResult::Notification(_) | JsonRpcResult::Notifications(_)
        ));
        assert!(state.open_documents.contains(&document));
        assert_eq!(state.open_documents, state.server.open_documents);
    }

    #[test]
    fn lifecycle_flags_are_owned_by_global_state() {
        let (sender, _receiver) = unbounded();
        let mut state = GlobalState::new(sender, LaunchConfiguration::new());

        let initialize = state.initialize(
            lsp_server::RequestId::from(1),
            lsp_types::InitializeParams {
                process_id: None,
                capabilities: lsp_types::ClientCapabilities::default(),
                ..lsp_types::InitializeParams::default()
            },
        );
        assert!(initialize.into_response().is_some());
        assert!(state.is_initialized());
        assert!(state.server.initialized);

        let shutdown = state.shutdown(lsp_server::RequestId::from(2), ());
        assert!(shutdown.into_response().is_some());
        assert!(state.is_shutdown_requested());
        assert!(state.server.shutdown_requested);

        let exit = state.exit(());
        assert_eq!(exit, JsonRpcResult::None);
        assert!(state.is_exited());
        assert!(state.server.exited);

        let (sender, _receiver) = unbounded();
        let mut state = GlobalState::new(sender, LaunchConfiguration::new());
        let result = state.handle_legacy_json(
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "exit",
                "params": null
            })
            .to_string(),
        );
        assert!(result.into_response().is_some());
        assert!(state.is_exited());
    }

    #[test]
    fn typed_completion_resolve_dispatch_projects_completion_item() {
        let (sender, _receiver) = unbounded();
        let mut state = GlobalState::new(sender, LaunchConfiguration::new());
        state.initialized = true;
        state.server.initialized = true;
        let request = Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(7),
            method: "completionItem/resolve".to_owned(),
            params: serde_json::to_value(lsp_types::CompletionItem {
                label: "plain".to_owned(),
                kind: Some(lsp_types::CompletionItemKind::VARIABLE),
                data: Some(serde_json::json!({ "source": "vela" })),
                ..lsp_types::CompletionItem::default()
            })
            .expect("completion item should serialize"),
        });

        let result = state.handle_message(&request, "");

        let response = result
            .into_response()
            .expect("typed completion resolve should return a response");
        let response: serde_json::Value =
            serde_json::from_str(&response).expect("response should be JSON");
        assert_eq!(response["id"], 7);
        assert_eq!(response["result"]["label"], "plain");
        assert_eq!(response["result"]["kind"], 6);
        assert!(response["result"].get("documentation").is_none());
    }

    #[test]
    fn typed_hover_dispatch_projects_hover_response() {
        let (sender, _receiver) = unbounded();
        let mut state = GlobalState::new(sender, LaunchConfiguration::new());
        state.initialized = true;
        state.server.initialized = true;
        let document = DocumentId::from("file:///workspace/scripts/main.vela");
        let text = "pub fn main(amount: i64) -> i64 { return amount }";
        state
            .server
            .workspace
            .open_document(document.clone(), text, SourceVersion::new(1));
        state.server.open_documents.insert(document.clone());
        let _ = state
            .server
            .publish_current_diagnostics(document.as_str(), &document);
        state.sync_from_legacy_server();
        let request = Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(8),
            method: "textDocument/hover".to_owned(),
            params: serde_json::to_value(lsp_types::HoverParams {
                text_document_position_params: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier {
                        uri: lsp_types::Url::parse(document.as_str())
                            .expect("document URI should parse"),
                    },
                    position: lsp_types::Position::new(
                        0,
                        u32::try_from(
                            text.rfind("amount")
                                .expect("hover fixture should contain amount use"),
                        )
                        .expect("position should fit in u32"),
                    ),
                },
                work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
            })
            .expect("hover params should serialize"),
        });

        let result = state.handle_message(&request, "");

        let response = result
            .into_response()
            .expect("typed hover should return a response");
        let response: serde_json::Value =
            serde_json::from_str(&response).expect("response should be JSON");
        assert_eq!(response["id"], 8);
        assert_eq!(response["result"]["contents"]["kind"], "markdown");
        let value = response["result"]["contents"]["value"]
            .as_str()
            .expect("hover contents should be markdown");
        assert!(value.contains("amount"), "{value}");
        assert!(value.contains("_parameter_: i64"), "{value}");
    }

    #[test]
    fn typed_signature_help_dispatch_projects_signature_response() {
        let (sender, _receiver) = unbounded();
        let mut state = GlobalState::new(sender, LaunchConfiguration::new());
        state.initialized = true;
        state.server.initialized = true;
        let document = DocumentId::from("file:///workspace/scripts/main.vela");
        let text = "pub fn grant(amount: i64, bonus: i64) -> bool { return true } pub fn main() { grant(1, 2) }";
        state
            .server
            .workspace
            .open_document(document.clone(), text, SourceVersion::new(1));
        state.server.open_documents.insert(document.clone());
        let _ = state
            .server
            .publish_current_diagnostics(document.as_str(), &document);
        state.sync_from_legacy_server();
        let request = Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(9),
            method: "textDocument/signatureHelp".to_owned(),
            params: serde_json::to_value(lsp_types::SignatureHelpParams {
                text_document_position_params: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier {
                        uri: lsp_types::Url::parse(document.as_str())
                            .expect("document URI should parse"),
                    },
                    position: lsp_types::Position::new(
                        0,
                        u32::try_from(
                            text.find("2)")
                                .expect("signature fixture should contain second argument"),
                        )
                        .expect("position should fit in u32"),
                    ),
                },
                work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
                context: None,
            })
            .expect("signatureHelp params should serialize"),
        });

        let result = state.handle_message(&request, "");

        let response = result
            .into_response()
            .expect("typed signatureHelp should return a response");
        let response: serde_json::Value =
            serde_json::from_str(&response).expect("response should be JSON");
        assert_eq!(response["id"], 9);
        assert_eq!(response["result"]["activeSignature"], 0);
        assert_eq!(response["result"]["activeParameter"], 1);
        assert_eq!(
            response["result"]["signatures"][0]["label"],
            "grant(amount: i64, bonus: i64) -> bool"
        );
        assert_eq!(
            response["result"]["signatures"][0]["parameters"][1]["label"],
            "bonus: i64"
        );
    }

    #[test]
    fn typed_navigation_dispatch_projects_location_responses() {
        let (sender, _receiver) = unbounded();
        let mut state = GlobalState::new(sender, LaunchConfiguration::new());
        state.initialized = true;
        state.server.initialized = true;
        let document = DocumentId::from("file:///workspace/scripts/main.vela");
        let text = "\
struct Inventory {
    slots: i64,
}

struct Player {
    inventory: Inventory,
}

fn grant() -> i64 { return 1 }
fn main(player: Player) { grant(); return player.inventory }";
        state
            .server
            .workspace
            .open_document(document.clone(), text, SourceVersion::new(1));
        state.server.open_documents.insert(document.clone());
        let _ = state
            .server
            .publish_current_diagnostics(document.as_str(), &document);
        state.sync_from_legacy_server();

        for (id, method) in [
            (10, "textDocument/definition"),
            (11, "textDocument/declaration"),
        ] {
            let response = typed_navigation_response(
                &mut state,
                id,
                method,
                &document,
                9,
                text.lines()
                    .nth(9)
                    .expect("main line should exist")
                    .find("grant")
                    .expect("call should contain grant"),
            );
            assert_eq!(response["result"]["uri"], document.as_str());
            assert_eq!(response["result"]["range"]["start"]["line"], 8);
            assert_eq!(response["result"]["range"]["start"]["character"], 3);
            assert_eq!(response["result"]["range"]["end"]["character"], 8);
        }

        let response = typed_navigation_response(
            &mut state,
            12,
            "textDocument/typeDefinition",
            &document,
            9,
            text.lines()
                .nth(9)
                .expect("main line should exist")
                .rfind("inventory")
                .expect("field use should contain inventory"),
        );
        assert_eq!(response["result"]["uri"], document.as_str());
        assert_eq!(response["result"]["range"]["start"]["line"], 0);
        assert_eq!(response["result"]["range"]["start"]["character"], 7);
        assert_eq!(response["result"]["range"]["end"]["character"], 16);
    }

    #[test]
    fn typed_references_dispatch_projects_location_array() {
        let (sender, _receiver) = unbounded();
        let mut state = GlobalState::new(sender, LaunchConfiguration::new());
        state.initialized = true;
        state.server.initialized = true;
        let document = DocumentId::from("file:///workspace/scripts/main.vela");
        let text = "\
pub fn main(amount: i64) -> i64 {
    let next = amount + 1
    return next + amount
}";
        state
            .server
            .workspace
            .open_document(document.clone(), text, SourceVersion::new(1));
        state.server.open_documents.insert(document.clone());
        let _ = state
            .server
            .publish_current_diagnostics(document.as_str(), &document);
        state.sync_from_legacy_server();
        let line = text.lines().nth(2).expect("return line should exist");
        let character = line
            .find("amount")
            .expect("return line should contain amount");

        let response = typed_references_response(&mut state, 13, &document, 2, character, true);
        let references = response["result"]
            .as_array()
            .expect("references response should be an array");
        assert_eq!(references.len(), 3, "{references:?}");
        assert_eq!(references[0]["uri"], document.as_str());
        assert_eq!(references[0]["range"]["start"]["line"], 0);
        assert_eq!(references[0]["range"]["start"]["character"], 12);
        assert_eq!(references[2]["range"]["start"]["line"], 2);
        assert_eq!(references[2]["range"]["start"]["character"], 18);

        let response = typed_references_response(&mut state, 14, &document, 2, character, false);
        let references = response["result"]
            .as_array()
            .expect("references response should be an array");
        assert_eq!(references.len(), 2, "{references:?}");
        assert!(
            references
                .iter()
                .all(|reference| reference["range"]["start"]["line"] != 0)
        );
    }

    #[test]
    fn typed_document_highlight_dispatch_projects_highlights() {
        let (sender, _receiver) = unbounded();
        let mut state = GlobalState::new(sender, LaunchConfiguration::new());
        state.initialized = true;
        state.server.initialized = true;
        let document = DocumentId::from("file:///workspace/scripts/main.vela");
        let text = "\
pub fn main(amount: i64) -> i64 {
    let next = amount + 1
    return next + amount
}";
        state
            .server
            .workspace
            .open_document(document.clone(), text, SourceVersion::new(1));
        state.server.open_documents.insert(document.clone());
        let _ = state
            .server
            .publish_current_diagnostics(document.as_str(), &document);
        state.sync_from_legacy_server();
        let line = text.lines().nth(2).expect("return line should exist");
        let character = line
            .find("amount")
            .expect("return line should contain amount");

        let response = typed_document_highlight_response(&mut state, 15, &document, 2, character);
        let highlights = response["result"]
            .as_array()
            .expect("documentHighlight response should be an array");

        assert_eq!(highlights.len(), 3, "{highlights:?}");
        assert_eq!(highlights[0]["kind"], 1);
        assert_eq!(highlights[1]["kind"], 2);
        assert_eq!(highlights[0]["range"]["start"]["line"], 0);
        assert_eq!(highlights[0]["range"]["start"]["character"], 12);
        assert_eq!(highlights[2]["range"]["start"]["line"], 2);
        assert_eq!(highlights[2]["range"]["start"]["character"], 18);
    }

    #[test]
    fn typed_document_symbol_dispatch_projects_nested_symbols() {
        let (sender, _receiver) = unbounded();
        let mut state = GlobalState::new(sender, LaunchConfiguration::new());
        state.initialized = true;
        state.server.initialized = true;
        let document = DocumentId::from("file:///workspace/scripts/main.vela");
        let text = "\
struct Player {
    level: i64,
}

pub fn main(player: Player) -> i64 {
    return player.level
}";
        state
            .server
            .workspace
            .open_document(document.clone(), text, SourceVersion::new(1));
        state.server.open_documents.insert(document.clone());
        let _ = state
            .server
            .publish_current_diagnostics(document.as_str(), &document);
        state.sync_from_legacy_server();

        let response = typed_document_symbol_response(&mut state, 16, &document);
        let symbols = response["result"]
            .as_array()
            .expect("documentSymbol response should be an array");

        let player = symbols
            .iter()
            .find(|symbol| symbol["name"] == "Player")
            .expect("Player symbol should project");
        assert_eq!(player["kind"], 23);
        assert!(
            player["children"]
                .as_array()
                .expect("Player should include field children")
                .iter()
                .any(|child| child["name"] == "level" && child["kind"] == 8)
        );
        assert!(
            symbols
                .iter()
                .any(|symbol| symbol["name"] == "main" && symbol["kind"] == 12)
        );
    }

    #[test]
    fn typed_workspace_symbol_dispatch_projects_symbols() {
        let (sender, _receiver) = unbounded();
        let mut launch_configuration = LaunchConfiguration::new();
        launch_configuration.add_workspace_root("/workspace/scripts");
        let mut state = GlobalState::new(sender, launch_configuration);
        state.initialized = true;
        state.server.initialized = true;
        let document = DocumentId::from("file:///workspace/scripts/game/reward.vela");
        let text = "pub fn grant() -> i64 { return 1 }";
        state
            .server
            .workspace
            .open_document(document.clone(), text, SourceVersion::new(1));
        state.server.open_documents.insert(document.clone());
        let _ = state
            .server
            .publish_current_diagnostics(document.as_str(), &document);
        state.sync_from_legacy_server();

        let response = typed_workspace_symbol_response(&mut state, 17, "reward.vela");
        let symbols = response["result"]
            .as_array()
            .expect("workspaceSymbol response should be an array");
        let reward = symbols
            .iter()
            .find(|symbol| symbol["name"] == "reward.vela")
            .expect("file symbol should project");

        assert_eq!(reward["kind"], 1);
        assert_eq!(reward["data"]["detail"], "game::reward");
        assert_eq!(reward["location"]["uri"], document.as_str());
    }

    #[test]
    fn typed_folding_range_dispatch_projects_ranges() {
        let (sender, _receiver) = unbounded();
        let mut state = GlobalState::new(sender, LaunchConfiguration::new());
        state.initialized = true;
        state.server.initialized = true;
        let document = DocumentId::from("file:///workspace/scripts/main.vela");
        let text = "\
use game::reward
use game::player

pub fn main() {
    if true {
        return
    }
}";
        state
            .server
            .workspace
            .open_document(document.clone(), text, SourceVersion::new(1));
        state.server.open_documents.insert(document.clone());
        let _ = state
            .server
            .publish_current_diagnostics(document.as_str(), &document);
        state.sync_from_legacy_server();

        let response = typed_folding_range_response(&mut state, 18, &document);
        let ranges = response["result"]
            .as_array()
            .expect("foldingRange response should be an array");

        assert!(ranges.iter().any(|range| {
            range["kind"] == "imports" && range["startLine"] == 0 && range["endLine"] == 1
        }));
        assert!(ranges.iter().any(|range| {
            range["kind"] == "region" && range["startLine"] == 3 && range["endLine"] == 7
        }));
    }

    #[test]
    fn typed_selection_range_dispatch_projects_parent_chain() {
        let (sender, _receiver) = unbounded();
        let mut state = GlobalState::new(sender, LaunchConfiguration::new());
        state.initialized = true;
        state.server.initialized = true;
        let document = DocumentId::from("file:///workspace/scripts/main.vela");
        let text = "\
pub fn main(player: Player) -> i64 {
    let next = player.level + 1
    return next
}";
        state
            .server
            .workspace
            .open_document(document.clone(), text, SourceVersion::new(1));
        state.server.open_documents.insert(document.clone());
        let _ = state
            .server
            .publish_current_diagnostics(document.as_str(), &document);
        state.sync_from_legacy_server();

        let response = typed_selection_range_response(&mut state, 19, &document, 1, 22);
        let ranges = response["result"]
            .as_array()
            .expect("selectionRange response should be an array");
        assert_eq!(ranges.len(), 1);
        let chain = json_selection_chain(&ranges[0]);

        assert!(chain.iter().any(|range| {
            range["start"]["line"] == 1
                && range["start"]["character"] == 22
                && range["end"]["character"] == 27
        }));
        assert!(chain.iter().any(|range| {
            range["start"]["line"] == 1
                && range["start"]["character"] == 15
                && range["end"]["character"] == 27
        }));
    }

    #[test]
    fn typed_semantic_token_dispatch_projects_full_delta_and_range() {
        let (sender, _receiver) = unbounded();
        let mut state = GlobalState::new(sender, LaunchConfiguration::new());
        state.initialized = true;
        state.server.initialized = true;
        let document = DocumentId::from("file:///workspace/scripts/main.vela");
        let text = "pub fn main() { let value = 1 return value }";
        state
            .server
            .workspace
            .open_document(document.clone(), text, SourceVersion::new(1));
        state.server.open_documents.insert(document.clone());
        state.sync_from_legacy_server();

        let full_response = typed_semantic_tokens_full_response(&mut state, 20, &document);
        let full_data = full_response["result"]["data"]
            .as_array()
            .expect("semanticTokens/full response should include data");
        assert!(!full_data.is_empty());
        let result_id = full_response["result"]["resultId"]
            .as_str()
            .expect("semanticTokens/full response should include resultId")
            .to_owned();

        let delta_response =
            typed_semantic_tokens_delta_response(&mut state, 21, &document, &result_id);
        assert_eq!(delta_response["result"]["edits"], serde_json::json!([]));

        let range_response = typed_semantic_tokens_range_response(&mut state, 22, &document);
        let range_data = range_response["result"]["data"]
            .as_array()
            .expect("semanticTokens/range response should include data");
        assert!(!range_data.is_empty());
    }

    #[test]
    fn typed_code_action_dispatch_projects_quickfix_edits() {
        let (sender, _receiver) = unbounded();
        let mut state = GlobalState::new(sender, LaunchConfiguration::new());
        state.initialized = true;
        state.server.initialized = true;
        let document = DocumentId::from("file:///workspace/scripts/main.vela");
        let text = "pub fn main(scores: Array<i64>) { return scores.frist() }";
        state
            .server
            .workspace
            .open_document(document.clone(), text, SourceVersion::new(1));
        state.server.open_documents.insert(document.clone());
        state.sync_from_legacy_server();
        let typo_start = text.find("frist").expect("fixture should contain typo");

        let response = typed_code_action_response(
            &mut state,
            23,
            &document,
            u32::try_from(typo_start).expect("position should fit in u32"),
            u32::try_from(typo_start + "frist".len()).expect("position should fit in u32"),
        );
        let actions = response["result"]
            .as_array()
            .expect("codeAction response should be an array");
        let action = actions
            .iter()
            .find(|action| action["title"] == "Replace with `first`")
            .expect("quickfix should project");

        assert_eq!(action["kind"], "quickfix");
        assert_eq!(
            action["edit"]["changes"][document.as_str()][0]["newText"],
            "first"
        );
    }

    #[test]
    fn typed_inlay_hint_dispatch_projects_parameter_hints() {
        let (sender, _receiver) = unbounded();
        let mut state = GlobalState::new(sender, LaunchConfiguration::new());
        state.initialized = true;
        state.server.initialized = true;
        let document = DocumentId::from("file:///workspace/scripts/main.vela");
        let text = "pub fn grant(amount: i64, reason: String) -> i64 { return amount }\npub fn main() { return grant(10, \"quest\") }";
        state
            .server
            .workspace
            .open_document(document.clone(), text, SourceVersion::new(1));
        state.server.open_documents.insert(document.clone());
        state.sync_from_legacy_server();

        let response = typed_inlay_hint_response(&mut state, 24, &document);
        let hints = response["result"]
            .as_array()
            .expect("inlayHint response should be an array");

        assert_eq!(hints.len(), 2);
        assert_eq!(hints[0]["label"], "amount:");
        assert_eq!(hints[0]["kind"], 2);
        assert_eq!(hints[0]["paddingRight"], true);
        assert_eq!(hints[1]["label"], "reason:");
    }

    #[test]
    fn typed_formatting_dispatch_projects_text_edits() {
        let (sender, _receiver) = unbounded();
        let mut state = GlobalState::new(sender, LaunchConfiguration::new());
        state.initialized = true;
        state.server.initialized = true;
        let document = DocumentId::from("file:///workspace/scripts/main.vela");
        let text = "pub fn main() {   \n    return 1   \n}\n";
        state
            .server
            .workspace
            .open_document(document.clone(), text, SourceVersion::new(1));
        state.server.open_documents.insert(document.clone());
        let _ = state
            .server
            .publish_current_diagnostics(document.as_str(), &document);
        state.sync_from_legacy_server();

        let document_response = typed_formatting_response(&mut state, 20, &document);
        let document_edits = document_response["result"]
            .as_array()
            .expect("formatting response should be an array");
        assert_eq!(document_edits.len(), 1);
        assert_eq!(
            document_edits[0]["newText"],
            "pub fn main() {\n    return 1\n}\n"
        );

        let range_response = typed_range_formatting_response(&mut state, 21, &document);
        let range_edits = range_response["result"]
            .as_array()
            .expect("rangeFormatting response should be an array");
        assert_eq!(range_edits.len(), 1);
        assert_eq!(range_edits[0]["range"]["start"]["line"], 1);
        assert_eq!(range_edits[0]["newText"], "");

        let on_type_response = typed_on_type_formatting_response(&mut state, 22, &document);
        let on_type_edits = on_type_response["result"]
            .as_array()
            .expect("onTypeFormatting response should be an array");
        assert_eq!(on_type_edits.len(), 1);
        assert_eq!(on_type_edits[0]["range"]["start"]["line"], 0);
        assert_eq!(
            on_type_edits[0]["newText"],
            "pub fn main() {\n    return 1\n}\n"
        );
    }

    #[test]
    fn typed_prepare_rename_dispatch_projects_placeholder_range() {
        let (sender, _receiver) = unbounded();
        let mut state = GlobalState::new(sender, LaunchConfiguration::new());
        state.initialized = true;
        state.server.initialized = true;
        let document = DocumentId::from("file:///workspace/scripts/main.vela");
        let text = "\
pub fn main(amount: i64) -> i64 {
    return amount
}";
        state
            .server
            .workspace
            .open_document(document.clone(), text, SourceVersion::new(1));
        state.server.open_documents.insert(document.clone());
        let _ = state
            .server
            .publish_current_diagnostics(document.as_str(), &document);
        state.sync_from_legacy_server();
        let line = text.lines().nth(1).expect("return line should exist");
        let character = line
            .find("amount")
            .expect("return line should contain amount");

        let response = typed_prepare_rename_response(&mut state, 15, &document, 1, character);

        assert_eq!(response["result"]["placeholder"], "amount");
        assert_eq!(response["result"]["range"]["start"]["line"], 1);
        assert_eq!(response["result"]["range"]["start"]["character"], 11);
        assert_eq!(response["result"]["range"]["end"]["character"], 17);
    }

    #[test]
    fn typed_rename_dispatch_projects_workspace_edit() {
        let (sender, _receiver) = unbounded();
        let mut state = GlobalState::new(sender, LaunchConfiguration::new());
        state.initialized = true;
        state.server.initialized = true;
        let document = DocumentId::from("file:///workspace/scripts/main.vela");
        let text = "\
pub fn main(amount: i64) -> i64 {
    return amount
}";
        state
            .server
            .workspace
            .open_document(document.clone(), text, SourceVersion::new(2));
        state.server.open_documents.insert(document.clone());
        let _ = state
            .server
            .publish_current_diagnostics(document.as_str(), &document);
        state.sync_from_legacy_server();
        let line = text.lines().nth(1).expect("return line should exist");
        let character = line
            .find("amount")
            .expect("return line should contain amount");

        let response = typed_rename_response(&mut state, 16, &document, 1, character, "total");

        let edits = response["result"]["changes"][document.as_str()]
            .as_array()
            .expect("rename changes should contain document edits");
        assert_eq!(edits.len(), 2);
        assert_eq!(edits[0]["newText"], "total");
        assert_eq!(
            response["result"]["documentChanges"][0]["textDocument"]["uri"],
            document.as_str()
        );
        assert_eq!(
            response["result"]["documentChanges"][0]["textDocument"]["version"],
            2
        );
        assert_eq!(
            response["result"]["documentChanges"][0]["edits"][0]["newText"],
            "total"
        );
    }

    #[test]
    fn typed_prepare_call_hierarchy_dispatch_projects_items() {
        let (sender, _receiver) = unbounded();
        let mut state = GlobalState::new(sender, LaunchConfiguration::new());
        state.initialized = true;
        state.server.initialized = true;
        let document = DocumentId::from("file:///workspace/scripts/main.vela");
        let text = "pub fn grant() -> i64 { return 1 }\npub fn main() { return grant() }";
        state
            .server
            .workspace
            .open_document(document.clone(), text, SourceVersion::new(1));
        state.server.open_documents.insert(document.clone());
        state.sync_from_legacy_server();
        let line = text.lines().nth(1).expect("main line should exist");
        let character = line.find("grant").expect("main line should contain grant");

        let response =
            typed_prepare_call_hierarchy_response(&mut state, 17, &document, 1, character);
        let items = response["result"]
            .as_array()
            .expect("prepareCallHierarchy response should be an array");

        assert_eq!(items.len(), 1, "{items:?}");
        assert_eq!(items[0]["name"], "grant");
        assert_eq!(items[0]["kind"], 12);
        assert_eq!(items[0]["uri"], document.as_str());
        assert_eq!(items[0]["selectionRange"]["start"]["line"], 0);
        assert_eq!(items[0]["selectionRange"]["start"]["character"], 7);
        assert!(items[0]["data"].is_object());
    }

    #[test]
    fn typed_call_hierarchy_incoming_and_outgoing_dispatch_project_calls() {
        let (sender, _receiver) = unbounded();
        let mut state = GlobalState::new(sender, LaunchConfiguration::new());
        state.initialized = true;
        state.server.initialized = true;
        let document = DocumentId::from("file:///workspace/scripts/main.vela");
        let text = "pub fn grant() -> i64 { return 1 }\npub fn main() { return grant() }";
        state
            .server
            .workspace
            .open_document(document.clone(), text, SourceVersion::new(1));
        state.server.open_documents.insert(document.clone());
        state.sync_from_legacy_server();
        let main_line = text.lines().nth(1).expect("main line should exist");
        let grant_character = main_line
            .find("grant")
            .expect("main line should contain grant");
        let main_character = main_line
            .find("main")
            .expect("main line should contain main");
        let grant_item: lsp_types::CallHierarchyItem =
            serde_json::from_value(
                typed_prepare_call_hierarchy_response(
                    &mut state,
                    18,
                    &document,
                    1,
                    grant_character,
                )["result"][0]
                    .clone(),
            )
            .expect("grant item should deserialize");
        let main_item: lsp_types::CallHierarchyItem =
            serde_json::from_value(
                typed_prepare_call_hierarchy_response(&mut state, 19, &document, 1, main_character)
                    ["result"][0]
                    .clone(),
            )
            .expect("main item should deserialize");

        let incoming = typed_incoming_calls_response(&mut state, 20, grant_item);
        let outgoing = typed_outgoing_calls_response(&mut state, 21, main_item);

        let incoming = incoming["result"]
            .as_array()
            .expect("incomingCalls response should be an array");
        assert_eq!(incoming.len(), 1, "{incoming:?}");
        assert_eq!(incoming[0]["from"]["name"], "main");
        assert_eq!(
            incoming[0]["fromRanges"]
                .as_array()
                .expect("incomingCalls should contain fromRanges")
                .len(),
            1
        );

        let outgoing = outgoing["result"]
            .as_array()
            .expect("outgoingCalls response should be an array");
        assert_eq!(outgoing.len(), 1, "{outgoing:?}");
        assert_eq!(outgoing[0]["to"]["name"], "grant");
        assert_eq!(
            outgoing[0]["fromRanges"]
                .as_array()
                .expect("outgoingCalls should contain fromRanges")
                .len(),
            1
        );
    }

    #[test]
    fn typed_cancellation_is_tracked_by_global_request_queue() {
        let (sender, _receiver) = unbounded();
        let mut state = GlobalState::new(sender, LaunchConfiguration::new());
        let request_id = RequestId::Number(7);

        let result = state.cancel_request(lsp_types::CancelParams {
            id: lsp_types::NumberOrString::Number(7),
        });

        assert_eq!(result, JsonRpcResult::None);
        assert!(state.take_cancelled_request(&request_id));
        assert!(!state.take_cancelled_request(&request_id));
        assert!(state.server.cancelled_requests.is_empty());
    }

    #[test]
    fn request_queue_tracks_typed_request_ids() {
        let mut queue = RequestQueue::default();
        let numeric = RequestId::Number(7);
        let string = RequestId::String("hover-1".to_owned());

        queue.start(numeric.clone());
        queue.start(string.clone());
        assert!(queue.incoming.contains(&numeric));
        assert!(queue.incoming.contains(&string));

        queue.finish(&numeric);
        assert!(!queue.incoming.contains(&numeric));
        assert!(queue.incoming.contains(&string));

        let message = Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from("hover-1".to_owned()),
            method: "textDocument/hover".to_owned(),
            params: serde_json::json!({}),
        });
        assert_eq!(RequestQueue::request_id(&message), Some(string));
    }

    fn typed_navigation_response(
        state: &mut GlobalState,
        id: i32,
        method: &str,
        document: &DocumentId,
        line: u32,
        character: usize,
    ) -> serde_json::Value {
        let request = Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(id),
            method: method.to_owned(),
            params: serde_json::to_value(lsp_types::GotoDefinitionParams {
                text_document_position_params: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier {
                        uri: lsp_types::Url::parse(document.as_str())
                            .expect("document URI should parse"),
                    },
                    position: lsp_types::Position::new(
                        line,
                        u32::try_from(character).expect("position should fit in u32"),
                    ),
                },
                work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
                partial_result_params: lsp_types::PartialResultParams::default(),
            })
            .expect("goto params should serialize"),
        });
        let result = state.handle_message(&request, "");
        let response = result
            .into_response()
            .expect("typed navigation should return a response");
        serde_json::from_str(&response).expect("response should be JSON")
    }

    fn typed_prepare_call_hierarchy_response(
        state: &mut GlobalState,
        id: i32,
        document: &DocumentId,
        line: u32,
        character: usize,
    ) -> serde_json::Value {
        let request = Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(id),
            method: "textDocument/prepareCallHierarchy".to_owned(),
            params: serde_json::to_value(lsp_types::CallHierarchyPrepareParams {
                text_document_position_params: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier {
                        uri: lsp_types::Url::parse(document.as_str())
                            .expect("document URI should parse"),
                    },
                    position: lsp_types::Position::new(
                        line,
                        u32::try_from(character).expect("position should fit in u32"),
                    ),
                },
                work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
            })
            .expect("prepareCallHierarchy params should serialize"),
        });
        let result = state.handle_message(&request, "");
        let response = result
            .into_response()
            .expect("typed prepareCallHierarchy should return a response");
        serde_json::from_str(&response).expect("response should be JSON")
    }

    fn typed_incoming_calls_response(
        state: &mut GlobalState,
        id: i32,
        item: lsp_types::CallHierarchyItem,
    ) -> serde_json::Value {
        let request = Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(id),
            method: "callHierarchy/incomingCalls".to_owned(),
            params: serde_json::to_value(lsp_types::CallHierarchyIncomingCallsParams {
                item,
                work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
                partial_result_params: lsp_types::PartialResultParams::default(),
            })
            .expect("incomingCalls params should serialize"),
        });
        let result = state.handle_message(&request, "");
        let response = result
            .into_response()
            .expect("typed incomingCalls should return a response");
        serde_json::from_str(&response).expect("response should be JSON")
    }

    fn typed_outgoing_calls_response(
        state: &mut GlobalState,
        id: i32,
        item: lsp_types::CallHierarchyItem,
    ) -> serde_json::Value {
        let request = Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(id),
            method: "callHierarchy/outgoingCalls".to_owned(),
            params: serde_json::to_value(lsp_types::CallHierarchyOutgoingCallsParams {
                item,
                work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
                partial_result_params: lsp_types::PartialResultParams::default(),
            })
            .expect("outgoingCalls params should serialize"),
        });
        let result = state.handle_message(&request, "");
        let response = result
            .into_response()
            .expect("typed outgoingCalls should return a response");
        serde_json::from_str(&response).expect("response should be JSON")
    }

    fn typed_rename_response(
        state: &mut GlobalState,
        id: i32,
        document: &DocumentId,
        line: u32,
        character: usize,
        new_name: &str,
    ) -> serde_json::Value {
        let request = Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(id),
            method: "textDocument/rename".to_owned(),
            params: serde_json::to_value(lsp_types::RenameParams {
                text_document_position: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier {
                        uri: lsp_types::Url::parse(document.as_str())
                            .expect("document URI should parse"),
                    },
                    position: lsp_types::Position::new(
                        line,
                        u32::try_from(character).expect("position should fit in u32"),
                    ),
                },
                new_name: new_name.to_owned(),
                work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
            })
            .expect("rename params should serialize"),
        });
        let result = state.handle_message(&request, "");
        let response = result
            .into_response()
            .expect("typed rename should return a response");
        serde_json::from_str(&response).expect("response should be JSON")
    }

    fn typed_prepare_rename_response(
        state: &mut GlobalState,
        id: i32,
        document: &DocumentId,
        line: u32,
        character: usize,
    ) -> serde_json::Value {
        let request = Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(id),
            method: "textDocument/prepareRename".to_owned(),
            params: serde_json::to_value(lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier {
                    uri: lsp_types::Url::parse(document.as_str())
                        .expect("document URI should parse"),
                },
                position: lsp_types::Position::new(
                    line,
                    u32::try_from(character).expect("position should fit in u32"),
                ),
            })
            .expect("prepareRename params should serialize"),
        });
        let result = state.handle_message(&request, "");
        let response = result
            .into_response()
            .expect("typed prepareRename should return a response");
        serde_json::from_str(&response).expect("response should be JSON")
    }

    fn typed_references_response(
        state: &mut GlobalState,
        id: i32,
        document: &DocumentId,
        line: u32,
        character: usize,
        include_declaration: bool,
    ) -> serde_json::Value {
        let request = Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(id),
            method: "textDocument/references".to_owned(),
            params: serde_json::to_value(lsp_types::ReferenceParams {
                text_document_position: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier {
                        uri: lsp_types::Url::parse(document.as_str())
                            .expect("document URI should parse"),
                    },
                    position: lsp_types::Position::new(
                        line,
                        u32::try_from(character).expect("position should fit in u32"),
                    ),
                },
                work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
                partial_result_params: lsp_types::PartialResultParams::default(),
                context: lsp_types::ReferenceContext {
                    include_declaration,
                },
            })
            .expect("reference params should serialize"),
        });
        let result = state.handle_message(&request, "");
        let response = result
            .into_response()
            .expect("typed references should return a response");
        serde_json::from_str(&response).expect("response should be JSON")
    }

    fn typed_document_highlight_response(
        state: &mut GlobalState,
        id: i32,
        document: &DocumentId,
        line: u32,
        character: usize,
    ) -> serde_json::Value {
        let request = Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(id),
            method: "textDocument/documentHighlight".to_owned(),
            params: serde_json::to_value(lsp_types::DocumentHighlightParams {
                text_document_position_params: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier {
                        uri: lsp_types::Url::parse(document.as_str())
                            .expect("document URI should parse"),
                    },
                    position: lsp_types::Position::new(
                        line,
                        u32::try_from(character).expect("position should fit in u32"),
                    ),
                },
                work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
                partial_result_params: lsp_types::PartialResultParams::default(),
            })
            .expect("documentHighlight params should serialize"),
        });
        let result = state.handle_message(&request, "");
        let response = result
            .into_response()
            .expect("typed documentHighlight should return a response");
        serde_json::from_str(&response).expect("response should be JSON")
    }

    fn typed_document_symbol_response(
        state: &mut GlobalState,
        id: i32,
        document: &DocumentId,
    ) -> serde_json::Value {
        let request = Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(id),
            method: "textDocument/documentSymbol".to_owned(),
            params: serde_json::to_value(lsp_types::DocumentSymbolParams {
                text_document: lsp_types::TextDocumentIdentifier {
                    uri: lsp_types::Url::parse(document.as_str())
                        .expect("document URI should parse"),
                },
                work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
                partial_result_params: lsp_types::PartialResultParams::default(),
            })
            .expect("documentSymbol params should serialize"),
        });
        let result = state.handle_message(&request, "");
        let response = result
            .into_response()
            .expect("typed documentSymbol should return a response");
        serde_json::from_str(&response).expect("response should be JSON")
    }

    fn typed_workspace_symbol_response(
        state: &mut GlobalState,
        id: i32,
        query: &str,
    ) -> serde_json::Value {
        let request = Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(id),
            method: "workspace/symbol".to_owned(),
            params: serde_json::to_value(lsp_types::WorkspaceSymbolParams {
                query: query.to_owned(),
                work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
                partial_result_params: lsp_types::PartialResultParams::default(),
            })
            .expect("workspaceSymbol params should serialize"),
        });
        let result = state.handle_message(&request, "");
        let response = result
            .into_response()
            .expect("typed workspaceSymbol should return a response");
        serde_json::from_str(&response).expect("response should be JSON")
    }

    fn typed_folding_range_response(
        state: &mut GlobalState,
        id: i32,
        document: &DocumentId,
    ) -> serde_json::Value {
        let request = Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(id),
            method: "textDocument/foldingRange".to_owned(),
            params: serde_json::to_value(lsp_types::FoldingRangeParams {
                text_document: lsp_types::TextDocumentIdentifier {
                    uri: lsp_types::Url::parse(document.as_str())
                        .expect("document URI should parse"),
                },
                work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
                partial_result_params: lsp_types::PartialResultParams::default(),
            })
            .expect("foldingRange params should serialize"),
        });
        let result = state.handle_message(&request, "");
        let response = result
            .into_response()
            .expect("typed foldingRange should return a response");
        serde_json::from_str(&response).expect("response should be JSON")
    }

    fn typed_selection_range_response(
        state: &mut GlobalState,
        id: i32,
        document: &DocumentId,
        line: u32,
        character: u32,
    ) -> serde_json::Value {
        let request = Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(id),
            method: "textDocument/selectionRange".to_owned(),
            params: serde_json::to_value(lsp_types::SelectionRangeParams {
                text_document: lsp_types::TextDocumentIdentifier {
                    uri: lsp_types::Url::parse(document.as_str())
                        .expect("document URI should parse"),
                },
                positions: vec![lsp_types::Position::new(line, character)],
                work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
                partial_result_params: lsp_types::PartialResultParams::default(),
            })
            .expect("selectionRange params should serialize"),
        });
        let result = state.handle_message(&request, "");
        let response = result
            .into_response()
            .expect("typed selectionRange should return a response");
        serde_json::from_str(&response).expect("response should be JSON")
    }

    fn typed_semantic_tokens_full_response(
        state: &mut GlobalState,
        id: i32,
        document: &DocumentId,
    ) -> serde_json::Value {
        let request = Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(id),
            method: "textDocument/semanticTokens/full".to_owned(),
            params: serde_json::to_value(lsp_types::SemanticTokensParams {
                text_document: lsp_types::TextDocumentIdentifier {
                    uri: lsp_types::Url::parse(document.as_str())
                        .expect("document URI should parse"),
                },
                work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
                partial_result_params: lsp_types::PartialResultParams::default(),
            })
            .expect("semanticTokens/full params should serialize"),
        });
        let result = state.handle_message(&request, "");
        let response = result
            .into_response()
            .expect("typed semanticTokens/full should return a response");
        serde_json::from_str(&response).expect("response should be JSON")
    }

    fn typed_semantic_tokens_delta_response(
        state: &mut GlobalState,
        id: i32,
        document: &DocumentId,
        previous_result_id: &str,
    ) -> serde_json::Value {
        let request = Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(id),
            method: "textDocument/semanticTokens/full/delta".to_owned(),
            params: serde_json::to_value(lsp_types::SemanticTokensDeltaParams {
                text_document: lsp_types::TextDocumentIdentifier {
                    uri: lsp_types::Url::parse(document.as_str())
                        .expect("document URI should parse"),
                },
                previous_result_id: previous_result_id.to_owned(),
                work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
                partial_result_params: lsp_types::PartialResultParams::default(),
            })
            .expect("semanticTokens/full/delta params should serialize"),
        });
        let result = state.handle_message(&request, "");
        let response = result
            .into_response()
            .expect("typed semanticTokens/full/delta should return a response");
        serde_json::from_str(&response).expect("response should be JSON")
    }

    fn typed_semantic_tokens_range_response(
        state: &mut GlobalState,
        id: i32,
        document: &DocumentId,
    ) -> serde_json::Value {
        let request = Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(id),
            method: "textDocument/semanticTokens/range".to_owned(),
            params: serde_json::to_value(lsp_types::SemanticTokensRangeParams {
                text_document: lsp_types::TextDocumentIdentifier {
                    uri: lsp_types::Url::parse(document.as_str())
                        .expect("document URI should parse"),
                },
                range: lsp_types::Range::new(
                    lsp_types::Position::new(0, 0),
                    lsp_types::Position::new(0, 42),
                ),
                work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
                partial_result_params: lsp_types::PartialResultParams::default(),
            })
            .expect("semanticTokens/range params should serialize"),
        });
        let result = state.handle_message(&request, "");
        let response = result
            .into_response()
            .expect("typed semanticTokens/range should return a response");
        serde_json::from_str(&response).expect("response should be JSON")
    }

    fn typed_code_action_response(
        state: &mut GlobalState,
        id: i32,
        document: &DocumentId,
        start_character: u32,
        end_character: u32,
    ) -> serde_json::Value {
        let request = Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(id),
            method: "textDocument/codeAction".to_owned(),
            params: serde_json::to_value(lsp_types::CodeActionParams {
                text_document: lsp_types::TextDocumentIdentifier {
                    uri: lsp_types::Url::parse(document.as_str())
                        .expect("document URI should parse"),
                },
                range: lsp_types::Range::new(
                    lsp_types::Position::new(0, start_character),
                    lsp_types::Position::new(0, end_character),
                ),
                context: lsp_types::CodeActionContext::default(),
                work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
                partial_result_params: lsp_types::PartialResultParams::default(),
            })
            .expect("codeAction params should serialize"),
        });
        let result = state.handle_message(&request, "");
        let response = result
            .into_response()
            .expect("typed codeAction should return a response");
        serde_json::from_str(&response).expect("response should be JSON")
    }

    fn typed_inlay_hint_response(
        state: &mut GlobalState,
        id: i32,
        document: &DocumentId,
    ) -> serde_json::Value {
        let request = Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(id),
            method: "textDocument/inlayHint".to_owned(),
            params: serde_json::to_value(lsp_types::InlayHintParams {
                text_document: lsp_types::TextDocumentIdentifier {
                    uri: lsp_types::Url::parse(document.as_str())
                        .expect("document URI should parse"),
                },
                range: lsp_types::Range::new(
                    lsp_types::Position::new(1, 0),
                    lsp_types::Position::new(1, 80),
                ),
                work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
            })
            .expect("inlayHint params should serialize"),
        });
        let result = state.handle_message(&request, "");
        let response = result
            .into_response()
            .expect("typed inlayHint should return a response");
        serde_json::from_str(&response).expect("response should be JSON")
    }

    fn json_selection_chain(range: &serde_json::Value) -> Vec<&serde_json::Value> {
        let mut ranges = Vec::new();
        let mut current = Some(range);
        while let Some(selection) = current {
            ranges.push(&selection["range"]);
            current = selection.get("parent");
        }
        ranges
    }

    fn typed_formatting_response(
        state: &mut GlobalState,
        id: i32,
        document: &DocumentId,
    ) -> serde_json::Value {
        let request = Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(id),
            method: "textDocument/formatting".to_owned(),
            params: serde_json::to_value(lsp_types::DocumentFormattingParams {
                text_document: lsp_types::TextDocumentIdentifier {
                    uri: lsp_types::Url::parse(document.as_str())
                        .expect("document URI should parse"),
                },
                options: lsp_formatting_options(),
                work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
            })
            .expect("formatting params should serialize"),
        });
        let result = state.handle_message(&request, "");
        let response = result
            .into_response()
            .expect("typed formatting should return a response");
        serde_json::from_str(&response).expect("response should be JSON")
    }

    fn typed_range_formatting_response(
        state: &mut GlobalState,
        id: i32,
        document: &DocumentId,
    ) -> serde_json::Value {
        let request = Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(id),
            method: "textDocument/rangeFormatting".to_owned(),
            params: serde_json::to_value(lsp_types::DocumentRangeFormattingParams {
                text_document: lsp_types::TextDocumentIdentifier {
                    uri: lsp_types::Url::parse(document.as_str())
                        .expect("document URI should parse"),
                },
                range: lsp_types::Range::new(
                    lsp_types::Position::new(1, 0),
                    lsp_types::Position::new(2, 0),
                ),
                options: lsp_formatting_options(),
                work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
            })
            .expect("rangeFormatting params should serialize"),
        });
        let result = state.handle_message(&request, "");
        let response = result
            .into_response()
            .expect("typed rangeFormatting should return a response");
        serde_json::from_str(&response).expect("response should be JSON")
    }

    fn typed_on_type_formatting_response(
        state: &mut GlobalState,
        id: i32,
        document: &DocumentId,
    ) -> serde_json::Value {
        let request = Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(id),
            method: "textDocument/onTypeFormatting".to_owned(),
            params: serde_json::to_value(lsp_types::DocumentOnTypeFormattingParams {
                text_document_position: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier {
                        uri: lsp_types::Url::parse(document.as_str())
                            .expect("document URI should parse"),
                    },
                    position: lsp_types::Position::new(2, 1),
                },
                ch: "}".to_owned(),
                options: lsp_formatting_options(),
            })
            .expect("onTypeFormatting params should serialize"),
        });
        let result = state.handle_message(&request, "");
        let response = result
            .into_response()
            .expect("typed onTypeFormatting should return a response");
        serde_json::from_str(&response).expect("response should be JSON")
    }

    fn lsp_formatting_options() -> lsp_types::FormattingOptions {
        lsp_types::FormattingOptions {
            tab_size: 4,
            insert_spaces: true,
            properties: Default::default(),
            trim_trailing_whitespace: None,
            insert_final_newline: None,
            trim_final_newlines: None,
        }
    }

    fn workspace_config_with_schema(root: &str, schema: &str) -> WorkspaceConfig {
        let mut config = WorkspaceConfig::workspace([WorkspaceRoot::from(root)]);
        config.set_schema(SchemaConfig::from_path(schema));
        config
    }
}
