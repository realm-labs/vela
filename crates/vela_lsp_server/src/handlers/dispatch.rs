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

use crate::{ErrorCode, JsonRpcResult, RequestId, error_response, global_state::GlobalState};

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

    let mut dispatcher = RequestDispatcher::new(global_state, request, legacy_input);
    dispatcher
        .on_sync_mut::<lsp_types::request::Initialize>()
        .on_sync_mut::<lsp_types::request::Shutdown>()
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
        .on_sync_mut::<Initialized>()
        .on_sync_mut::<Exit>()
        .on_sync_mut::<Cancel>()
        .on_sync_mut::<DidOpenTextDocument>()
        .on_sync_mut::<DidChangeTextDocument>()
        .on_sync_mut::<DidCloseTextDocument>()
        .on_sync_mut::<DidSaveTextDocument>()
        .on_sync_mut::<DidChangeConfiguration>()
        .on_sync_mut::<DidChangeWorkspaceFolders>()
        .on_sync_mut::<DidChangeWatchedFiles>()
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

    pub(crate) fn on_sync_mut<R>(&mut self) -> &mut Self
    where
        R: lsp_types::request::Request,
    {
        self.dispatch_legacy::<R>();
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
            self.result = if !self.global_state.is_initialized()
                || self.global_state.is_shutdown_requested()
            {
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
        if self
            .request
            .as_ref()
            .is_some_and(|request| request.method == R::METHOD)
        {
            self.result = self.global_state.handle_legacy_json(self.legacy_input);
            self.request = None;
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

    pub(crate) fn finish(&mut self) -> JsonRpcResult {
        if self.notification.take().is_some() {
            self.result = JsonRpcResult::None;
        }
        std::mem::replace(&mut self.result, JsonRpcResult::None)
    }
}

fn method_not_found(id: lsp_server::RequestId, method: &str) -> JsonRpcResult {
    JsonRpcResult::Response(error_response(
        Some(rpc_request_id(id)),
        ErrorCode::MethodNotFound,
        format!("method `{method}` is not implemented"),
    ))
}

fn request_cancelled(id: RequestId) -> JsonRpcResult {
    JsonRpcResult::Response(error_response(
        Some(id),
        ErrorCode::RequestCancelled,
        "request was cancelled before processing",
    ))
}

fn rpc_request_id(id: lsp_server::RequestId) -> RequestId {
    let value = serde_json::to_value(id).expect("lsp-server request id should serialize");
    serde_json::from_value(value).expect("lsp-server request id should match JSON-RPC id shape")
}
