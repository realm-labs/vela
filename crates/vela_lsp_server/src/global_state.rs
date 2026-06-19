use std::collections::BTreeSet;

use crossbeam_channel::Sender;
use lsp_server::Message;
use lsp_types::{
    DidChangeConfigurationParams, DidChangeWatchedFilesParams, DidChangeWorkspaceFoldersParams,
    FileChangeType,
};
use vela_language_service::WorkspaceRoot;

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
    rpc::request_id_from_lsp,
    success_response,
    transport::{ResultSummary, messages_from_result},
    with_work_done_progress,
};

pub(crate) struct GlobalState {
    sender: Sender<Message>,
    launch_configuration: LaunchConfiguration,
    request_queue: RequestQueue,
    server: LspServer,
}

impl GlobalState {
    pub(crate) fn new(sender: Sender<Message>, launch_configuration: LaunchConfiguration) -> Self {
        let server = LspServer::with_launch_configuration(launch_configuration.clone());
        Self {
            sender,
            launch_configuration,
            request_queue: RequestQueue::default(),
            server,
        }
    }

    pub(crate) const fn launch_configuration(&self) -> &LaunchConfiguration {
        &self.launch_configuration
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
        self.server.is_exited()
    }

    pub(crate) const fn is_initialized(&self) -> bool {
        self.server.is_initialized()
    }

    pub(crate) const fn is_shutdown_requested(&self) -> bool {
        self.server.is_shutdown_requested()
    }

    pub(crate) fn take_cancelled_request(&mut self, id: &RequestId) -> bool {
        self.server.take_cancelled_request(id)
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
        if self.server.initialized {
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
                    ErrorCode::InvalidRequest,
                    format!("invalid initialize params: {error}"),
                ));
            }
        };

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
        self.server.shutdown_lsp(id, params)
    }

    pub(crate) fn initialized(&mut self, params: lsp_types::InitializedParams) -> JsonRpcResult {
        self.server.initialized_lsp(params)
    }

    pub(crate) fn exit(&mut self, params: ()) -> JsonRpcResult {
        self.server.exit_lsp(params)
    }

    pub(crate) fn cancel_request(&mut self, params: lsp_types::CancelParams) -> JsonRpcResult {
        self.server.cancel_request_lsp(params)
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
        self.apply_config_change(ConfigChange::from_workspace_roots(workspace_roots));
        self.publish_workspace_diagnostics()
    }

    pub(crate) fn did_change_watched_files(
        &mut self,
        params: DidChangeWatchedFilesParams,
    ) -> JsonRpcResult {
        for change in coalesced_watched_file_changes(params.changes) {
            let uri = change.uri.to_string();
            let config_change = if change.typ == FileChangeType::DELETED {
                self.server.remove_watched_file(&uri)
            } else {
                self.server.upsert_watched_file(&uri)
            };
            if let Some(config_change) = config_change {
                self.apply_config_change(config_change);
            }
        }
        self.publish_workspace_diagnostics()
    }

    pub(crate) fn handle_legacy_json(&mut self, input: &str) -> JsonRpcResult {
        self.server.handle_json(input)
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
}

#[derive(Debug, Default)]
struct RequestQueue {
    incoming: BTreeSet<String>,
}

impl RequestQueue {
    fn request_id(message: &Message) -> Option<String> {
        match message {
            Message::Request(request) => Some(request.id.to_string()),
            Message::Response(_) | Message::Notification(_) => None,
        }
    }

    fn start(&mut self, id: String) {
        self.incoming.insert(id);
    }

    fn finish(&mut self, id: &str) {
        self.incoming.remove(id);
    }
}

fn coalesced_watched_file_changes(changes: Vec<lsp_types::FileEvent>) -> Vec<lsp_types::FileEvent> {
    let mut latest_by_uri = std::collections::BTreeMap::<String, (usize, FileChangeType)>::new();
    for (index, change) in changes.into_iter().enumerate() {
        latest_by_uri.insert(change.uri.to_string(), (index, change.typ));
    }

    let mut events = latest_by_uri
        .into_iter()
        .map(|(uri, (index, typ))| {
            (
                index,
                lsp_types::FileEvent {
                    uri: uri.parse().expect("coalesced URI should remain valid"),
                    typ,
                },
            )
        })
        .collect::<Vec<_>>();
    events.sort_by_key(|(index, _)| *index);
    events.into_iter().map(|(_, event)| event).collect()
}
