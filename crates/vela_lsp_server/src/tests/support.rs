use std::time::Duration;

use crossbeam_channel::{Receiver, select, unbounded};
use lsp_server::{Message, Notification, Request as ServerRequest, RequestId, Response};
use lsp_types::{
    InitializeParams, InitializeResult,
    notification::Notification as LspNotification,
    request::{Initialize, Request as LspRequest},
};
use serde::{Serialize, de::DeserializeOwned};

use crate::{
    LaunchConfiguration,
    global_state::{GlobalState, GlobalStateSnapshot},
    task::{TaskOutcome, TaskResult},
};

const TASK_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) struct TestServer {
    state: GlobalState,
    outbound: Receiver<Message>,
    next_request_id: i32,
}

impl TestServer {
    pub(crate) fn new() -> Self {
        Self::with_launch_configuration(LaunchConfiguration::default())
    }

    pub(crate) fn with_launch_configuration(configuration: LaunchConfiguration) -> Self {
        let (sender, outbound) = unbounded();
        Self {
            state: GlobalState::new(sender, configuration),
            outbound,
            next_request_id: 0,
        }
    }

    pub(crate) fn initialize(&mut self, params: InitializeParams) -> InitializeResult {
        self.request::<Initialize>(params)
    }

    pub(crate) fn request<R>(&mut self, params: R::Params) -> R::Result
    where
        R: LspRequest,
        R::Params: Serialize,
        R::Result: DeserializeOwned,
    {
        let response = self.request_response::<R>(params);
        let result = response.error.map_or_else(
            || {
                response
                    .result
                    .expect("successful response should contain a result")
            },
            |error| panic!("{} request failed: {error:?}", R::METHOD),
        );
        serde_json::from_value(result)
            .unwrap_or_else(|error| panic!("{} response should be typed: {error}", R::METHOD))
    }

