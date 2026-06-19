use std::collections::BTreeSet;

use lsp_types::{
    CancelParams as LspCancelParams, InitializeParams as LspInitializeParams,
    InitializedParams as LspInitializedParams, NumberOrString,
};
use serde_json::Value as JsonValue;
use vela_language_service::WorkspaceRoot;

use crate::{
    ErrorCode, JsonRpcResult, LspServer, RequestId, capabilities::initialize_result,
    client::InitializeParams, config_change::ConfigChange, error_response,
    rpc::CancelRequestParams, rpc::request_id_from_lsp, success_response, watching,
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
                    ErrorCode::InvalidParams,
                    format!("invalid initialize params: {error}"),
                ));
            }
        };
        self.initialized = true;
        self.apply_config_change(ConfigChange::from_initialize(
            workspace_roots_from_initialize(&params),
            params.initialization_options.clone(),
        ));
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
        self.register_watched_files_after_initialized()
    }

    pub(crate) fn initialized_lsp(&mut self, _params: LspInitializedParams) -> JsonRpcResult {
        self.register_watched_files_after_initialized()
    }

    fn register_watched_files_after_initialized(&mut self) -> JsonRpcResult {
        if self.client_supports_watched_file_registration
            && !self.file_watching_disabled
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

    pub(crate) fn shutdown_lsp(&mut self, id: lsp_server::RequestId, _params: ()) -> JsonRpcResult {
        let id = request_id_from_lsp(id);
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

    pub(crate) fn exit_lsp(&mut self, _params: ()) -> JsonRpcResult {
        self.exited = true;
        JsonRpcResult::None
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

    pub(crate) fn cancel_request_lsp(&mut self, params: LspCancelParams) -> JsonRpcResult {
        self.cancelled_requests
            .insert(request_id_from_lsp_number_or_string(params.id));
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

fn request_id_from_lsp_number_or_string(id: NumberOrString) -> RequestId {
    match id {
        NumberOrString::Number(id) => RequestId::Number(i64::from(id)),
        NumberOrString::String(id) => RequestId::String(id),
    }
}

pub(crate) fn workspace_roots_from_lsp_initialize(
    params: &LspInitializeParams,
) -> BTreeSet<String> {
    params
        .workspace_folders
        .iter()
        .flatten()
        .map(|folder| WorkspaceRoot::from(folder.uri.to_string()))
        .chain(
            #[allow(deprecated)]
            params
                .root_uri
                .iter()
                .map(ToString::to_string)
                .map(WorkspaceRoot::from),
        )
        .map(|root| root.path().to_owned())
        .collect()
}

pub(crate) fn lsp_supports_work_done_progress(params: &LspInitializeParams) -> bool {
    params
        .capabilities
        .window
        .as_ref()
        .and_then(|window| window.work_done_progress)
        .unwrap_or(false)
}

pub(crate) fn lsp_supports_watched_file_registration(params: &LspInitializeParams) -> bool {
    params
        .capabilities
        .workspace
        .as_ref()
        .and_then(|workspace| workspace.did_change_watched_files.as_ref())
        .and_then(|watched_files| watched_files.dynamic_registration)
        .unwrap_or(false)
}

pub(crate) fn lsp_semantic_token_projection(
    params: &LspInitializeParams,
) -> crate::semantic_tokens::SemanticTokenProjection {
    let semantic_tokens = params
        .capabilities
        .text_document
        .as_ref()
        .and_then(|text_document| text_document.semantic_tokens.as_ref());
    let token_types = semantic_tokens.map(|semantic_tokens| {
        semantic_tokens
            .token_types
            .iter()
            .map(lsp_types::SemanticTokenType::as_str)
            .map(str::to_owned)
            .collect::<Vec<_>>()
    });
    let token_modifiers = semantic_tokens.map(|semantic_tokens| {
        semantic_tokens
            .token_modifiers
            .iter()
            .map(lsp_types::SemanticTokenModifier::as_str)
            .map(str::to_owned)
            .collect::<Vec<_>>()
    });
    crate::semantic_tokens::SemanticTokenProjection::for_client(
        token_types.as_deref(),
        token_modifiers.as_deref(),
    )
}
