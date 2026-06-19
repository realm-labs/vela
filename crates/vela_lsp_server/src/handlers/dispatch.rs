use std::fmt::Debug;

use lsp_server::{Message, Notification, Request};
use lsp_types::{
    notification::{
        Cancel, DidChangeConfiguration, DidChangeTextDocument, DidChangeWatchedFiles,
        DidChangeWorkspaceFolders, DidCloseTextDocument, DidOpenTextDocument, DidSaveTextDocument,
        Exit, Initialized,
    },
    request::{
        CodeActionRequest, Completion, DocumentSymbolRequest, FoldingRangeRequest, Formatting,
        GotoDeclaration, GotoDefinition, GotoTypeDefinition, HoverRequest, InlayHintRequest,
        References, Rename, SelectionRangeRequest, SemanticTokensFullDeltaRequest,
        SemanticTokensFullRequest, SemanticTokensRangeRequest, SignatureHelpRequest,
    },
};
use serde::de::DeserializeOwned;

use crate::{
    ErrorCode, JsonRpcResult, RequestId, error_response, global_state::GlobalState,
    rpc::request_id_from_lsp,
};

pub(crate) fn dispatch_message(
    global_state: &mut GlobalState,
    message: &Message,
    legacy_input: &str,
) -> JsonRpcResult {
    match message {
        Message::Request(request) => dispatch_request(global_state, request.clone(), legacy_input),
        Message::Notification(notification) => {
            dispatch_notification(global_state, notification.clone(), legacy_input)
        }
        Message::Response(_) => global_state.handle_legacy_json(legacy_input),
    }
}

fn dispatch_request(
    global_state: &mut GlobalState,
    request: Request,
    legacy_input: &str,
) -> JsonRpcResult {
    let request_id = rpc_request_id(request.id.clone());
    if global_state.take_cancelled_request(&request_id) {
        return request_cancelled(request_id);
    }
    if global_state.is_shutdown_requested() && request.method != "exit" {
        return server_shut_down(request.id);
    }
    if !global_state.is_initialized() && !is_pre_initialize_method(&request.method) {
        return server_not_initialized(request.id);
    }

    let mut dispatcher = RequestDispatcher::new(global_state, request, legacy_input);
    dispatcher
        .on_sync_mut_typed::<lsp_types::request::Initialize>(GlobalState::initialize)
        .on_sync_mut_typed::<lsp_types::request::Shutdown>(GlobalState::shutdown)
        .on_sync::<DocumentSymbolRequest>()
        .on_latency_sensitive::<Completion>()
        .on_latency_sensitive::<HoverRequest>()
        .on_latency_sensitive::<SignatureHelpRequest>()
        .on_latency_sensitive::<SemanticTokensFullRequest>()
        .on_latency_sensitive::<SemanticTokensFullDeltaRequest>()
        .on_worker::<GotoDefinition>()
        .on_worker::<GotoDeclaration>()
        .on_worker::<GotoTypeDefinition>()
        .on_worker::<References>()
        .on_worker::<Rename>()
        .on_worker::<CodeActionRequest>()
        .on_worker::<FoldingRangeRequest>()
        .on_worker::<SelectionRangeRequest>()
        .on_worker::<SemanticTokensRangeRequest>()
        .on_worker::<InlayHintRequest>()
        .on_fmt_thread::<Formatting>()
        .finish()
}

fn dispatch_notification(
    global_state: &mut GlobalState,
    notification: Notification,
    legacy_input: &str,
) -> JsonRpcResult {
    let mut dispatcher = NotificationDispatcher::new(global_state, notification, legacy_input);
    dispatcher
        .on_sync_mut_typed::<Initialized>(GlobalState::initialized)
        .on_sync_mut_typed::<Exit>(GlobalState::exit)
        .on_sync_mut_typed::<Cancel>(GlobalState::cancel_request)
        .on_sync_mut_typed::<DidChangeConfiguration>(GlobalState::did_change_configuration)
        .on_sync_mut_typed::<DidChangeWorkspaceFolders>(GlobalState::did_change_workspace_folders)
        .on_sync_mut_typed::<DidChangeWatchedFiles>(GlobalState::did_change_watched_files)
        .on_sync_mut::<DidOpenTextDocument>()
        .on_sync_mut::<DidChangeTextDocument>()
        .on_sync_mut::<DidCloseTextDocument>()
        .on_sync_mut::<DidSaveTextDocument>()
        .finish()
}

pub(crate) struct RequestDispatcher<'a> {
    global_state: &'a mut GlobalState,
    request: Option<Request>,
    legacy_input: &'a str,
    result: JsonRpcResult,
}

