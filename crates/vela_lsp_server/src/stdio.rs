use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, Write};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::{JsonRpcResult, LaunchConfiguration, LspServer, rpc};

pub fn run_stdio<R, W>(reader: R, writer: W) -> io::Result<()>
where
    R: BufRead,
    W: Write,
{
    run_stdio_with_configuration(reader, writer, LaunchConfiguration::new())
}

pub fn run_stdio_with_configuration<R, W>(
    reader: R,
    writer: W,
    configuration: LaunchConfiguration,
) -> io::Result<()>
where
    R: BufRead,
    W: Write,
{
    let mut transport = StdioTransport::new(reader, writer);
    let mut profiler = RequestProfiler::from_configuration(&configuration)?;
    let mut server = LspServer::with_launch_configuration(configuration);
    let mut sequence = 0_u64;
    while let Some(message) = transport.read_message()? {
        sequence = sequence.saturating_add(1);
        let metadata = MessageMetadata::from_json(&message);
        let input_bytes = message.len();
        profiler.begin(sequence, &metadata, input_bytes)?;
        let handle_start = Instant::now();
        let result = server.handle_json(&message);
        let handle_ms = elapsed_ms(handle_start);
        let summary = ResultSummary::from_result(&result);
        let write_start = Instant::now();
        transport.write_result(result)?;
        transport.flush()?;
        let write_ms = elapsed_ms(write_start);
        profiler.end(
            sequence,
            &metadata,
            input_bytes,
            handle_ms,
            write_ms,
            &summary,
        )?;
        if server.is_exited() {
            break;
        }
    }
    transport.flush()
}

struct StdioTransport<R, W> {
    reader: R,
    writer: W,
}

