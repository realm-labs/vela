use std::fs::{File, OpenOptions};
use std::io::{self, Write};

use crate::{
    LaunchConfiguration,
    profile::timestamp_ms,
    task::{TaskOutcome, TaskResult, TaskTiming},
    transport::{MessageMetadata, ResultSummary},
};

pub(crate) struct TraceSink {
    writer: Option<TraceWriter>,
}

enum TraceWriter {
    File(File),
    Stderr(io::Stderr),
}

#[derive(Debug, Clone)]
pub(crate) struct TaskTraceMetadata {
    method: Option<String>,
    id: Option<String>,
    document_uri: Option<String>,
    generation: Option<u64>,
    lane: &'static str,
    timing: Option<TaskTiming>,
}

impl TaskTraceMetadata {
    pub(crate) fn from_task(task: &TaskResult) -> Self {
        Self {
            method: task.method().map(str::to_owned),
            id: task.request_id().map(request_id_string),
            document_uri: task.document_uri().map(str::to_owned),
            generation: task
                .generation_token()
                .map(|token| token.generation().get()),
            lane: task.lane().as_trace_str(),
            timing: task.timing(),
        }
    }
}

impl TraceSink {
    pub(crate) fn from_configuration(configuration: &LaunchConfiguration) -> anyhow::Result<Self> {
        let Some(path) = configuration.trace_log_path() else {
            return Ok(Self { writer: None });
        };
        let writer = if path == "-" || path.eq_ignore_ascii_case("stderr") {
            TraceWriter::Stderr(io::stderr())
        } else {
            TraceWriter::File(OpenOptions::new().create(true).append(true).open(path)?)
        };
        let mut sink = Self {
            writer: Some(writer),
        };
        sink.session_start(path, configuration)?;
        Ok(sink)
    }

    pub(crate) fn message_received(
        &mut self,
        sequence: u64,
        metadata: &MessageMetadata,
        input_bytes: usize,
    ) -> anyhow::Result<()> {
        self.write_json(serde_json::json!({
            "event": "message_received",
            "timestampMs": timestamp_ms(),
            "seq": sequence,
            "kind": metadata.kind(),
            "method": metadata.method(),
            "id": metadata.id(),
            "documentUri": metadata.document_uri(),
            "inputBytes": input_bytes,
            "lane": "main",
            "status": "received"
        }))
    }

    pub(crate) fn response_sent(
        &mut self,
        sequence: u64,
        metadata: &MessageMetadata,
        handle_ms: u64,
        write_ms: u64,
        summary: &ResultSummary,
    ) -> anyhow::Result<()> {
        self.write_json(serde_json::json!({
            "event": "response_sent",
            "timestampMs": timestamp_ms(),
            "seq": sequence,
            "kind": metadata.kind(),
            "method": metadata.method(),
            "id": metadata.id(),
            "documentUri": metadata.document_uri(),
            "resultKind": summary.kind(),
            "outputMessages": summary.messages(),
            "outputBytes": summary.bytes(),
            "lane": "main",
            "handleMs": handle_ms,
            "writeMs": write_ms,
            "totalMs": handle_ms.saturating_add(write_ms),
            "status": "completed"
        }))
    }

    pub(crate) fn task_lifecycle(&mut self, task: &TaskResult) -> anyhow::Result<()> {
        let Some(timing) = task.timing() else {
            return Ok(());
        };
        let metadata = TaskTraceMetadata::from_task(task);
        self.task_event("request_queued", &metadata, timing, timing.queued_ms())?;
        self.task_event("task_started", &metadata, timing, timing.started_ms())?;
        self.task_event("task_ended", &metadata, timing, timing.ended_ms())
    }

    pub(crate) fn task_result(
        &mut self,
        metadata: &TaskTraceMetadata,
        outcome: TaskOutcome,
        write_ms: u64,
        summary: &ResultSummary,
    ) -> anyhow::Result<()> {
        if let Some(event) = task_outcome_event(outcome) {
            self.task_status_event(event, metadata, outcome, write_ms, summary)?;
        }
        self.write_json(serde_json::json!({
            "event": "response_sent",
            "timestampMs": timestamp_ms(),
            "kind": "task",
            "method": metadata.method.as_deref(),
            "id": metadata.id.as_deref(),
            "documentUri": metadata.document_uri.as_deref(),
            "generation": metadata.generation,
            "lane": metadata.lane,
            "resultKind": summary.kind(),
            "outputMessages": summary.messages(),
            "outputBytes": summary.bytes(),
            "status": task_outcome_status(outcome),
            "queueMs": task_queue_ms(metadata),
            "handleMs": task_handle_ms(metadata),
            "writeMs": write_ms,
            "totalMs": task_total_ms(metadata, write_ms)
        }))
    }

