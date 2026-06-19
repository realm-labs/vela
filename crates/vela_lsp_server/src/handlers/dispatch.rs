use std::{any::Any, fmt::Debug, panic};

use lsp_server::{Message, Notification, Request, RequestId, Response, ResponseError};
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
    ErrorCode, JsonRpcResult,
    global_state::GlobalState,
    global_state::GlobalStateSnapshot,
    rpc::typed_messages,
    task::{RetryTask, TaskLane},
};

pub(crate) fn dispatch_message(
    global_state: &mut GlobalState,
    message: &Message,
    legacy_input: &str,
) -> Vec<Message> {
    match message {
        Message::Request(request) => dispatch_request(global_state, request.clone(), legacy_input),
        Message::Notification(notification) => {
            dispatch_notification(global_state, notification.clone(), legacy_input)
        }
        Message::Response(_) => typed_messages(global_state.handle_legacy_json(legacy_input)),
    }
}

#[cfg(test)]
pub(crate) fn dispatch_message_result(
    global_state: &mut GlobalState,
    message: &Message,
    legacy_input: &str,
) -> JsonRpcResult {
    crate::rpc::result_from_messages(dispatch_message(global_state, message, legacy_input))
}

fn dispatch_request(
    global_state: &mut GlobalState,
    request: Request,
    legacy_input: &str,
) -> Vec<Message> {
    let request_id = request.id.clone();
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
        .on_retryable_latency_snapshot_typed::<Completion>(
            GlobalStateSnapshot::completion,
            RetryTask::completion,
        )
        .on_retryable_latency_snapshot_typed::<ResolveCompletionItem>(
            GlobalStateSnapshot::completion_resolve,
            RetryTask::completion_resolve,
        )
        .on_latency_sensitive_snapshot_messages_typed::<HoverRequest>(GlobalStateSnapshot::hover)
        .on_latency_sensitive_snapshot_messages_typed::<SignatureHelpRequest>(
            GlobalStateSnapshot::signature_help,
        )
        .on_retryable_latency_snapshot_typed::<SemanticTokensFullRequest>(
            GlobalStateSnapshot::semantic_tokens_full,
            RetryTask::semantic_tokens_full,
        )
        .on_latency_sensitive_snapshot_messages_typed::<SemanticTokensFullDeltaRequest>(
            GlobalStateSnapshot::semantic_tokens_full_delta,
        )
        .on_worker_snapshot_messages_typed::<GotoDefinition>(GlobalStateSnapshot::definition)
        .on_worker_snapshot_messages_typed::<GotoDeclaration>(GlobalStateSnapshot::declaration)
        .on_worker_snapshot_messages_typed::<GotoTypeDefinition>(
            GlobalStateSnapshot::type_definition,
        )
        .on_worker_snapshot_messages_typed::<References>(GlobalStateSnapshot::references)
        .on_worker_snapshot_messages_typed::<DocumentHighlightRequest>(
            GlobalStateSnapshot::document_highlight,
        )
        .on_retryable_worker_snapshot_messages_typed::<DocumentSymbolRequest>(
            GlobalStateSnapshot::document_symbol,
            RetryTask::document_symbol,
        )
        .on_retryable_worker_snapshot_messages_typed::<WorkspaceSymbolRequest>(
            GlobalStateSnapshot::workspace_symbol,
            RetryTask::workspace_symbol,
        )
        .on_retryable_worker_snapshot_messages_typed::<FoldingRangeRequest>(
            GlobalStateSnapshot::folding_range,
            RetryTask::folding_range,
        )
        .on_worker_snapshot_messages_typed::<SelectionRangeRequest>(
            GlobalStateSnapshot::selection_range,
        )
        .on_worker_snapshot_messages_typed::<PrepareRenameRequest>(
            GlobalStateSnapshot::prepare_rename,
        )
        .on_worker_snapshot_messages_typed::<Rename>(GlobalStateSnapshot::rename)
        .on_worker_snapshot_messages_typed::<CallHierarchyPrepare>(
            GlobalStateSnapshot::prepare_call_hierarchy,
        )
        .on_worker_snapshot_messages_typed::<CallHierarchyIncomingCalls>(
            GlobalStateSnapshot::incoming_calls,
        )
        .on_worker_snapshot_messages_typed::<CallHierarchyOutgoingCalls>(
            GlobalStateSnapshot::outgoing_calls,
        )
        .on_worker_snapshot_messages_typed::<CodeActionRequest>(GlobalStateSnapshot::code_action)
        .on_worker_snapshot_messages_typed::<SemanticTokensRangeRequest>(
            GlobalStateSnapshot::semantic_tokens_range,
        )
        .on_worker_snapshot_typed::<InlayHintRequest>(GlobalStateSnapshot::inlay_hint)
        .on_fmt_thread_snapshot_typed::<Formatting>(GlobalStateSnapshot::formatting)
        .on_fmt_thread_snapshot_typed::<RangeFormatting>(GlobalStateSnapshot::range_formatting)
        .on_fmt_thread_snapshot_typed::<OnTypeFormatting>(GlobalStateSnapshot::on_type_formatting)
        .finish()
}