impl<'a> RequestDispatcher<'a> {
    fn new(global_state: &'a mut GlobalState, request: Request, legacy_input: &'a str) -> Self {
        Self {
            global_state,
            request: Some(request),
            legacy_input,
            result: JsonRpcResult::None,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn on_sync_mut<R>(&mut self) -> &mut Self
    where
        R: lsp_types::request::Request,
    {
        self.dispatch_legacy::<R>();
        self
    }

    pub(crate) fn on_sync_mut_typed<R>(
        &mut self,
        f: fn(&mut GlobalState, lsp_server::RequestId, R::Params) -> JsonRpcResult,
    ) -> &mut Self
    where
        R: lsp_types::request::Request,
        R::Params: DeserializeOwned + Debug,
    {
        let Some(request) = self.take_matching::<R>() else {
            return self;
        };
        let id = request.id;
        let params = match serde_json::from_value::<R::Params>(request.params) {
            Ok(params) => params,
            Err(error) => {
                self.result = invalid_params(id, R::METHOD, error);
                return self;
            }
        };
        self.result = f(self.global_state, id, params);
        self
    }

    pub(crate) fn on_sync<R>(&mut self) -> &mut Self
    where
        R: lsp_types::request::Request,
    {
        self.dispatch_legacy::<R>();
        self
    }

    pub(crate) fn on_latency_sensitive<R>(&mut self) -> &mut Self
    where
        R: lsp_types::request::Request,
    {
        self.dispatch_legacy::<R>();
        self
    }

    pub(crate) fn on_worker<R>(&mut self) -> &mut Self
    where
        R: lsp_types::request::Request,
    {
        self.dispatch_legacy::<R>();
        self
    }

    pub(crate) fn on_fmt_thread<R>(&mut self) -> &mut Self
    where
        R: lsp_types::request::Request,
    {
        self.dispatch_legacy::<R>();
        self
    }

    pub(crate) fn finish(&mut self) -> JsonRpcResult {
        if let Some(request) = self.request.take() {
            self.result = if is_known_notification_method(&request.method) {
                self.global_state.handle_legacy_json(self.legacy_input)
            } else {
                method_not_found(request.id, &request.method)
            };
        }
        std::mem::replace(&mut self.result, JsonRpcResult::None)
    }

    fn dispatch_legacy<R>(&mut self)
    where
        R: lsp_types::request::Request,
    {
        if self.take_matching::<R>().is_some() {
            self.result = self.global_state.handle_legacy_json(self.legacy_input);
        }
    }

    fn take_matching<R>(&mut self) -> Option<Request>
    where
        R: lsp_types::request::Request,
    {
        if self
            .request
            .as_ref()
            .is_some_and(|request| request.method == R::METHOD)
        {
            self.request.take()
        } else {
            None
        }
    }
}

pub(crate) struct NotificationDispatcher<'a> {
    global_state: &'a mut GlobalState,
    notification: Option<Notification>,
    legacy_input: &'a str,
    result: JsonRpcResult,
}

impl<'a> NotificationDispatcher<'a> {
    fn new(
        global_state: &'a mut GlobalState,
        notification: Notification,
        legacy_input: &'a str,
    ) -> Self {
        Self {
            global_state,
            notification: Some(notification),
            legacy_input,
            result: JsonRpcResult::None,
        }
    }

    pub(crate) fn on_sync_mut<N>(&mut self) -> &mut Self
    where
        N: lsp_types::notification::Notification,
    {
        if self
            .notification
            .as_ref()
            .is_some_and(|notification| notification.method == N::METHOD)
        {
            self.result = self.global_state.handle_legacy_json(self.legacy_input);
            self.notification = None;
        }
        self
    }

    pub(crate) fn on_sync_mut_typed<N>(
        &mut self,
        f: fn(&mut GlobalState, N::Params) -> JsonRpcResult,
    ) -> &mut Self
    where
        N: lsp_types::notification::Notification,
        N::Params: DeserializeOwned + Debug,
    {
        let Some(notification) = self.take_matching::<N>() else {
            return self;
        };
        let params = match serde_json::from_value::<N::Params>(notification.params) {
            Ok(params) => params,
            Err(_) => {
                self.result = JsonRpcResult::None;
                return self;
            }
        };
        self.result = f(self.global_state, params);
        self
    }

    pub(crate) fn finish(&mut self) -> JsonRpcResult {
        if self.notification.take().is_some() {
            self.result = JsonRpcResult::None;
        }
        std::mem::replace(&mut self.result, JsonRpcResult::None)
    }

    fn take_matching<N>(&mut self) -> Option<Notification>
    where
        N: lsp_types::notification::Notification,
    {
        if self
            .notification
            .as_ref()
            .is_some_and(|notification| notification.method == N::METHOD)
        {
            self.notification.take()
        } else {
            None
        }
    }
}

fn method_not_found(id: lsp_server::RequestId, method: &str) -> JsonRpcResult {
    JsonRpcResult::Response(error_response(
        Some(request_id_from_lsp(id)),
        ErrorCode::MethodNotFound,
        format!("method `{method}` is not implemented"),
    ))
}

fn server_not_initialized(id: lsp_server::RequestId) -> JsonRpcResult {
    JsonRpcResult::Response(error_response(
        Some(request_id_from_lsp(id)),
        ErrorCode::ServerNotInitialized,
        "server has not been initialized",
    ))
}

fn server_shut_down(id: lsp_server::RequestId) -> JsonRpcResult {
    JsonRpcResult::Response(error_response(
        Some(request_id_from_lsp(id)),
        ErrorCode::InvalidRequest,
        "server has shut down",
    ))
}

fn request_cancelled(id: RequestId) -> JsonRpcResult {
    JsonRpcResult::Response(error_response(
        Some(id),
        ErrorCode::RequestCancelled,
        "request was cancelled before processing",
    ))
}

fn invalid_params(
    id: lsp_server::RequestId,
    method: &str,
    error: serde_json::Error,
) -> JsonRpcResult {
    JsonRpcResult::Response(error_response(
        Some(request_id_from_lsp(id)),
        ErrorCode::InvalidRequest,
        format!("invalid {method} params: {error}"),
    ))
}

fn rpc_request_id(id: lsp_server::RequestId) -> RequestId {
    request_id_from_lsp(id)
}

fn is_pre_initialize_method(method: &str) -> bool {
    matches!(
        method,
        "initialize" | "initialized" | "exit" | "$/cancelRequest"
    )
}

fn is_known_notification_method(method: &str) -> bool {
    matches!(
        method,
        "initialized"
            | "exit"
            | "$/cancelRequest"
            | "textDocument/didOpen"
            | "textDocument/didChange"
            | "textDocument/didClose"
            | "textDocument/didSave"
            | "workspace/didChangeConfiguration"
            | "workspace/didChangeWorkspaceFolders"
            | "workspace/didChangeWatchedFiles"
    )
}