    fn session_start(
        &mut self,
        path: &str,
        configuration: &LaunchConfiguration,
    ) -> anyhow::Result<()> {
        self.write_json(serde_json::json!({
            "event": "session_start",
            "timestampMs": timestamp_ms(),
            "pid": std::process::id(),
            "transport": "lsp-server",
            "tracePath": path,
            "profilePath": configuration.profile_path(),
            "profileSlowMs": configuration.profile_slow_ms(),
            "watchFilesEnabled": configuration.watch_files_enabled(),
            "workspaceRoots": configuration.workspace_roots(),
            "hostSchema": configuration.host_schema()
        }))
    }

    fn task_event(
        &mut self,
        event: &str,
        metadata: &TaskTraceMetadata,
        timing: TaskTiming,
        timestamp_ms: u128,
    ) -> anyhow::Result<()> {
        self.write_json(serde_json::json!({
            "event": event,
            "timestampMs": timestamp_ms,
            "method": metadata.method.as_deref(),
            "id": metadata.id.as_deref(),
            "documentUri": metadata.document_uri.as_deref(),
            "generation": metadata.generation,
            "lane": metadata.lane,
            "status": task_lifecycle_status(event),
            "queueMs": timing.started_ms().saturating_sub(timing.queued_ms()),
            "handleMs": timing.ended_ms().saturating_sub(timing.started_ms())
        }))
    }

    fn task_status_event(
        &mut self,
        event: &str,
        metadata: &TaskTraceMetadata,
        outcome: TaskOutcome,
        write_ms: u64,
        summary: &ResultSummary,
    ) -> anyhow::Result<()> {
        self.write_json(serde_json::json!({
            "event": event,
            "timestampMs": timestamp_ms(),
            "method": metadata.method.as_deref(),
            "id": metadata.id.as_deref(),
            "documentUri": metadata.document_uri.as_deref(),
            "generation": metadata.generation,
            "lane": metadata.lane,
            "status": task_outcome_status(outcome),
            "queueMs": task_queue_ms(metadata),
            "handleMs": task_handle_ms(metadata),
            "writeMs": write_ms,
            "totalMs": task_total_ms(metadata, write_ms),
            "outputMessages": summary.messages(),
            "outputBytes": summary.bytes()
        }))
    }

    fn write_json(&mut self, value: serde_json::Value) -> anyhow::Result<()> {
        let Some(writer) = self.writer.as_mut() else {
            return Ok(());
        };
        match writer {
            TraceWriter::File(writer) => write_json_line(writer, &value)?,
            TraceWriter::Stderr(writer) => write_json_line(writer, &value)?,
        }
        Ok(())
    }
}

fn write_json_line(writer: &mut impl Write, value: &serde_json::Value) -> anyhow::Result<()> {
    serde_json::to_writer(&mut *writer, value)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

fn request_id_string(id: &lsp_server::RequestId) -> String {
    match serde_json::to_value(id) {
        Ok(value) => value
            .as_str()
            .map(str::to_owned)
            .or_else(|| value.as_i64().map(|id| id.to_string()))
            .unwrap_or_else(|| id.to_string()),
        Err(_) => id.to_string(),
    }
}

fn task_outcome_event(outcome: TaskOutcome) -> Option<&'static str> {
    match outcome {
        TaskOutcome::Completed => None,
        TaskOutcome::Cancelled => Some("request_cancelled"),
        TaskOutcome::StaleDiscarded => Some("request_stale"),
        TaskOutcome::Retried => Some("request_retried"),
    }
}

fn task_outcome_status(outcome: TaskOutcome) -> &'static str {
    match outcome {
        TaskOutcome::Completed => "completed",
        TaskOutcome::Cancelled => "cancelled",
        TaskOutcome::StaleDiscarded => "stale_discarded",
        TaskOutcome::Retried => "retried",
    }
}

fn task_lifecycle_status(event: &str) -> &'static str {
    match event {
        "request_queued" => "queued",
        "task_started" => "started",
        "task_ended" => "ended",
        _ => "unknown",
    }
}

fn task_queue_ms(metadata: &TaskTraceMetadata) -> Option<u128> {
    metadata
        .timing
        .map(|timing| timing.started_ms().saturating_sub(timing.queued_ms()))
}

fn task_handle_ms(metadata: &TaskTraceMetadata) -> Option<u128> {
    metadata
        .timing
        .map(|timing| timing.ended_ms().saturating_sub(timing.started_ms()))
}

fn task_total_ms(metadata: &TaskTraceMetadata, write_ms: u64) -> Option<u128> {
    metadata.timing.map(|timing| {
        timing
            .ended_ms()
            .saturating_sub(timing.queued_ms())
            .saturating_add(u128::from(write_ms))
    })
}

#[cfg(test)]
mod tests {
    use lsp_server::{Message, Request, RequestId};
    use std::fs;

    use crate::{
        LaunchConfiguration,
        task::{TaskLane, TaskOutcome, TaskResult, TaskTiming},
        transport::{MessageMetadata, ResultSummary},
    };