fn dispatch_notification(
    global_state: &mut GlobalState,
    notification: Notification,
    _legacy_input: &str,
) -> Vec<Message> {
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

pub(crate) fn retry_stale_request(global_state: &mut GlobalState, retry: RetryTask) {
    match retry {
        RetryTask::Completion {
            id,
            request_id,
            params,
            attempts,
        } => {
            schedule_retry(
                global_state,
                RetrySchedule {
                    lane: TaskLane::Latency,
                    method: <Completion as lsp_types::request::Request>::METHOD,
                    id,
                    request_id,
                    params: *params,
                    attempts,
                    retry: |id, request_id, params, attempts| RetryTask::Completion {
                        id,
                        request_id,
                        params: Box::new(params),
                        attempts,
                    },
                    f: GlobalStateSnapshot::completion,
                },
            );
        }
        RetryTask::CompletionResolve {
            id,
            request_id,
            params,
            attempts,
        } => {
            schedule_retry(
                global_state,
                RetrySchedule {
                    lane: TaskLane::Latency,
                    method: <ResolveCompletionItem as lsp_types::request::Request>::METHOD,
                    id,
                    request_id,
                    params: *params,
                    attempts,
                    retry: |id, request_id, params, attempts| RetryTask::CompletionResolve {
                        id,
                        request_id,
                        params: Box::new(params),
                        attempts,
                    },
                    f: GlobalStateSnapshot::completion_resolve,
                },
            );
        }
        RetryTask::SemanticTokensFull {
            id,
            request_id,
            params,
            attempts,
        } => {
            schedule_retry(
                global_state,
                RetrySchedule {
                    lane: TaskLane::Latency,
                    method: <SemanticTokensFullRequest as lsp_types::request::Request>::METHOD,
                    id,
                    request_id,
                    params: *params,
                    attempts,
                    retry: |id, request_id, params, attempts| RetryTask::SemanticTokensFull {
                        id,
                        request_id,
                        params: Box::new(params),
                        attempts,
                    },
                    f: GlobalStateSnapshot::semantic_tokens_full,
                },
            );
        }
        RetryTask::DocumentSymbol {
            id,
            request_id,
            params,
            attempts,
        } => {
            schedule_messages_retry(
                global_state,
                RetryMessagesSchedule {
                    lane: TaskLane::Worker,
                    method: <DocumentSymbolRequest as lsp_types::request::Request>::METHOD,
                    id,
                    request_id,
                    params: *params,
                    attempts,
                    retry: |id, request_id, params, attempts| RetryTask::DocumentSymbol {
                        id,
                        request_id,
                        params: Box::new(params),
                        attempts,
                    },
                    f: GlobalStateSnapshot::document_symbol,
                },
            );
        }
        RetryTask::FoldingRange {
            id,
            request_id,
            params,
            attempts,
        } => {
            schedule_messages_retry(
                global_state,
                RetryMessagesSchedule {
                    lane: TaskLane::Worker,
                    method: <FoldingRangeRequest as lsp_types::request::Request>::METHOD,
                    id,
                    request_id,
                    params: *params,
                    attempts,
                    retry: |id, request_id, params, attempts| RetryTask::FoldingRange {
                        id,
                        request_id,
                        params: Box::new(params),
                        attempts,
                    },
                    f: GlobalStateSnapshot::folding_range,
                },
            );
        }
        RetryTask::WorkspaceSymbol {
            id,
            request_id,
            params,
            attempts,
        } => {
            schedule_messages_retry(
                global_state,
                RetryMessagesSchedule {
                    lane: TaskLane::Worker,
                    method: <WorkspaceSymbolRequest as lsp_types::request::Request>::METHOD,
                    id,
                    request_id,
                    params: *params,
                    attempts,
                    retry: |id, request_id, params, attempts| RetryTask::WorkspaceSymbol {
                        id,
                        request_id,
                        params: Box::new(params),
                        attempts,
                    },
                    f: GlobalStateSnapshot::workspace_symbol,
                },
            );
        }
    }
}

struct RetrySchedule<P, C> {
    lane: TaskLane,
    method: &'static str,
    id: lsp_server::RequestId,
    request_id: RequestId,
    params: P,
    attempts: u8,
    retry: C,
    f: fn(GlobalStateSnapshot, lsp_server::RequestId, P) -> JsonRpcResult,
}

struct RetryMessagesSchedule<P, C> {
    lane: TaskLane,
    method: &'static str,
    id: lsp_server::RequestId,
    request_id: RequestId,
    params: P,
    attempts: u8,
    retry: C,
    f: fn(GlobalStateSnapshot, lsp_server::RequestId, P) -> Vec<Message>,
}

fn schedule_retry<P, C>(global_state: &mut GlobalState, schedule: RetrySchedule<P, C>)
where
    P: Clone + Send + 'static,
    C: Fn(lsp_server::RequestId, RequestId, P, u8) -> RetryTask + Send + 'static,
{
    let RetrySchedule {
        lane,
        method,
        id,
        request_id,
        params,
        attempts,
        retry,
        f,
    } = schedule;
    let snapshot = global_state.snapshot();
    let generation = global_state.register_in_flight_cancellation(request_id.clone());
    global_state.task_scheduler().spawn_retryable_for_request(
        lane,
        method,
        request_id.clone(),
        generation,
        retry(id.clone(), request_id, params.clone(), attempts),
        move || match panic::catch_unwind(panic::AssertUnwindSafe(|| {
            f(snapshot, id.clone(), params)
        })) {
            Ok(result) => typed_messages(result),
            Err(payload) => handler_panic(id, method, payload.as_ref()),
        },
    );
}

fn schedule_messages_retry<P, C>(
    global_state: &mut GlobalState,
    schedule: RetryMessagesSchedule<P, C>,
) where
    P: Clone + Send + 'static,
    C: Fn(lsp_server::RequestId, RequestId, P, u8) -> RetryTask + Send + 'static,
{
    let RetryMessagesSchedule {
        lane,
        method,
        id,
        request_id,
        params,
        attempts,
        retry,
        f,
    } = schedule;
    let snapshot = global_state.snapshot();
    let generation = global_state.register_in_flight_cancellation(request_id.clone());
    global_state.task_scheduler().spawn_retryable_for_request(
        lane,
        method,
        request_id.clone(),
        generation,
        retry(id.clone(), request_id, params.clone(), attempts),
        move || match panic::catch_unwind(panic::AssertUnwindSafe(|| {
            f(snapshot, id.clone(), params)
        })) {
            Ok(messages) => messages,
            Err(payload) => handler_panic(id, method, payload.as_ref()),
        },
    );
}

pub(crate) struct RequestDispatcher<'a> {
    global_state: &'a mut GlobalState,
    request: Option<Request>,
    legacy_input: &'a str,
    result: Vec<Message>,
}

