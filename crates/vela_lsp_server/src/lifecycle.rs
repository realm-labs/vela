use std::collections::BTreeSet;

use serde_json::Value as JsonValue;
use vela_language_service::WorkspaceRoot;

use crate::{
    ErrorCode, JsonRpcResult, LspServer, RequestId, capabilities::initialize_result,
    client::InitializeParams, config::workspace_config_from_roots_and_editor_config,
    error_response, rpc::CancelRequestParams, success_response, watching,
};

impl LspServer {
    pub(crate) fn initialize(&mut self, id: Option<RequestId>, params: JsonValue) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        if self.initialized {
            return JsonRpcResult::Response(error_response(
                Some(id),
                ErrorCode::InvalidRequest,
                "server is already initialized",
            ));
        }

        let params = match serde_json::from_value::<InitializeParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::Response(error_response(
                    Some(id),
                    ErrorCode::InvalidRequest,
                    format!("invalid initialize params: {error}"),
                ));
            }
        };
        self.initialized = true;
        self.workspace_roots = workspace_roots_from_initialize(&params);
        if params.initialization_options.is_some() {
            self.editor_config = params.initialization_options.clone();
        }
        self.config = workspace_config_from_roots_and_editor_config(
            &self.workspace_roots,
            self.editor_config.as_ref(),
        );
        self.reload_schema_from_config();
        self.client_supports_work_done_progress = params.capabilities.supports_work_done_progress();
        self.client_supports_watched_file_registration =
            params.capabilities.supports_watched_file_registration();
        self.semantic_token_projection = params.capabilities.semantic_token_projection();
        JsonRpcResult::Response(success_response(
            id,
            initialize_result(&self.semantic_token_projection),
        ))
    }

    pub(crate) fn initialized(&mut self, id: Option<RequestId>) -> JsonRpcResult {
        if let Some(id) = id {
            return JsonRpcResult::Response(error_response(
                Some(id),
                ErrorCode::InvalidRequest,
                "`initialized` must be sent as a notification",
            ));
        }
        if self.client_supports_watched_file_registration
            && !self.watched_files_registered
            && let Some(registration) =
                watching::registration_request(self.config.as_ref(), &self.workspace_roots)
        {
            self.watched_files_registered = true;
            return JsonRpcResult::Notification(registration);
        }
        JsonRpcResult::None
    }

    pub(crate) fn shutdown(&mut self, id: Option<RequestId>) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        self.shutdown_requested = true;
        JsonRpcResult::Response(success_response(id, JsonValue::Null))
    }

    pub(crate) fn exit(&mut self, id: Option<RequestId>) -> JsonRpcResult {
        self.exited = true;
        id.map_or(JsonRpcResult::None, |id| {
            JsonRpcResult::Response(error_response(
                Some(id),
                ErrorCode::InvalidRequest,
                "`exit` must be sent as a notification",
            ))
        })
    }

    pub(crate) fn cancel_request(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        if let Some(id) = id {
            return JsonRpcResult::Response(error_response(
                Some(id),
                ErrorCode::InvalidRequest,
                "`$/cancelRequest` must be sent as a notification",
            ));
        }

        let Ok(params) = serde_json::from_value::<CancelRequestParams>(params) else {
            return JsonRpcResult::None;
        };
        self.cancelled_requests.insert(params.id);
        JsonRpcResult::None
    }

    pub(crate) fn method_not_found(&self, id: Option<RequestId>, method: &str) -> JsonRpcResult {
        id.map_or(JsonRpcResult::None, |id| {
            JsonRpcResult::Response(error_response(
                Some(id),
                ErrorCode::MethodNotFound,
                format!("method `{method}` is not implemented"),
            ))
        })
    }
}

pub(crate) fn is_pre_initialize_method(method: &str) -> bool {
    matches!(
        method,
        "initialize" | "initialized" | "exit" | "$/cancelRequest"
    )
}

fn workspace_roots_from_initialize(params: &InitializeParams) -> BTreeSet<String> {
    params
        .workspace_folders
        .iter()
        .flatten()
        .map(|folder| WorkspaceRoot::from(folder.uri.clone()))
        .chain(params.root_uri.iter().cloned().map(WorkspaceRoot::from))
        .map(|root| root.path().to_owned())
        .collect()
}