    use super::{TaskTraceMetadata, TraceSink};

    #[test]
    fn trace_sink_accepts_stderr_destination() {
        let mut configuration = LaunchConfiguration::new();
        configuration.set_trace_log_path("-");
        let mut trace =
            TraceSink::from_configuration(&configuration).expect("stderr trace should open");
        let message = Message::Request(Request {
            id: RequestId::from(1),
            method: "initialize".to_owned(),
            params: serde_json::Value::Null,
        });
        let metadata = MessageMetadata::from_message(&message);
        trace
            .message_received(1, &metadata, 64)
            .expect("stderr trace should write message receipt");
        trace
            .response_sent(1, &metadata, 5, 2, &ResultSummary::from_messages(&[]))
            .expect("stderr trace should write response summary");
    }

    #[test]
    fn trace_sink_writes_task_lifecycle_events() {
        let path = temp_trace_path("task-lifecycle");
        let mut configuration = LaunchConfiguration::new();
        configuration.set_trace_log_path(path.to_string_lossy().into_owned());
        let mut trace =
            TraceSink::from_configuration(&configuration).expect("file trace should open");
        let task = TaskResult::timed_response_for_test(
            TaskLane::Worker,
            Some("textDocument/references".to_owned()),
            Some("file:///workspace/scripts/main.vela".to_owned()),
            Some(RequestId::from(7)),
            TaskTiming::new(10, 14, 25),
        );

        trace
            .task_lifecycle(&task)
            .expect("task lifecycle trace should write");

        let output = fs::read_to_string(&path).expect("trace file should be readable");
        let events = output
            .lines()
            .map(|line| {
                serde_json::from_str::<serde_json::Value>(line)
                    .expect("trace line should be valid JSON")
            })
            .collect::<Vec<_>>();
        assert!(
            events
                .iter()
                .any(|event| event["event"] == "request_queued")
        );
        assert!(events.iter().any(|event| event["event"] == "task_started"));
        let task_ended = events
            .iter()
            .find(|event| event["event"] == "task_ended")
            .expect("task_ended event should be written");
        assert_eq!(task_ended["method"], "textDocument/references");
        assert_eq!(task_ended["id"], "7");
        assert_eq!(
            task_ended["documentUri"],
            "file:///workspace/scripts/main.vela"
        );
        assert_eq!(task_ended["lane"], "worker");
        assert_eq!(task_ended["queueMs"], 4);
        assert_eq!(task_ended["handleMs"], 11);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn trace_sink_writes_task_result_status_events() {
        let path = temp_trace_path("task-result-status");
        let mut configuration = LaunchConfiguration::new();
        configuration.set_trace_log_path(path.to_string_lossy().into_owned());
        let mut trace =
            TraceSink::from_configuration(&configuration).expect("file trace should open");
        let task = TaskResult::timed_response_for_test(
            TaskLane::Formatting,
            Some("textDocument/formatting".to_owned()),
            Some("file:///workspace/scripts/main.vela".to_owned()),
            Some(RequestId::from("fmt-1".to_owned())),
            TaskTiming::new(1, 2, 3),
        );
        let metadata = TaskTraceMetadata::from_task(&task);
        let summary = ResultSummary::from_messages(&[]);

        trace
            .task_result(&metadata, TaskOutcome::Cancelled, 4, &summary)
            .expect("cancelled task status should write");
        trace
            .task_result(&metadata, TaskOutcome::StaleDiscarded, 4, &summary)
            .expect("stale task status should write");
        trace
            .task_result(&metadata, TaskOutcome::Retried, 4, &summary)
            .expect("retried task status should write");

        let output = fs::read_to_string(&path).expect("trace file should be readable");
        let events = output
            .lines()
            .map(|line| {
                serde_json::from_str::<serde_json::Value>(line)
                    .expect("trace line should be valid JSON")
            })
            .collect::<Vec<_>>();
        assert!(
            events.iter().any(
                |event| event["event"] == "request_cancelled" && event["status"] == "cancelled"
            )
        );
        assert!(
            events
                .iter()
                .any(|event| event["event"] == "request_stale"
                    && event["status"] == "stale_discarded")
        );
        assert!(
            events
                .iter()
                .any(|event| event["event"] == "request_retried" && event["status"] == "retried")
        );
        assert!(events.iter().any(|event| event["event"] == "response_sent"
            && event["kind"] == "task"
            && event["lane"] == "formatting"
            && event["id"] == "fmt-1"
            && event["documentUri"] == "file:///workspace/scripts/main.vela"
            && event["queueMs"] == 1
            && event["handleMs"] == 1
            && event["writeMs"] == 4
            && event["totalMs"] == 6));

        let _ = fs::remove_file(path);
    }

    fn temp_trace_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "vela-lsp-{name}-{}-{}.jsonl",
            std::process::id(),
            crate::profile::timestamp_ms()
        ))
    }
}