impl<'a> RequestDispatcher<'a> {
    fn new(global_state: &'a mut GlobalState, request: Request, legacy_input: &'a str) -> Self {
        Self {
            global_state,
            request: Some(request),
            legacy_input,
            result: Vec::new(),
        }
    }

    pub(crate) fn on_sync_mut_typed<R>(
        &mut self,
        f: fn(&mut GlobalState, lsp_server::RequestId, R::Params) -> Vec<Message>,
    ) -> &mut Self
    where
        R: lsp_types::request::Request,
        R::Params: DeserializeOwned + Debug,
    {
        self.dispatch_typed::<R>(f);
        self
    }

    pub(crate) fn on_latency_sensitive_snapshot_messages_typed<R>(
        &mut self,
        f: fn(GlobalStateSnapshot, lsp_server::RequestId, R::Params) -> Vec<Message>,
    ) -> &mut Self
    where
        R: lsp_types::request::Request,
        R::Params: DeserializeOwned + Debug,
    {
        self.dispatch_snapshot_messages_typed::<R>(f);
        self
    }

    pub(crate) fn on_retryable_latency_snapshot_typed<R>(
        &mut self,
        f: fn(GlobalStateSnapshot, lsp_server::RequestId, R::Params) -> JsonRpcResult,
        retry: fn(lsp_server::RequestId, RequestId, R::Params) -> RetryTask,
    ) -> &mut Self
    where
        R: lsp_types::request::Request,
        R::Params: DeserializeOwned + Debug + Send + Clone + 'static,
    {
        self.dispatch_retryable_snapshot_task_typed::<R>(TaskLane::Latency, f, retry);
        self
    }

