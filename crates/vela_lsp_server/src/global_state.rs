use std::collections::BTreeSet;

use crossbeam_channel::Sender;
use lsp_server::Message;
use lsp_types::{
    DidChangeConfigurationParams, DidChangeWatchedFilesParams, DidChangeWorkspaceFoldersParams,
};
use vela_language_service::{
    DocumentId, LanguageServiceDatabases, WorkspaceGeneration, WorkspaceRoot, WorkspaceSnapshot,
};

use crate::{
    ErrorCode, JsonRpcResult, LaunchConfiguration, LspServer, RequestId,
    capabilities::initialize_result,
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
    success_response,
    transport::{ResultSummary, messages_from_result},
    with_work_done_progress,
};

pub(crate) struct GlobalState {
    sender: Sender<Message>,
    launch_configuration: LaunchConfiguration,
    request_queue: RequestQueue,
    reload_scheduler: ReloadScheduler,
    server: LspServer,
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

    pub(crate) const fn is_initialized(&self) -> bool {
        self.initialized
    }

    pub(crate) const fn is_shutdown_requested(&self) -> bool {
        self.shutdown_requested
    }
}

impl GlobalState {
    pub(crate) fn new(sender: Sender<Message>, launch_configuration: LaunchConfiguration) -> Self {
        let server = LspServer::with_launch_configuration(launch_configuration.clone());
        Self {
            sender,
            launch_configuration,
            request_queue: RequestQueue::default(),
            reload_scheduler: ReloadScheduler::default(),
            server,
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
            workspace: self.server.workspace.snapshot(),
            databases: self.server.databases.clone(),
            workspace_roots: self.server.workspace_roots.clone(),
            open_documents: self.server.open_documents.clone(),
            generation: self.server.databases.generation(),
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
        self.server.apply_config_change(change);
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
        self.server.client_supports_work_done_progress = lsp_supports_work_done_progress(&params);
        self.server.client_supports_watched_file_registration =
            lsp_supports_watched_file_registration(&params);
        self.server.semantic_token_projection = lsp_semantic_token_projection(&params);
        JsonRpcResult::Response(success_response(
            id,
            initialize_result(&self.server.semantic_token_projection),
        ))
    }

    pub(crate) fn shutdown(&mut self, id: lsp_server::RequestId, params: ()) -> JsonRpcResult {
        let result = self.server.shutdown_lsp(id, params);
        self.shutdown_requested = true;
        result
    }

    pub(crate) fn initialized(&mut self, params: lsp_types::InitializedParams) -> JsonRpcResult {
        self.server.initialized_lsp(params)
    }

    pub(crate) fn exit(&mut self, params: ()) -> JsonRpcResult {
        let result = self.server.exit_lsp(params);
        self.exited = true;
        result
    }

    pub(crate) fn cancel_request(&mut self, params: lsp_types::CancelParams) -> JsonRpcResult {
        self.request_queue
            .cancel(request_id_from_lsp_number_or_string(params.id));
        JsonRpcResult::None
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
        self.server.publish_open_diagnostics()
    }

    pub(crate) fn did_change_workspace_folders(
        &mut self,
        params: DidChangeWorkspaceFoldersParams,
    ) -> JsonRpcResult {
        let mut workspace_roots = self.server.workspace_roots.clone();
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
        self.publish_workspace_diagnostics()
    }

    pub(crate) fn did_change_watched_files(
        &mut self,
        params: DidChangeWatchedFilesParams,
    ) -> JsonRpcResult {
        let schema_path = self.server.schema_path().map(str::to_owned);
        self.reload_scheduler.schedule_watched_files(
            params.changes,
            schema_path.as_deref(),
            &self.server.open_documents,
        );
        for work in self.reload_scheduler.drain() {
            self.apply_reload_work(work);
        }
        self.publish_workspace_diagnostics()
    }

    pub(crate) fn handle_legacy_json(&mut self, input: &str) -> JsonRpcResult {
        let result = self.server.handle_json(input);
        self.sync_lifecycle_from_legacy_server();
        result
    }

    fn sync_lifecycle_from_legacy_server(&mut self) {
        self.initialized |= self.server.initialized;
        self.shutdown_requested |= self.server.shutdown_requested;
        self.exited |= self.server.exited;
    }

    fn publish_workspace_diagnostics(&mut self) -> JsonRpcResult {
        let has_open_documents = !self.server.open_documents.is_empty();
        let result = self.server.publish_open_diagnostics();
        if has_open_documents && self.server.client_supports_work_done_progress {
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
    use vela_language_service::{DocumentId, SourceVersion};

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
        state.server.open_documents.insert(document.clone());
        state.server.workspace.open_document(
            document.clone(),
            "fn main() { 1 }",
            SourceVersion::new(3),
        );
        state.initialized = true;
        state.server.initialized = true;

        let snapshot = state.snapshot();
        state.server.workspace.change_document(
            document.clone(),
            "fn main() { 2 }",
            SourceVersion::new(4),
        );
        state.server.open_documents.clear();
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
        assert!(snapshot.is_initialized());
        assert!(!snapshot.is_shutdown_requested());
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
}
