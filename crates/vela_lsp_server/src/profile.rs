use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::LaunchConfiguration;

pub(crate) trait ProfileMetadata {
    fn kind(&self) -> &'static str;
    fn method(&self) -> Option<&str>;
    fn id(&self) -> Option<&str>;
    fn document_uri(&self) -> Option<&str>;
}

pub(crate) fn timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis())
}

pub(crate) trait ProfileSummary {
    fn kind(&self) -> &'static str;
    fn messages(&self) -> usize;
    fn bytes(&self) -> usize;
}

pub(crate) struct RequestProfiler {
    writer: Option<File>,
    slow_ms: u64,
}

impl RequestProfiler {
    pub(crate) fn from_configuration(configuration: &LaunchConfiguration) -> io::Result<Self> {
        let Some(path) = configuration.profile_path() else {
            return Ok(Self {
                writer: None,
                slow_ms: configuration.profile_slow_ms(),
            });
        };
        let writer = OpenOptions::new().create(true).append(true).open(path)?;
        let mut profiler = Self {
            writer: Some(writer),
            slow_ms: configuration.profile_slow_ms(),
        };
        profiler.session_start(path)?;
        Ok(profiler)
    }

    fn session_start(&mut self, path: &str) -> io::Result<()> {
        self.write_json(serde_json::json!({
            "event": "session_start",
            "timestampMs": timestamp_ms(),
            "pid": std::process::id(),
            "profilePath": path,
            "slowMs": self.slow_ms,
            "transport": "lsp-server"
        }))
    }

    pub(crate) fn begin(
        &mut self,
        sequence: u64,
        metadata: &impl ProfileMetadata,
        input_bytes: usize,
    ) -> io::Result<()> {
        self.write_json(serde_json::json!({
            "event": "begin",
            "timestampMs": timestamp_ms(),
            "seq": sequence,
            "kind": metadata.kind(),
            "method": metadata.method(),
            "id": metadata.id(),
            "documentUri": metadata.document_uri(),
            "inputBytes": input_bytes
        }))
    }

    pub(crate) fn end(
        &mut self,
        sequence: u64,
        metadata: &impl ProfileMetadata,
        input_bytes: usize,
        handle_ms: u64,
        write_ms: u64,
        summary: &impl ProfileSummary,
    ) -> io::Result<()> {
        let total_ms = handle_ms.saturating_add(write_ms);
        self.write_json(serde_json::json!({
            "event": "end",
            "timestampMs": timestamp_ms(),
            "seq": sequence,
            "kind": metadata.kind(),
            "method": metadata.method(),
            "id": metadata.id(),
            "documentUri": metadata.document_uri(),
            "inputBytes": input_bytes,
            "resultKind": summary.kind(),
            "outputMessages": summary.messages(),
            "outputBytes": summary.bytes(),
            "handleMs": handle_ms,
            "writeMs": write_ms,
            "totalMs": total_ms,
            "slow": total_ms >= self.slow_ms
        }))
    }

    fn write_json(&mut self, value: serde_json::Value) -> io::Result<()> {
        let Some(writer) = self.writer.as_mut() else {
            return Ok(());
        };
        serde_json::to_writer(&mut *writer, &value).map_err(io::Error::other)?;
        writer.write_all(b"\n")?;
        writer.flush()?;
        Ok(())
    }
}