    pub(crate) fn on_worker_snapshot_typed<R>(
        &mut self,
        f: fn(GlobalStateSnapshot, lsp_server::RequestId, R::Params) -> JsonRpcResult,
    ) -> &mut Self
    where
        R: lsp_types::request::Request,
        R::Params: DeserializeOwned + Debug,
    {
        self.dispatch_snapshot_typed::<R>(f);
        self
    }

    pub(crate) fn on_worker_snapshot_messages_typed<R>(
        &mut self,
        f: fn(GlobalStateSnapshot, lsp_server::RequestId, R::Params) -> Vec<Message>,
    ) -> &mut Self
    where
        R: lsp_types::request::Request,
        R::Params: DeserializeOwned + Debug,
    {
        self.dispatch_snapshot_messages_typed::<R>(f);
        self
    }

    pub(crate) fn on_retryable_worker_snapshot_messages_typed<R>(
        &mut self,
        f: fn(GlobalStateSnapshot, lsp_server::RequestId, R::Params) -> Vec<Message>,
        retry: fn(lsp_server::RequestId, RequestId, R::Params) -> RetryTask,
    ) -> &mut Self
    where
        R: lsp_types::request::Request,
        R::Params: DeserializeOwned + Debug + Send + Clone + 'static,
    {
        self.dispatch_retryable_snapshot_messages_task_typed::<R>(TaskLane::Worker, f, retry);
        self
    }

    pub(crate) fn on_fmt_thread_snapshot_typed<R>(
        &mut self,
        f: fn(GlobalStateSnapshot, lsp_server::RequestId, R::Params) -> JsonRpcResult,
    ) -> &mut Self
    where
        R: lsp_types::request::Request,
        R::Params: DeserializeOwned + Debug + Send + 'static,
    {
        self.dispatch_snapshot_task_typed::<R>(TaskLane::Formatting, f);
        self
    }

    pub(crate) fn finish(&mut self) -> Vec<Message> {
        if let Some(request) = self.request.take() {
            self.result = if is_known_notification_method(&request.method) {
                typed_messages(self.global_state.handle_legacy_json(self.legacy_input))
            } else {
                method_not_found(request.id, &request.method)
            };
        }
        std::mem::take(&mut self.result)
    }