impl<R, W> StdioTransport<R, W>
where
    R: BufRead,
    W: Write,
{
    fn new(reader: R, writer: W) -> Self {
        Self { reader, writer }
    }

    fn read_message(&mut self) -> io::Result<Option<String>> {
        let Some(content_length) = self.read_content_length()? else {
            return Ok(None);
        };
        let mut body = vec![0_u8; content_length];
        self.reader.read_exact(&mut body)?;
        String::from_utf8(body)
            .map(Some)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }

    fn read_content_length(&mut self) -> io::Result<Option<usize>> {
        let mut content_length: Option<usize> = None;
        let mut line = String::new();
        loop {
            line.clear();
            let bytes = self.reader.read_line(&mut line)?;
            if bytes == 0 {
                return Ok(None);
            }
            let trimmed = line.trim_end_matches(['\r', '\n']);
            if trimmed.is_empty() {
                return content_length.map(Some).ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "missing Content-Length header")
                });
            }
            let Some((name, value)) = trimmed.split_once(':') else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("invalid LSP header `{trimmed}`"),
                ));
            };
            if name.eq_ignore_ascii_case("Content-Length") {
                let length = value.trim().parse::<usize>().map_err(|error| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("invalid Content-Length `{}`: {error}", value.trim()),
                    )
                })?;
                content_length = Some(length);
            }
        }
    }

    fn write_result(&mut self, result: JsonRpcResult) -> io::Result<()> {
        match result {
            JsonRpcResult::Response(response) => self.write_message(&rpc::serialize_message(
                &lsp_server::Message::Response(response),
            )),
            JsonRpcResult::Notification(message) => {
                self.write_message(&rpc::serialize_message(&message))
            }
            JsonRpcResult::Notifications(messages) => {
                for message in messages {
                    self.write_message(&rpc::serialize_message(&message))?;
                }
                Ok(())
            }
            JsonRpcResult::None => Ok(()),
        }
    }

    fn write_message(&mut self, message: &str) -> io::Result<()> {
        write!(
            self.writer,
            "Content-Length: {}\r\n\r\n{}",
            message.len(),
            message
        )
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

struct RequestProfiler {
    writer: Option<File>,
    slow_ms: u64,
}

impl RequestProfiler {
    fn from_configuration(configuration: &LaunchConfiguration) -> io::Result<Self> {
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
            "slowMs": self.slow_ms
        }))
    }

    fn begin(
        &mut self,
        sequence: u64,
        metadata: &MessageMetadata,
        input_bytes: usize,
    ) -> io::Result<()> {
        self.write_json(serde_json::json!({
            "event": "begin",
            "timestampMs": timestamp_ms(),
            "seq": sequence,
            "kind": metadata.kind,
            "method": metadata.method,
            "id": metadata.id,
            "documentUri": metadata.document_uri,
            "inputBytes": input_bytes
        }))
    }

    fn end(
        &mut self,
        sequence: u64,
        metadata: &MessageMetadata,
        input_bytes: usize,
        handle_ms: u64,
        write_ms: u64,
        summary: &ResultSummary,
    ) -> io::Result<()> {
        let total_ms = handle_ms.saturating_add(write_ms);
        self.write_json(serde_json::json!({
            "event": "end",
            "timestampMs": timestamp_ms(),
            "seq": sequence,
            "kind": metadata.kind,
            "method": metadata.method,
            "id": metadata.id,
            "documentUri": metadata.document_uri,
            "inputBytes": input_bytes,
            "resultKind": summary.kind,
            "outputMessages": summary.messages,
            "outputBytes": summary.bytes,
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

#[derive(Debug)]
struct MessageMetadata {
    kind: &'static str,
    method: Option<String>,
    id: Option<String>,
    document_uri: Option<String>,
}

impl MessageMetadata {
    fn from_json(message: &str) -> Self {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(message) else {
            return Self {
                kind: "invalid",
                method: None,
                id: None,
                document_uri: None,
            };
        };
        let method = value
            .get("method")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        let id = value.get("id").map(serde_json::Value::to_string);
        let kind = match (method.as_ref(), id.as_ref()) {
            (Some(_), Some(_)) => "request",
            (Some(_), None) => "notification",
            (None, Some(_)) if value.get("result").is_some() => "response",
            (None, Some(_)) if value.get("error").is_some() => "json_rpc_error",
            _ => "unknown",
        };
        Self {
            kind,
            method,
            id,
            document_uri: document_uri(&value),
        }
    }
}

struct ResultSummary {
    kind: &'static str,
    messages: usize,
    bytes: usize,
}

impl ResultSummary {
    const fn none() -> Self {
        Self {
            kind: "none",
            messages: 0,
            bytes: 0,
        }
    }

    fn from_result(result: &JsonRpcResult) -> Self {
        match result {
            JsonRpcResult::Response(message) => Self {
                kind: "response",
                messages: 1,
                bytes: rpc::serialize_message(&lsp_server::Message::Response(message.clone()))
                    .len(),
            },
            JsonRpcResult::Notification(message) => Self {
                kind: "notification",
                messages: 1,
                bytes: rpc::serialize_message(message).len(),
            },
            JsonRpcResult::Notifications(messages) => Self {
                kind: "notifications",
                messages: messages.len(),
                bytes: messages
                    .iter()
                    .map(|message| rpc::serialize_message(message).len())
                    .sum(),
            },
            JsonRpcResult::None => Self::none(),
        }
    }
}

fn document_uri(value: &serde_json::Value) -> Option<String> {
    value
        .pointer("/params/textDocument/uri")
        .or_else(|| value.pointer("/params/uri"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}

fn elapsed_ms(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::{self, Cursor, Write};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::LaunchConfiguration;

    #[test]
    fn stdio_flushes_after_each_response_before_stream_end() {
        let initialize = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "processId": null,
                "capabilities": {}
            }
        })
        .to_string();
        let input = format!("Content-Length: {}\r\n\r\n{initialize}", initialize.len());
        let mut writer = FlushCountingWriter::default();

        super::run_stdio(Cursor::new(input.into_bytes()), &mut writer)
            .expect("stdio transport should flush initialize response");

        assert!(
            writer.flush_count >= 2,
            "expected one flush after the response and one final flush, got {}",
            writer.flush_count
        );
        assert!(
            String::from_utf8_lossy(&writer.bytes).contains("\"id\":1"),
            "initialize response should be written"
        );
    }

    #[test]
    fn stdio_profile_writes_begin_and_end_events() {
        let initialize = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "processId": null,
                "capabilities": {}
            }
        })
        .to_string();
        let input = format!("Content-Length: {}\r\n\r\n{initialize}", initialize.len());
        let profile_path = temp_profile_path();
        let _ = fs::remove_file(&profile_path);
        let mut configuration = LaunchConfiguration::new();
        configuration.set_profile_path(profile_path.display().to_string());
        configuration.set_profile_slow_ms(0);
        let mut writer = FlushCountingWriter::default();

        super::run_stdio_with_configuration(
            Cursor::new(input.into_bytes()),
            &mut writer,
            configuration,
        )
        .expect("stdio transport should write profile events");

        let profile = fs::read_to_string(&profile_path).expect("profile should be readable");
        let events = profile
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("valid jsonl"))
            .collect::<Vec<_>>();
        let _ = fs::remove_file(&profile_path);

        assert_eq!(events[0]["event"], "session_start");
        assert_eq!(events[1]["event"], "begin");
        assert_eq!(events[1]["seq"], 1);
        assert_eq!(events[1]["kind"], "request");
        assert_eq!(events[1]["method"], "initialize");
        assert_eq!(events[1]["id"], "1");
        assert_eq!(events[2]["event"], "end");
        assert_eq!(events[2]["resultKind"], "response");
        assert_eq!(events[2]["outputMessages"], 1);
        assert_eq!(events[2]["slow"], true);
        assert!(
            events[2]["outputBytes"].as_u64().unwrap_or_default() > 0,
            "{events:?}"
        );
    }

    fn temp_profile_path() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        std::env::temp_dir().join(format!(
            "vela_lsp_profile_{}_{}.jsonl",
            std::process::id(),
            suffix
        ))
    }

    #[derive(Default)]
    struct FlushCountingWriter {
        bytes: Vec<u8>,
        flush_count: usize,
    }

    impl Write for FlushCountingWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.bytes.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            self.flush_count += 1;
            Ok(())
        }
    }
}
