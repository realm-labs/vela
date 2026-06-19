use std::collections::BTreeSet;

#[cfg(test)]
use lsp_server::RequestId;
use lsp_types::InitializeParams as LspInitializeParams;
#[cfg(test)]
use serde_json::Value as JsonValue;
use vela_language_service::WorkspaceRoot;

#[cfg(test)]
use crate::{
    ErrorCode, JsonRpcResult, LspServer, capabilities::initialize_result, client::InitializeParams,
    config_change::ConfigChange, legacy_rpc::CancelRequestParams, watching,
};

#[cfg(test)]
impl LspServer {
    pub(crate) fn initialize(&mut self, id: Option<RequestId>, params: JsonValue) -> JsonRpcResult {
        let Some(id) = id else {
            return JsonRpcResult::None;
        };
        if self.initialized {
            return JsonRpcResult::error(
                Some(id),
                ErrorCode::InvalidRequest,
                "server is already initialized",
            );
        }

        let params = match serde_json::from_value::<InitializeParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return JsonRpcResult::error(
                    Some(id),
                    ErrorCode::InvalidParams,
                    format!("invalid initialize params: {error}"),
                );
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
        JsonRpcResult::ok(id, initialize_result(&self.semantic_token_projection))
    }

    pub(crate) fn initialized(&mut self, id: Option<RequestId>) -> JsonRpcResult {
        if let Some(id) = id {
            return JsonRpcResult::error(
                Some(id),
                ErrorCode::InvalidRequest,
                "`initialized` must be sent as a notification",
            );
        }
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
        JsonRpcResult::ok(id, JsonValue::Null)
    }

    pub(crate) fn exit(&mut self, id: Option<RequestId>) -> JsonRpcResult {
        self.exited = true;
        id.map_or(JsonRpcResult::None, |id| {
            JsonRpcResult::error(
                Some(id),
                ErrorCode::InvalidRequest,
                "`exit` must be sent as a notification",
            )
        })
    }

    pub(crate) fn cancel_request(
        &mut self,
        id: Option<RequestId>,
        params: JsonValue,
    ) -> JsonRpcResult {
        if let Some(id) = id {
            return JsonRpcResult::error(
                Some(id),
                ErrorCode::InvalidRequest,
                "`$/cancelRequest` must be sent as a notification",
            );
        }

        let Ok(params) = serde_json::from_value::<CancelRequestParams>(params) else {
            return JsonRpcResult::None;
        };
        let _ = params.id;
        JsonRpcResult::None
    }

    pub(crate) fn method_not_found(&self, id: Option<RequestId>, method: &str) -> JsonRpcResult {
        id.map_or(JsonRpcResult::None, |id| {
            JsonRpcResult::error(
                Some(id),
                ErrorCode::MethodNotFound,
                format!("method `{method}` is not implemented"),
            )
        })
    }
}

#[cfg(test)]
pub(crate) fn is_pre_initialize_method(method: &str) -> bool {
    matches!(
        method,
        "initialize" | "initialized" | "exit" | "$/cancelRequest"
    )
}

#[cfg(test)]
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