    fn dispatch_typed<R>(
        &mut self,
        f: fn(&mut GlobalState, lsp_server::RequestId, R::Params) -> Vec<Message>,
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
            Ok(messages) => messages,
            Err(payload) => handler_panic(id, R::METHOD, payload.as_ref()),
        };
    }

    fn dispatch_snapshot_typed<R>(
        &mut self,
        f: fn(GlobalStateSnapshot, lsp_server::RequestId, R::Params) -> JsonRpcResult,
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
        let snapshot = self.global_state.snapshot();
        self.result = match panic::catch_unwind(panic::AssertUnwindSafe(|| {
            f(snapshot, id.clone(), params)
        })) {
            Ok(result) => typed_messages(result),
            Err(payload) => handler_panic(id, R::METHOD, payload.as_ref()),
        };
    }

    fn dispatch_snapshot_messages_typed<R>(
        &mut self,
        f: fn(GlobalStateSnapshot, lsp_server::RequestId, R::Params) -> Vec<Message>,
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
        let snapshot = self.global_state.snapshot();
        self.result = match panic::catch_unwind(panic::AssertUnwindSafe(|| {
            f(snapshot, id.clone(), params)
        })) {
            Ok(messages) => messages,
            Err(payload) => handler_panic(id, R::METHOD, payload.as_ref()),
        };
    }

    fn dispatch_snapshot_task_typed<R>(
        &mut self,
        lane: TaskLane,
        f: fn(GlobalStateSnapshot, lsp_server::RequestId, R::Params) -> JsonRpcResult,
    ) where
        R: lsp_types::request::Request,
        R::Params: DeserializeOwned + Debug + Send + 'static,
    {
        let Some(request) = self.take_matching::<R>() else {
            return;
        };
        let id = request.id;
        let request_id = id.clone();
        let params = match serde_json::from_value::<R::Params>(request.params) {
            Ok(params) => params,
            Err(error) => {
                self.result = invalid_params(id, R::METHOD, error);
                return;
            }
        };
        let snapshot = self.global_state.snapshot();
        let generation = self
            .global_state
            .register_in_flight_cancellation(request_id.clone());
        self.global_state.task_scheduler().spawn_for_request(
            lane,
            R::METHOD,
            request_id,
            generation,
            move || match panic::catch_unwind(panic::AssertUnwindSafe(|| {
                f(snapshot, id.clone(), params)
            })) {
                Ok(result) => typed_messages(result),
                Err(payload) => handler_panic(id, R::METHOD, payload.as_ref()),
            },
        );
        self.result.clear();
    }

    fn dispatch_retryable_snapshot_task_typed<R>(
        &mut self,
        lane: TaskLane,
        f: fn(GlobalStateSnapshot, lsp_server::RequestId, R::Params) -> JsonRpcResult,
        retry: fn(lsp_server::RequestId, RequestId, R::Params) -> RetryTask,
    ) where
        R: lsp_types::request::Request,
        R::Params: DeserializeOwned + Debug + Send + Clone + 'static,
    {
        let Some(request) = self.take_matching::<R>() else {
            return;
        };
        let id = request.id;
        let request_id = id.clone();
        let params = match serde_json::from_value::<R::Params>(request.params) {
            Ok(params) => params,
            Err(error) => {
                self.result = invalid_params(id, R::METHOD, error);
                return;
            }
        };
        let snapshot = self.global_state.snapshot();
        let generation = self
            .global_state
            .register_in_flight_cancellation(request_id.clone());
        let retry = retry(id.clone(), request_id.clone(), params.clone());
        self.global_state
            .task_scheduler()
            .spawn_retryable_for_request(
                lane,
                R::METHOD,
                request_id,
                generation,
                retry,
                move || match panic::catch_unwind(panic::AssertUnwindSafe(|| {
                    f(snapshot, id.clone(), params)
                })) {
                    Ok(result) => typed_messages(result),
                    Err(payload) => handler_panic(id, R::METHOD, payload.as_ref()),
                },
            );
        self.result.clear();
    }

    fn dispatch_retryable_snapshot_messages_task_typed<R>(
        &mut self,
        lane: TaskLane,
        f: fn(GlobalStateSnapshot, lsp_server::RequestId, R::Params) -> Vec<Message>,
        retry: fn(lsp_server::RequestId, RequestId, R::Params) -> RetryTask,
    ) where
        R: lsp_types::request::Request,
        R::Params: DeserializeOwned + Debug + Send + Clone + 'static,
    {
        let Some(request) = self.take_matching::<R>() else {
            return;
        };
        let id = request.id;
        let request_id = id.clone();
        let params = match serde_json::from_value::<R::Params>(request.params) {
            Ok(params) => params,
            Err(error) => {
                self.result = invalid_params(id, R::METHOD, error);
                return;
            }
        };
        let snapshot = self.global_state.snapshot();
        let generation = self
            .global_state
            .register_in_flight_cancellation(request_id.clone());
        let retry = retry(id.clone(), request_id.clone(), params.clone());
        self.global_state
            .task_scheduler()
            .spawn_retryable_for_request(
                lane,
                R::METHOD,
                request_id,
                generation,
                retry,
                move || match panic::catch_unwind(panic::AssertUnwindSafe(|| {
                    f(snapshot, id.clone(), params)
                })) {
                    Ok(messages) => messages,
                    Err(payload) => handler_panic(id, R::METHOD, payload.as_ref()),
                },
            );
        self.result.clear();
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
    result: Vec<Message>,
}