    pub(crate) fn request_response<R>(&mut self, params: R::Params) -> Response
    where
        R: LspRequest,
        R::Params: Serialize,
    {
        let id = self.next_id();
        let message = Message::Request(ServerRequest {
            id: id.clone(),
            method: R::METHOD.to_owned(),
            params: serde_json::to_value(params)
                .unwrap_or_else(|error| panic!("{} params should serialize: {error}", R::METHOD)),
        });
        let messages = self.process(message, Some(&id));
        messages
            .into_iter()
            .find_map(|message| match message {
                Message::Response(response) if response.id == id => Some(response),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{} should emit a response", R::METHOD))
    }

    pub(crate) fn request_messages<R>(&mut self, id: i32, params: R::Params) -> Vec<Message>
    where
        R: LspRequest,
        R::Params: Serialize,
    {
        let id = RequestId::from(id);
        self.process(
            Message::Request(ServerRequest {
                id: id.clone(),
                method: R::METHOD.to_owned(),
                params: serde_json::to_value(params).unwrap_or_else(|error| {
                    panic!("{} params should serialize: {error}", R::METHOD)
                }),
            }),
            Some(&id),
        )
    }

    pub(crate) fn notify<N>(&mut self, params: N::Params) -> Vec<Message>
    where
        N: LspNotification,
        N::Params: Serialize,
    {
        let message = Message::Notification(Notification {
            method: N::METHOD.to_owned(),
            params: serde_json::to_value(params)
                .unwrap_or_else(|error| panic!("{} params should serialize: {error}", N::METHOD)),
        });
        self.process(message, None)
    }

    pub(crate) fn send_protocol_message(&mut self, message: Message) -> Vec<Message> {
        let response_id = match &message {
            Message::Request(request) => Some(request.id.clone()),
            Message::Notification(_) | Message::Response(_) => None,
        };
        self.process(message, response_id.as_ref())
    }

    pub(crate) const fn state(&self) -> &GlobalState {
        &self.state
    }

    pub(crate) fn snapshot(&self) -> GlobalStateSnapshot {
        self.state.snapshot()
    }

    // Separate scheduling from publication so lifecycle tests can interleave
    // real notifications deterministically, without timing-dependent sleeps.
    pub(crate) fn queue_request(&mut self, id: i32, method: &str, params: serde_json::Value) {
        let messages = self.process(
            Message::Request(ServerRequest {
                id: RequestId::from(id),
                method: method.to_owned(),
                params,
            }),
            None,
        );
        assert!(
            messages.is_empty(),
            "queued request published early: {messages:?}"
        );
    }

    pub(crate) fn receive_task(&self) -> TaskResult {
        self.recv_task()
            .expect("queued task must finish within five seconds")
    }

    pub(crate) fn publish_task(&mut self, task: TaskResult) -> (TaskOutcome, Vec<Message>) {
        let summary = self.state.send_task_result(task).expect("task publication");
        (summary.outcome(), self.collect_outbound())
    }

    fn next_id(&mut self) -> RequestId {
        let id = RequestId::from(self.next_request_id);
        self.next_request_id = self.next_request_id.saturating_add(1);
        id
    }

    fn process(&mut self, message: Message, response_id: Option<&RequestId>) -> Vec<Message> {
        if self.state.is_exited() {
            return Vec::new();
        }
        let messages = self
            .state
            .handle_message(&message)
            .expect("production message dispatch should succeed");
        self.state
            .send_messages(messages)
            .expect("in-memory outbound channel should remain open");

        let mut outbound = self.collect_outbound();
        while response_id.is_some_and(|id| !contains_response(&outbound, id)) {
            let task = self
                .recv_task()
                .unwrap_or_else(|| panic!("request task did not finish within {TASK_TIMEOUT:?}"));
            self.state
                .send_task_result(task)
                .expect("in-memory task response should send");
            outbound.extend(self.collect_outbound());
        }
        outbound
    }

    fn collect_outbound(&self) -> Vec<Message> {
        self.outbound.try_iter().collect()
    }

    fn recv_task(&self) -> Option<TaskResult> {
        select! {
            recv(self.state.task_scheduler().latency_results()) -> task => task.ok(),
            recv(self.state.task_scheduler().formatting_results()) -> task => task.ok(),
            recv(self.state.task_scheduler().worker_results()) -> task => task.ok(),
            default(TASK_TIMEOUT) => None,
        }
    }
}

impl Default for TestServer {
    fn default() -> Self {
        Self::new()
    }
}

fn contains_response(messages: &[Message], id: &RequestId) -> bool {
    messages
        .iter()
        .any(|message| matches!(message, Message::Response(response) if &response.id == id))
}

#[cfg(test)]
mod tests {
    use lsp_types::{
        ClientCapabilities, InitializeParams, InitializedParams,
        notification::{Exit, Initialized},
        request::Shutdown,
    };

    use super::TestServer;

    #[test]
    fn typed_harness_uses_production_lifecycle_and_response_path() {
        let mut server = TestServer::new();
        let result = server.initialize(InitializeParams {
            capabilities: ClientCapabilities::default(),
            ..InitializeParams::default()
        });
        assert_eq!(
            result.server_info.expect("server info").name,
            "vela_lsp_server"
        );

        assert!(
            server
                .notify::<Initialized>(InitializedParams {})
                .is_empty()
        );
        let shutdown = server.request_response::<Shutdown>(());
        assert!(shutdown.error.is_none());
        assert!(server.notify::<Exit>(()).is_empty());
        assert!(server.state().is_exited());
    }
}

/// Test roots must not rely on wall-clock resolution for isolation.
pub(crate) fn unique_temp_root(label: &str) -> std::path::PathBuf {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    unique_temp_root_at(label, timestamp)
}

fn unique_temp_root_at(label: &str, timestamp: u128) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);
    let sequence = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "vela-lsp-{label}-{}-{timestamp}-{sequence}",
        std::process::id()
    ));
    std::fs::create_dir(&root).expect("unique fixture root must be new");
    root
}

#[test]
fn parallel_fixture_roots_are_unique_with_a_frozen_clock() {
    let roots = std::thread::scope(|scope| {
        let tasks = (0..32)
            .map(|_| scope.spawn(|| unique_temp_root_at("frozen", 0)))
            .collect::<Vec<_>>();
        tasks
            .into_iter()
            .map(|task| task.join().expect("fixture allocation"))
            .collect::<Vec<_>>()
    });
    let unique = roots.iter().collect::<std::collections::BTreeSet<_>>();
    assert_eq!(unique.len(), 32);
    for root in roots {
        std::fs::remove_dir(root).expect("remove fixture root");
    }
}
