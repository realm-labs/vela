use std::fs::{File, OpenOptions};
use std::io::Write;

use crate::{
    LaunchConfiguration,
    profile::timestamp_ms,
    transport::{MessageMetadata, ResultSummary},
};

pub(crate) struct TraceSink {
    writer: Option<File>,
}

impl TraceSink {
    pub(crate) fn from_configuration(configuration: &LaunchConfiguration) -> anyhow::Result<Self> {
        let Some(path) = configuration.trace_log_path() else {
            return Ok(Self { writer: None });
        };
        let writer = OpenOptions::new().create(true).append(true).open(path)?;
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
        serde_json::to_writer(&mut *writer, &value)?;
        writer.write_all(b"\n")?;
        writer.flush()?;
        Ok(())
    }
}
