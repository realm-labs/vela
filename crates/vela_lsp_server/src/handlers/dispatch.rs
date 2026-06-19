use std::{any::Any, fmt::Debug, panic};

use lsp_server::{Message, Notification, Request};
use lsp_types::{
    notification::{
        Cancel, DidChangeConfiguration, DidChangeTextDocument, DidChangeWatchedFiles,
        DidChangeWorkspaceFolders, DidCloseTextDocument, DidOpenTextDocument, DidSaveTextDocument,
        Exit, Initialized,
    },
    request::{
        CallHierarchyIncomingCalls, CallHierarchyOutgoingCalls, CallHierarchyPrepare,
        CodeActionRequest, Completion, DocumentHighlightRequest, DocumentSymbolRequest,
        FoldingRangeRequest, Formatting, GotoDeclaration, GotoDefinition, GotoTypeDefinition,
        HoverRequest, InlayHintRequest, OnTypeFormatting, PrepareRenameRequest, RangeFormatting,
        References, Rename, ResolveCompletionItem, SelectionRangeRequest,
        SemanticTokensFullDeltaRequest, SemanticTokensFullRequest, SemanticTokensRangeRequest,
        SignatureHelpRequest, WorkspaceSymbolRequest,
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
        .on_latency_sensitive_typed::<Completion>(GlobalState::completion)
        .on_latency_sensitive_typed::<ResolveCompletionItem>(GlobalState::completion_resolve)
        .on_latency_sensitive_typed::<HoverRequest>(GlobalState::hover)
        .on_latency_sensitive_typed::<SignatureHelpRequest>(GlobalState::signature_help)
        .on_latency_sensitive_typed::<SemanticTokensFullRequest>(GlobalState::semantic_tokens_full)
        .on_latency_sensitive_typed::<SemanticTokensFullDeltaRequest>(
            GlobalState::semantic_tokens_full_delta,
        )
        .on_worker_typed::<GotoDefinition>(GlobalState::definition)
        .on_worker_typed::<GotoDeclaration>(GlobalState::declaration)
        .on_worker_typed::<GotoTypeDefinition>(GlobalState::type_definition)
        .on_worker_typed::<References>(GlobalState::references)
        .on_worker_typed::<DocumentHighlightRequest>(GlobalState::document_highlight)
        .on_worker_typed::<DocumentSymbolRequest>(GlobalState::document_symbol)
        .on_worker_typed::<WorkspaceSymbolRequest>(GlobalState::workspace_symbol)
        .on_worker_typed::<FoldingRangeRequest>(GlobalState::folding_range)
        .on_worker_typed::<SelectionRangeRequest>(GlobalState::selection_range)
        .on_worker_typed::<PrepareRenameRequest>(GlobalState::prepare_rename)
        .on_worker_typed::<Rename>(GlobalState::rename)
        .on_worker_typed::<CallHierarchyPrepare>(GlobalState::prepare_call_hierarchy)
        .on_worker_typed::<CallHierarchyIncomingCalls>(GlobalState::incoming_calls)
        .on_worker_typed::<CallHierarchyOutgoingCalls>(GlobalState::outgoing_calls)
        .on_worker::<CodeActionRequest>()
        .on_worker_typed::<SemanticTokensRangeRequest>(GlobalState::semantic_tokens_range)
        .on_worker::<InlayHintRequest>()
        .on_fmt_thread_typed::<Formatting>(GlobalState::formatting)
        .on_fmt_thread_typed::<RangeFormatting>(GlobalState::range_formatting)
        .on_fmt_thread_typed::<OnTypeFormatting>(GlobalState::on_type_formatting)
        .finish()
}

fn dispatch_notification(
    global_state: &mut GlobalState,
    notification: Notification,
    _legacy_input: &str,
) -> JsonRpcResult {
    let mut dispatcher = NotificationDispatcher::new(global_state, notification);
    dispatcher
        .on_sync_mut_typed::<Initialized>(GlobalState::initialized)
        .on_sync_mut_typed::<Exit>(GlobalState::exit)
        .on_sync_mut_typed::<Cancel>(GlobalState::cancel_request)
        .on_sync_mut_typed::<DidChangeConfiguration>(GlobalState::did_change_configuration)
        .on_sync_mut_typed::<DidChangeWorkspaceFolders>(GlobalState::did_change_workspace_folders)
        .on_sync_mut_typed::<DidChangeWatchedFiles>(GlobalState::did_change_watched_files)
        .on_sync_mut_typed::<DidSaveTextDocument>(GlobalState::did_save)
        .on_sync_mut_typed::<DidOpenTextDocument>(GlobalState::did_open)
        .on_sync_mut_typed::<DidChangeTextDocument>(GlobalState::did_change)
        .on_sync_mut_typed::<DidCloseTextDocument>(GlobalState::did_close)
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
        self.dispatch_typed::<R>(f);
        self
    }

    pub(crate) fn on_latency_sensitive_typed<R>(
        &mut self,
        f: fn(&mut GlobalState, lsp_server::RequestId, R::Params) -> JsonRpcResult,
    ) -> &mut Self
    where
        R: lsp_types::request::Request,
        R::Params: DeserializeOwned + Debug,
    {
        self.dispatch_typed::<R>(f);
        self
    }

    pub(crate) fn on_worker<R>(&mut self) -> &mut Self
    where
        R: lsp_types::request::Request,
    {
        self.dispatch_legacy::<R>();
        self
    }

    pub(crate) fn on_worker_typed<R>(
        &mut self,
        f: fn(&mut GlobalState, lsp_server::RequestId, R::Params) -> JsonRpcResult,
    ) -> &mut Self
    where
        R: lsp_types::request::Request,
        R::Params: DeserializeOwned + Debug,
    {
        self.dispatch_typed::<R>(f);
        self
    }

    pub(crate) fn on_fmt_thread_typed<R>(
        &mut self,
        f: fn(&mut GlobalState, lsp_server::RequestId, R::Params) -> JsonRpcResult,
    ) -> &mut Self
    where
        R: lsp_types::request::Request,
        R::Params: DeserializeOwned + Debug,
    {
        self.dispatch_typed::<R>(f);
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

    fn dispatch_typed<R>(
        &mut self,
        f: fn(&mut GlobalState, lsp_server::RequestId, R::Params) -> JsonRpcResult,
    ) where
        R: lsp_types::request::Request,
        R::Params: DeserializeOwned + Debug,
    {
        let Some(request) = self.take_matching::<R>() else {
            return;
        };
        let id = request.id;
        let params = match serde_json::from_value::<R::Params>(request.params) {
            Ok(params) => params,
            Err(error) => {
                self.result = invalid_params(id, R::METHOD, error);
                return;
            }
        };
        self.result = match panic::catch_unwind(panic::AssertUnwindSafe(|| {
            f(self.global_state, id.clone(), params)
        })) {
            Ok(result) => result,
            Err(payload) => handler_panic(id, R::METHOD, payload.as_ref()),
        };
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
    result: JsonRpcResult,
}

impl<'a> NotificationDispatcher<'a> {
    fn new(global_state: &'a mut GlobalState, notification: Notification) -> Self {
        Self {
            global_state,
            notification: Some(notification),
            result: JsonRpcResult::None,
        }
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
        self.result =
            match panic::catch_unwind(panic::AssertUnwindSafe(|| f(self.global_state, params))) {
                Ok(result) => result,
                Err(_) => JsonRpcResult::None,
            };
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

#[allow(dead_code)]
fn content_modified(id: RequestId) -> JsonRpcResult {
    JsonRpcResult::Response(error_response(
        Some(id),
        ErrorCode::ContentModified,
        "request result is stale because the document was modified",
    ))
}

fn invalid_params(
    id: lsp_server::RequestId,
    method: &str,
    error: serde_json::Error,
) -> JsonRpcResult {
    JsonRpcResult::Response(error_response(
        Some(request_id_from_lsp(id)),
        ErrorCode::InvalidParams,
        format!("invalid {method} params: {error}"),
    ))
}

fn handler_panic(
    id: lsp_server::RequestId,
    method: &str,
    payload: &(dyn Any + Send),
) -> JsonRpcResult {
    let detail = panic_message(payload).unwrap_or("unknown panic payload");
    JsonRpcResult::Response(error_response(
        Some(request_id_from_lsp(id)),
        ErrorCode::InternalError,
        format!("handler for `{method}` panicked: {detail}"),
    ))
}

fn panic_message(payload: &(dyn Any + Send)) -> Option<&str> {
    payload
        .downcast_ref::<&'static str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
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

#[cfg(test)]
mod tests {
    use crossbeam_channel::unbounded;
    use lsp_server::{Notification, Request};
    use serde_json::Value as JsonValue;

    use super::*;
    use crate::LaunchConfiguration;

    fn test_global_state() -> GlobalState {
        let (sender, _receiver) = unbounded();
        GlobalState::new(sender, LaunchConfiguration::new())
    }

    fn panic_request_handler(
        _state: &mut GlobalState,
        _id: lsp_server::RequestId,
        _params: lsp_types::InitializeParams,
    ) -> JsonRpcResult {
        panic!("synthetic request panic")
    }

    fn panic_notification_handler(
        _state: &mut GlobalState,
        _params: lsp_types::InitializedParams,
    ) -> JsonRpcResult {
        panic!("synthetic notification panic")
    }

    #[test]
    fn typed_request_dispatcher_projects_handler_panics_as_internal_error() {
        let mut global_state = test_global_state();
        let request = Request {
            id: lsp_server::RequestId::from(7),
            method: <lsp_types::request::Initialize as lsp_types::request::Request>::METHOD
                .to_owned(),
            params: serde_json::json!({
                "processId": null,
                "capabilities": {}
            }),
        };

        let mut dispatcher = RequestDispatcher::new(&mut global_state, request, "");
        let result = dispatcher
            .on_sync_mut_typed::<lsp_types::request::Initialize>(panic_request_handler)
            .finish();
        let response = result
            .into_response()
            .expect("panic should be projected as response");
        let response =
            serde_json::from_str::<JsonValue>(&response).expect("response should be valid JSON");

        assert_eq!(response["id"], 7);
        assert_eq!(response["error"]["code"], -32603);
        assert!(
            response["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("handler for `initialize` panicked"))
        );
    }

    #[test]
    fn dispatcher_projects_content_modified_as_lsp_error() {
        let result = content_modified(RequestId::String("hover-1".to_owned()));
        let response = result
            .into_response()
            .expect("content-modified should be projected as response");
        let response =
            serde_json::from_str::<JsonValue>(&response).expect("response should be valid JSON");

        assert_eq!(response["id"], "hover-1");
        assert_eq!(response["error"]["code"], -32801);
        assert!(
            response["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("stale"))
        );
    }

    #[test]
    fn dispatcher_projects_request_cancelled_as_lsp_error() {
        let result = request_cancelled(RequestId::Number(7));
        let response = result
            .into_response()
            .expect("request-cancelled should be projected as response");
        let response =
            serde_json::from_str::<JsonValue>(&response).expect("response should be valid JSON");

        assert_eq!(response["id"], 7);
        assert_eq!(response["error"]["code"], -32800);
        assert!(
            response["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("cancelled"))
        );
    }

    #[test]
    fn typed_notification_dispatcher_swallows_handler_panics() {
        let mut global_state = test_global_state();
        let notification = Notification {
            method:
                <lsp_types::notification::Initialized as lsp_types::notification::Notification>::METHOD
                    .to_owned(),
            params: serde_json::json!({}),
        };

        let mut dispatcher = NotificationDispatcher::new(&mut global_state, notification);
        let result = dispatcher
            .on_sync_mut_typed::<lsp_types::notification::Initialized>(panic_notification_handler)
            .finish();

        assert_eq!(result, JsonRpcResult::None);
    }
}