impl<'a> NotificationDispatcher<'a> {
    fn new(global_state: &'a mut GlobalState, notification: Notification) -> Self {
        Self {
            global_state,
            notification: Some(notification),
            result: Vec::new(),
        }
    }

    pub(crate) fn on_sync_mut_typed<N>(
        &mut self,
        f: fn(&mut GlobalState, N::Params) -> Vec<Message>,
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
                self.result.clear();
                return self;
            }
        };
        self.result = panic::catch_unwind(panic::AssertUnwindSafe(|| f(self.global_state, params)))
            .unwrap_or_default();
        self
    }

    pub(crate) fn finish(&mut self) -> Vec<Message> {
        if self.notification.take().is_some() {
            self.result.clear();
        }
        std::mem::take(&mut self.result)
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

fn method_not_found(id: lsp_server::RequestId, method: &str) -> Vec<Message> {
    error_message(
        id,
        ErrorCode::MethodNotFound,
        format!("method `{method}` is not implemented"),
    )
}

fn server_not_initialized(id: lsp_server::RequestId) -> Vec<Message> {
    error_message(
        id,
        ErrorCode::ServerNotInitialized,
        "server has not been initialized",
    )
}

fn server_shut_down(id: lsp_server::RequestId) -> Vec<Message> {
    error_message(id, ErrorCode::InvalidRequest, "server has shut down")
}

pub(crate) fn request_cancelled(id: RequestId) -> Vec<Message> {
    error_message(
        id,
        ErrorCode::RequestCancelled,
        "request was cancelled before processing",
    )
}

pub(crate) fn content_modified(id: RequestId) -> Vec<Message> {
    error_message(
        id,
        ErrorCode::ContentModified,
        "request result is stale because the document was modified",
    )
}

fn invalid_params(
    id: lsp_server::RequestId,
    method: &str,
    error: serde_json::Error,
) -> Vec<Message> {
    error_message(
        id,
        ErrorCode::InvalidParams,
        format!("invalid {method} params: {error}"),
    )
}

fn handler_panic(
    id: lsp_server::RequestId,
    method: &str,
    payload: &(dyn Any + Send),
) -> Vec<Message> {
    let detail = panic_message(payload).unwrap_or("unknown panic payload");
    error_message(
        id,
        ErrorCode::InternalError,
        format!("handler for `{method}` panicked: {detail}"),
    )
}

fn error_message(
    id: lsp_server::RequestId,
    code: ErrorCode,
    message: impl Into<String>,
) -> Vec<Message> {
    vec![Message::Response(Response {
        id,
        result: None,
        error: Some(ResponseError {
            code: code.value(),
            message: message.into(),
            data: None,
        }),
    })]
}

fn panic_message(payload: &(dyn Any + Send)) -> Option<&str> {
    payload
        .downcast_ref::<&'static str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
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
    ) -> Vec<Message> {
        panic!("synthetic request panic")
    }

    fn panic_notification_handler(
        _state: &mut GlobalState,
        _params: lsp_types::InitializedParams,
    ) -> Vec<Message> {
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
        let messages = dispatcher
            .on_sync_mut_typed::<lsp_types::request::Initialize>(panic_request_handler)
            .finish();
        let response = response_value(messages);

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
        let response = response_value(content_modified(RequestId::from("hover-1".to_owned())));

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
        let response = response_value(request_cancelled(RequestId::from(7)));

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
        let messages = dispatcher
            .on_sync_mut_typed::<lsp_types::notification::Initialized>(panic_notification_handler)
            .finish();

        assert!(messages.is_empty());
    }

    fn response_value(messages: Vec<Message>) -> JsonValue {
        assert_eq!(messages.len(), 1);
        let response = crate::rpc::serialize_message(&messages[0]);
        serde_json::from_str::<JsonValue>(&response).expect("response should be valid JSON")
    }
}
