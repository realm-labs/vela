use std::fs::{File, OpenOptions};
use std::io::{self, Write};

use crate::{
    LaunchConfiguration,
    profile::timestamp_ms,
    task::{TaskResult, TaskTiming},
    transport::{MessageMetadata, ResultSummary},
};

pub(crate) struct TraceSink {
    writer: Option<TraceWriter>,
}

enum TraceWriter {
    File(File),
    Stderr(io::Stderr),
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
            "lane": "main"
        }))
    }

    pub(crate) fn response_sent(
        &mut self,
        sequence: u64,
        metadata: &MessageMetadata,
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
            "lane": "main"
        }))
    }

    pub(crate) fn task_lifecycle(&mut self, task: &TaskResult) -> anyhow::Result<()> {
        let Some(timing) = task.timing() else {
            return Ok(());
        };
        self.task_event("request_queued", task, timing, timing.queued_ms())?;
        self.task_event("task_started", task, timing, timing.started_ms())?;
        self.task_event("task_ended", task, timing, timing.ended_ms())
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
        task: &TaskResult,
        timing: TaskTiming,
        timestamp_ms: u128,
    ) -> anyhow::Result<()> {
        let generation = task
            .generation_token()
            .map(|token| token.generation().get());
        self.write_json(serde_json::json!({
            "event": event,
            "timestampMs": timestamp_ms,
            "method": task.method(),
            "id": task.request_id().map(request_id_string),
            "generation": generation,
            "lane": task.lane().as_trace_str(),
            "queueMs": timing.started_ms().saturating_sub(timing.queued_ms()),
            "handleMs": timing.ended_ms().saturating_sub(timing.started_ms())
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
    id.to_string()
}

#[cfg(test)]
mod tests {
    use lsp_server::{Message, Request, RequestId};
    use std::fs;

    use crate::{
        LaunchConfiguration,
        task::{TaskLane, TaskResult, TaskTiming},
        transport::{MessageMetadata, ResultSummary},
    };

    use super::TraceSink;

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
            .response_sent(1, &metadata, &ResultSummary::from_messages(&[]))
            .expect("stderr trace should write response summary");
    }

    #[test]
    fn trace_sink_writes_task_lifecycle_events() {
        let path = temp_trace_path("task-lifecycle");
        let mut configuration = LaunchConfiguration::new();
        configuration.set_trace_log_path(path.to_string_lossy().into_owned());
        let mut trace =
            TraceSink::from_configuration(&configuration).expect("file trace should open");
        let task = TaskResult::lane_method_request_generation_timed_messages(
            TaskLane::Worker,
            Some("textDocument/references".to_owned()),
            Some(RequestId::from(7)),
            None,
            None,
            TaskTiming::new(10, 14, 25),
            Vec::new(),
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
        assert_eq!(task_ended["lane"], "worker");
        assert_eq!(task_ended["queueMs"], 4);
        assert_eq!(task_ended["handleMs"], 11);

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
