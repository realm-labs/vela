use std::thread;

use crossbeam_channel::{Receiver, Sender, unbounded};
use lsp_server::{Message, RequestId};
use lsp_types::{
    CompletionItem, CompletionParams, DocumentSymbolParams, FoldingRangeParams,
    SemanticTokensParams, WorkspaceSymbolParams,
};
use vela_language_service::GenerationToken;

#[cfg(test)]
use crate::JsonRpcResult;

type TaskJob = Box<dyn FnOnce() -> TaskResult + Send + 'static>;

#[derive(Debug, Clone)]
pub(crate) enum TaskResult {
    Response {
        lane: TaskLane,
        method: Option<String>,
        document_uri: Option<String>,
        request_id: Option<RequestId>,
        generation: Option<GenerationToken>,
        retry: Option<Box<RetryTask>>,
        timing: Option<TaskTiming>,
        messages: Vec<Message>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TaskTiming {
    queued_ms: u128,
    started_ms: u128,
    ended_ms: u128,
}

impl TaskTiming {
    pub(crate) const fn new(queued_ms: u128, started_ms: u128, ended_ms: u128) -> Self {
        Self {
            queued_ms,
            started_ms,
            ended_ms,
        }
    }

    pub(crate) const fn queued_ms(self) -> u128 {
        self.queued_ms
    }

    pub(crate) const fn started_ms(self) -> u128 {
        self.started_ms
    }

    pub(crate) const fn ended_ms(self) -> u128 {
        self.ended_ms
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TaskLifecycleEvent {
    kind: TaskLifecycleKind,
    lane: TaskLane,
    method: Option<String>,
    document_uri: Option<String>,
    request_id: Option<RequestId>,
    generation: Option<GenerationToken>,
    queued_ms: u128,
    started_ms: Option<u128>,
    ended_ms: Option<u128>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaskLifecycleKind {
    Queued,
    Started,
    Ended,
}

impl TaskLifecycleKind {
    pub(crate) const fn event_name(self) -> &'static str {
        match self {
            Self::Queued => "request_queued",
            Self::Started => "task_started",
            Self::Ended => "task_ended",
        }
    }

    pub(crate) const fn status(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Started => "started",
            Self::Ended => "ended",
        }
    }
}

impl TaskLifecycleEvent {
    fn queued(descriptor: &TaskDescriptor, queued_ms: u128) -> Self {
        Self::new(descriptor, TaskLifecycleKind::Queued, queued_ms, None, None)
    }

    fn started(descriptor: &TaskDescriptor, queued_ms: u128, started_ms: u128) -> Self {
        Self::new(
            descriptor,
            TaskLifecycleKind::Started,
            queued_ms,
            Some(started_ms),
            None,
        )
    }

    fn ended(
        descriptor: &TaskDescriptor,
        queued_ms: u128,
        started_ms: u128,
        ended_ms: u128,
    ) -> Self {
        Self::new(
            descriptor,
            TaskLifecycleKind::Ended,
            queued_ms,
            Some(started_ms),
            Some(ended_ms),
        )
    }

    fn new(
        descriptor: &TaskDescriptor,
        kind: TaskLifecycleKind,
        queued_ms: u128,
        started_ms: Option<u128>,
        ended_ms: Option<u128>,
    ) -> Self {
        Self {
            kind,
            lane: descriptor.lane,
            method: descriptor.method.clone(),
            document_uri: descriptor.document_uri.clone(),
            request_id: descriptor.request_id.clone(),
            generation: descriptor.generation.clone(),
            queued_ms,
            started_ms,
            ended_ms,
        }
    }

    pub(crate) const fn kind(&self) -> TaskLifecycleKind {
        self.kind
    }

    pub(crate) const fn lane(&self) -> TaskLane {
        self.lane
    }

    pub(crate) fn method(&self) -> Option<&str> {
        self.method.as_deref()
    }

    pub(crate) fn document_uri(&self) -> Option<&str> {
        self.document_uri.as_deref()
    }

    pub(crate) fn request_id(&self) -> Option<&RequestId> {
        self.request_id.as_ref()
    }

    pub(crate) fn generation_token(&self) -> Option<&GenerationToken> {
        self.generation.as_ref()
    }

    pub(crate) const fn timestamp_ms(&self) -> u128 {
        match self.kind {
            TaskLifecycleKind::Queued => self.queued_ms,
            TaskLifecycleKind::Started => match self.started_ms {
                Some(started_ms) => started_ms,
                None => self.queued_ms,
            },
            TaskLifecycleKind::Ended => match self.ended_ms {
                Some(ended_ms) => ended_ms,
                None => self.queued_ms,
            },
        }
    }

    pub(crate) fn queue_ms(&self) -> Option<u128> {
        self.started_ms
            .map(|started_ms| started_ms.saturating_sub(self.queued_ms))
            .or(Some(0))
    }

    pub(crate) fn handle_ms(&self) -> Option<u128> {
        self.started_ms
            .zip(self.ended_ms)
            .map(|(started_ms, ended_ms)| ended_ms.saturating_sub(started_ms))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaskOutcome {
    Completed,
    Cancelled,
    StaleDiscarded,
    Retried,
}

#[derive(Debug, Clone)]
pub(crate) struct TaskRequestMetadata {
    method: String,
    document_uri: Option<String>,
    request_id: RequestId,
    generation: GenerationToken,
}

impl TaskRequestMetadata {
    pub(crate) fn new(
        method: impl Into<String>,
        document_uri: Option<String>,
        request_id: RequestId,
        generation: GenerationToken,
    ) -> Self {
        Self {
            method: method.into(),
            document_uri,
            request_id,
            generation,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum RetryTask {
    Completion {
        id: lsp_server::RequestId,
        request_id: RequestId,
        params: Box<CompletionParams>,
        attempts: u8,
    },
    CompletionResolve {
        id: lsp_server::RequestId,
        request_id: RequestId,
        params: Box<CompletionItem>,
        attempts: u8,
    },
    SemanticTokensFull {
        id: lsp_server::RequestId,
        request_id: RequestId,
        params: Box<SemanticTokensParams>,
        attempts: u8,
    },
    DocumentSymbol {
        id: lsp_server::RequestId,
        request_id: RequestId,
        params: Box<DocumentSymbolParams>,
        attempts: u8,
    },
    FoldingRange {
        id: lsp_server::RequestId,
        request_id: RequestId,
        params: Box<FoldingRangeParams>,
        attempts: u8,
    },
    WorkspaceSymbol {
        id: lsp_server::RequestId,
        request_id: RequestId,
        params: Box<WorkspaceSymbolParams>,
        attempts: u8,
    },
}

impl RetryTask {
    pub(crate) fn completion(
        id: lsp_server::RequestId,
        request_id: RequestId,
        params: CompletionParams,
    ) -> Self {
        Self::Completion {
            id,
            request_id,
            params: Box::new(params),
            attempts: 0,
        }
    }

    pub(crate) fn completion_resolve(
        id: lsp_server::RequestId,
        request_id: RequestId,
        params: CompletionItem,
    ) -> Self {
        Self::CompletionResolve {
            id,
            request_id,
            params: Box::new(params),
            attempts: 0,
        }
    }

    pub(crate) fn semantic_tokens_full(
        id: lsp_server::RequestId,
        request_id: RequestId,
        params: SemanticTokensParams,
    ) -> Self {
        Self::SemanticTokensFull {
            id,
            request_id,
            params: Box::new(params),
            attempts: 0,
        }
    }

    pub(crate) fn document_symbol(
        id: lsp_server::RequestId,
        request_id: RequestId,
        params: DocumentSymbolParams,
    ) -> Self {
        Self::DocumentSymbol {
            id,
            request_id,
            params: Box::new(params),
            attempts: 0,
        }
    }

    pub(crate) fn folding_range(
        id: lsp_server::RequestId,
        request_id: RequestId,
        params: FoldingRangeParams,
    ) -> Self {
        Self::FoldingRange {
            id,
            request_id,
            params: Box::new(params),
            attempts: 0,
        }
    }

    pub(crate) fn workspace_symbol(
        id: lsp_server::RequestId,
        request_id: RequestId,
        params: WorkspaceSymbolParams,
    ) -> Self {
        Self::WorkspaceSymbol {
            id,
            request_id,
            params: Box::new(params),
            attempts: 0,
        }
    }

    pub(crate) fn next_attempt(self) -> Option<Self> {
        match self {
            Self::Completion {
                id,
                request_id,
                params,
                attempts,
            } if attempts == 0 => Some(Self::Completion {
                id,
                request_id,
                params,
                attempts: attempts + 1,
            }),
            Self::CompletionResolve {
                id,
                request_id,
                params,
                attempts,
            } if attempts == 0 => Some(Self::CompletionResolve {
                id,
                request_id,
                params,
                attempts: attempts + 1,
            }),
            Self::SemanticTokensFull {
                id,
                request_id,
                params,
                attempts,
            } if attempts == 0 => Some(Self::SemanticTokensFull {
                id,
                request_id,
                params,
                attempts: attempts + 1,
            }),
            Self::DocumentSymbol {
                id,
                request_id,
                params,
                attempts,
            } if attempts == 0 => Some(Self::DocumentSymbol {
                id,
                request_id,
                params,
                attempts: attempts + 1,
            }),
            Self::FoldingRange {
                id,
                request_id,
                params,
                attempts,
            } if attempts == 0 => Some(Self::FoldingRange {
                id,
                request_id,
                params,
                attempts: attempts + 1,
            }),
            Self::WorkspaceSymbol {
                id,
                request_id,
                params,
                attempts,
            } if attempts == 0 => Some(Self::WorkspaceSymbol {
                id,
                request_id,
                params,
                attempts: attempts + 1,
            }),
            Self::Completion { .. }
            | Self::CompletionResolve { .. }
            | Self::SemanticTokensFull { .. }
            | Self::DocumentSymbol { .. }
            | Self::FoldingRange { .. }
            | Self::WorkspaceSymbol { .. } => None,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaskLane {
    Main,
    Latency,
    Formatting,
    Worker,
}

impl TaskLane {
    pub(crate) const fn as_trace_str(self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::Latency => "latency",
            Self::Formatting => "formatting",
            Self::Worker => "worker",
        }
    }
}

pub(crate) struct TaskScheduler {
    latency_jobs: Sender<TaskJob>,
    formatting_jobs: Sender<TaskJob>,
    worker_jobs: Sender<TaskJob>,
    lifecycle_sender: Sender<TaskLifecycleEvent>,
    lifecycle_events: Receiver<TaskLifecycleEvent>,
    latency_results: Receiver<TaskResult>,
    formatting_results: Receiver<TaskResult>,
    worker_results: Receiver<TaskResult>,
}

#[derive(Clone)]
struct TaskDescriptor {
    lane: TaskLane,
    method: Option<String>,
    document_uri: Option<String>,
    request_id: Option<RequestId>,
    generation: Option<GenerationToken>,
    retry: Option<RetryTask>,
}

impl TaskDescriptor {
    const fn lane(lane: TaskLane) -> Self {
        Self {
            lane,
            method: None,
            document_uri: None,
            request_id: None,
            generation: None,
            retry: None,
        }
    }

    fn method(lane: TaskLane, method: impl Into<String>) -> Self {
        Self {
            lane,
            method: Some(method.into()),
            document_uri: None,
            request_id: None,
            generation: None,
            retry: None,
        }
    }

    fn request(lane: TaskLane, request: TaskRequestMetadata, retry: Option<RetryTask>) -> Self {
        Self {
            lane,
            method: Some(request.method),
            document_uri: request.document_uri,
            request_id: Some(request.request_id),
            generation: Some(request.generation),
            retry,
        }
    }
}

impl TaskScheduler {
    pub(crate) fn new() -> Self {
        let (latency_jobs, latency_results) = spawn_lane_worker(TaskLane::Latency);
        let (formatting_jobs, formatting_results) = spawn_lane_worker(TaskLane::Formatting);
        let (worker_jobs, worker_results) = spawn_lane_worker(TaskLane::Worker);
        let (lifecycle_sender, lifecycle_events) = unbounded::<TaskLifecycleEvent>();
        Self {
            latency_jobs,
            formatting_jobs,
            worker_jobs,
            lifecycle_sender,
            lifecycle_events,
            latency_results,
            formatting_results,
            worker_results,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn spawn(
        &self,
        lane: TaskLane,
        job: impl FnOnce() -> Vec<Message> + Send + 'static,
    ) {
        self.spawn_labeled(TaskDescriptor::lane(lane), job);
    }

    #[allow(dead_code)]
    pub(crate) fn spawn_for_method(
        &self,
        lane: TaskLane,
        method: impl Into<String>,
        job: impl FnOnce() -> Vec<Message> + Send + 'static,
    ) {
        self.spawn_labeled(TaskDescriptor::method(lane, method), job);
    }

    #[allow(dead_code)]
    pub(crate) fn spawn_for_request(
        &self,
        lane: TaskLane,
        request: TaskRequestMetadata,
        job: impl FnOnce() -> Vec<Message> + Send + 'static,
    ) {
        self.spawn_labeled(TaskDescriptor::request(lane, request, None), job);
    }

    pub(crate) fn spawn_retryable_for_request(
        &self,
        lane: TaskLane,
        request: TaskRequestMetadata,
        retry: RetryTask,
        job: impl FnOnce() -> Vec<Message> + Send + 'static,
    ) {
        self.spawn_labeled(TaskDescriptor::request(lane, request, Some(retry)), job);
    }

    fn spawn_labeled(
        &self,
        descriptor: TaskDescriptor,
        job: impl FnOnce() -> Vec<Message> + Send + 'static,
    ) {
        let lane = descriptor.lane;
        let queued_ms = crate::profile::timestamp_ms();
        self.lifecycle_sender
            .send(TaskLifecycleEvent::queued(&descriptor, queued_ms))
            .expect("task lifecycle receiver should be alive");
        let lifecycle_sender = self.lifecycle_sender.clone();
        let task = Box::new(move || {
            let started_ms = crate::profile::timestamp_ms();
            let _ = lifecycle_sender.send(TaskLifecycleEvent::started(
                &descriptor,
                queued_ms,
                started_ms,
            ));
            let messages = job();
            let ended_ms = crate::profile::timestamp_ms();
            let _ = lifecycle_sender.send(TaskLifecycleEvent::ended(
                &descriptor,
                queued_ms,
                started_ms,
                ended_ms,
            ));
            TaskResult::timed_response(
                descriptor,
                TaskTiming::new(queued_ms, started_ms, ended_ms),
                messages,
            )
        });
        match lane {
            TaskLane::Latency => self.latency_jobs.send(task),
            TaskLane::Formatting => self.formatting_jobs.send(task),
            TaskLane::Worker => self.worker_jobs.send(task),
            TaskLane::Main => unreachable!("main-thread work should not be scheduled as a task"),
        }
        .expect("task lane worker should be alive");
    }

    pub(crate) const fn lifecycle_events(&self) -> &Receiver<TaskLifecycleEvent> {
        &self.lifecycle_events
    }

    pub(crate) const fn latency_results(&self) -> &Receiver<TaskResult> {
        &self.latency_results
    }

    pub(crate) const fn formatting_results(&self) -> &Receiver<TaskResult> {
        &self.formatting_results
    }

    pub(crate) const fn worker_results(&self) -> &Receiver<TaskResult> {
        &self.worker_results
    }
}

impl Default for TaskScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskResult {
    #[cfg(test)]
    pub(crate) fn response(result: JsonRpcResult) -> Self {
        Self::lane_response(TaskLane::Main, result)
    }

    #[cfg(test)]
    pub(crate) fn lane_response(lane: TaskLane, result: JsonRpcResult) -> Self {
        Self::lane_method_response(lane, None, result)
    }

    #[cfg(test)]
    pub(crate) fn lane_method_response(
        lane: TaskLane,
        method: Option<String>,
        result: JsonRpcResult,
    ) -> Self {
        Self::lane_method_request_generation_messages(
            lane,
            method,
            None,
            None,
            None,
            crate::legacy_rpc::typed_messages(result),
        )
    }

    #[cfg(test)]
    pub(crate) fn lane_method_request_generation_messages(
        lane: TaskLane,
        method: Option<String>,
        request_id: Option<RequestId>,
        generation: Option<GenerationToken>,
        retry: Option<RetryTask>,
        messages: Vec<Message>,
    ) -> Self {
        Self::Response {
            lane,
            method,
            document_uri: None,
            request_id,
            generation,
            retry: retry.map(Box::new),
            timing: None,
            messages,
        }
    }

    fn timed_response(
        descriptor: TaskDescriptor,
        timing: TaskTiming,
        messages: Vec<Message>,
    ) -> Self {
        Self::Response {
            lane: descriptor.lane,
            method: descriptor.method,
            document_uri: descriptor.document_uri,
            request_id: descriptor.request_id,
            generation: descriptor.generation,
            retry: descriptor.retry.map(Box::new),
            timing: Some(timing),
            messages,
        }
    }

    #[cfg(test)]
    pub(crate) fn timed_response_for_test(
        lane: TaskLane,
        method: Option<String>,
        document_uri: Option<String>,
        request_id: Option<RequestId>,
        timing: TaskTiming,
    ) -> Self {
        Self::timed_response(
            TaskDescriptor {
                lane,
                method,
                document_uri,
                request_id,
                generation: None,
                retry: None,
            },
            timing,
            Vec::new(),
        )
    }

    pub(crate) const fn lane(&self) -> TaskLane {
        match self {
            Self::Response { lane, .. } => *lane,
        }
    }

    pub(crate) fn method(&self) -> Option<&str> {
        match self {
            Self::Response { method, .. } => method.as_deref(),
        }
    }

    pub(crate) fn document_uri(&self) -> Option<&str> {
        match self {
            Self::Response { document_uri, .. } => document_uri.as_deref(),
        }
    }

    pub(crate) fn request_id(&self) -> Option<&RequestId> {
        match self {
            Self::Response { request_id, .. } => request_id.as_ref(),
        }
    }

    pub(crate) fn generation_token(&self) -> Option<&GenerationToken> {
        match self {
            Self::Response { generation, .. } => generation.as_ref(),
        }
    }

    pub(crate) fn retry(&self) -> Option<&RetryTask> {
        match self {
            Self::Response { retry, .. } => retry.as_deref(),
        }
    }

    pub(crate) const fn timing(&self) -> Option<TaskTiming> {
        match self {
            Self::Response { timing, .. } => *timing,
        }
    }

    pub(crate) fn into_messages(self) -> Vec<Message> {
        match self {
            Self::Response { messages, .. } => messages,
        }
    }
}

fn spawn_lane_worker(lane: TaskLane) -> (Sender<TaskJob>, Receiver<TaskResult>) {
    let (job_sender, job_receiver) = unbounded::<TaskJob>();
    let (result_sender, result_receiver) = unbounded::<TaskResult>();
    thread::Builder::new()
        .name(format!("VelaLsp{lane:?}Task"))
        .spawn(move || {
            while let Ok(job) = job_receiver.recv() {
                if result_sender.send(job()).is_err() {
                    break;
                }
            }
        })
        .expect("task lane worker should spawn");
    (job_sender, result_receiver)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_server::Response;
    use std::time::Duration;
    use vela_language_service::LanguageServiceDatabases;

    #[test]
    fn task_result_stores_typed_messages() {
        let result = JsonRpcResult::Response(test_response("main"));

        let task_result = TaskResult::response(result.clone());

        assert_eq!(task_result.lane(), TaskLane::Main);
        assert_eq!(
            TaskResult::lane_response(TaskLane::Latency, result.clone()).lane(),
            TaskLane::Latency
        );
        assert_eq!(
            TaskResult::lane_response(TaskLane::Formatting, result.clone()).lane(),
            TaskLane::Formatting
        );
        assert_eq!(
            TaskResult::lane_response(TaskLane::Worker, result.clone()).lane(),
            TaskLane::Worker
        );
        assert_eq!(
            TaskResult::lane_method_response(
                TaskLane::Worker,
                Some("textDocument/hover".to_owned()),
                result.clone(),
            )
            .method(),
            Some("textDocument/hover")
        );
        assert!(task_result.request_id().is_none());
        assert!(task_result.generation_token().is_none());
        assert_response_messages(task_result.into_messages(), test_response("main"));
    }

    #[test]
    fn task_scheduler_executes_lane_jobs_on_background_workers() {
        let scheduler = TaskScheduler::new();

        scheduler.spawn(TaskLane::Latency, || test_messages("latency"));
        scheduler.spawn(TaskLane::Formatting, || test_messages("formatting"));
        scheduler.spawn(TaskLane::Worker, || test_messages("worker"));

        let latency = scheduler
            .latency_results()
            .recv_timeout(Duration::from_secs(1))
            .expect("latency lane should respond");
        let formatting = scheduler
            .formatting_results()
            .recv_timeout(Duration::from_secs(1))
            .expect("formatting lane should respond");
        let worker = scheduler
            .worker_results()
            .recv_timeout(Duration::from_secs(1))
            .expect("worker lane should respond");

        assert_eq!(latency.lane(), TaskLane::Latency);
        assert_eq!(formatting.lane(), TaskLane::Formatting);
        assert_eq!(worker.lane(), TaskLane::Worker);
        assert_task_timing(latency.timing());
        assert_task_timing(formatting.timing());
        assert_task_timing(worker.timing());
        assert_response_messages(latency.into_messages(), test_response("latency"));
        assert_response_messages(formatting.into_messages(), test_response("formatting"));
        assert_response_messages(worker.into_messages(), test_response("worker"));
    }

    #[test]
    fn task_scheduler_preserves_method_names_for_profiled_tasks() {
        let scheduler = TaskScheduler::new();

        scheduler.spawn_for_method(TaskLane::Worker, "textDocument/references", || {
            assert_eq!(
                thread::current().name(),
                Some("VelaLspWorkerTask"),
                "worker thread should still identify the lane"
            );
            test_messages("references")
        });

        let task = scheduler
            .worker_results()
            .recv_timeout(Duration::from_secs(1))
            .expect("worker lane should respond");

        assert_eq!(task.lane(), TaskLane::Worker);
        assert_eq!(task.method(), Some("textDocument/references"));
        assert_task_timing(task.timing());
        assert_response_messages(task.into_messages(), test_response("references"));
    }

    #[test]
    fn task_scheduler_preserves_request_id_and_generation_token() {
        let scheduler = TaskScheduler::new();
        let databases = LanguageServiceDatabases::new();
        let (token, _handle) = databases.begin_cancellable_background_request();
        let request_id = RequestId::from("fmt-1".to_owned());

        let request = TaskRequestMetadata::new(
            "textDocument/formatting",
            Some("file:///workspace/scripts/main.vela".to_owned()),
            request_id.clone(),
            token.clone(),
        );
        scheduler.spawn_for_request(TaskLane::Formatting, request, || test_messages("formatted"));

        let task = scheduler
            .formatting_results()
            .recv_timeout(Duration::from_secs(1))
            .expect("formatting lane should respond");

        assert_eq!(task.lane(), TaskLane::Formatting);
        assert_eq!(task.method(), Some("textDocument/formatting"));
        assert_eq!(
            task.document_uri(),
            Some("file:///workspace/scripts/main.vela")
        );
        assert_eq!(task.request_id(), Some(&request_id));
        let generation = task
            .generation_token()
            .expect("request task should carry generation token");
        assert_eq!(generation.generation(), token.generation());
        assert!(!generation.is_cancelled());
        assert_task_timing(task.timing());
        assert_response_messages(task.into_messages(), test_response("formatted"));
    }

    fn assert_task_timing(timing: Option<TaskTiming>) {
        let timing = timing.expect("scheduled task should carry timing metadata");
        assert!(timing.queued_ms() <= timing.started_ms());
        assert!(timing.started_ms() <= timing.ended_ms());
    }

    fn assert_response_messages(messages: Vec<Message>, response: Response) {
        let expected = crate::rpc::serialize_message(&Message::Response(response));
        let actual = messages
            .iter()
            .map(crate::rpc::serialize_message)
            .collect::<Vec<_>>();
        assert_eq!(actual, vec![expected]);
    }

    fn test_messages(value: &str) -> Vec<Message> {
        vec![Message::Response(test_response(value))]
    }

    fn test_response(value: &str) -> Response {
        Response {
            id: RequestId::from(value.to_owned()),
            result: Some(serde_json::json!(value)),
            error: None,
        }
    }
}
