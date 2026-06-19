use std::fs::{File, OpenOptions};
use std::io::{self, Write};

use crate::{
    LaunchConfiguration,
    profile::timestamp_ms,
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

#[cfg(test)]
mod tests {
    use lsp_server::{Message, Request, RequestId};

    use crate::{
        LaunchConfiguration,
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
}
