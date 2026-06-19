use std::fs::{File, OpenOptions};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use lsp_server::{Connection, Message, Notification, RequestId, Response, ResponseError};

use crate::{JsonRpcResult, LaunchConfiguration, LspServer};

pub fn run_connection(
    connection: Connection,
    configuration: LaunchConfiguration,
) -> anyhow::Result<()> {
    let mut server = LspServer::with_launch_configuration(configuration.clone());
    let mut profiler = RequestProfiler::from_configuration(&configuration)?;
    let mut sequence = 0_u64;

    while let Ok(message) = connection.receiver.recv() {
        sequence = sequence.saturating_add(1);
        let metadata = MessageMetadata::from_message(&message);
        let input = serialize_json_rpc_message(&message)?;
        let input_bytes = input.len();
        profiler.begin(sequence, &metadata, input_bytes)?;

        let handle_start = Instant::now();
        let result = server.handle_json(&input);
        let handle_ms = elapsed_ms(handle_start);
        let summary = ResultSummary::from_result(&result);

        let write_start = Instant::now();
        for message in messages_from_result(result)? {
            connection.sender.send(message)?;
        }
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

    Ok(())
}

fn messages_from_result(result: JsonRpcResult) -> anyhow::Result<Vec<Message>> {
    let strings = match result {
        JsonRpcResult::Response(message) | JsonRpcResult::Notification(message) => vec![message],
        JsonRpcResult::Notifications(messages) => messages,
        JsonRpcResult::None => Vec::new(),
    };
    strings
        .into_iter()
        .map(|message| {
            let value = serde_json::from_str::<serde_json::Value>(&message)?;
            message_from_json_rpc(value)
        })
        .collect()
}

fn serialize_json_rpc_message(message: &Message) -> anyhow::Result<String> {
    let mut value = serde_json::to_value(message)?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("LSP message did not serialize to an object"))?;
    object.insert(
        "jsonrpc".to_owned(),
        serde_json::Value::String("2.0".to_owned()),
    );
    serde_json::to_string(&value).map_err(Into::into)
}

fn message_from_json_rpc(value: serde_json::Value) -> anyhow::Result<Message> {
    if value.get("method").is_some() {
        let method = value
            .get("method")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("JSON-RPC notification is missing method"))?
            .to_owned();
        let params = value
            .get("params")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        return Ok(Message::Notification(Notification { method, params }));
    }

    let id = value
        .get("id")
        .ok_or_else(|| anyhow::anyhow!("JSON-RPC response is missing id"))
        .and_then(request_id_from_json)?;
    let result = value.get("result").cloned();
    let error = value
        .get("error")
        .cloned()
        .map(serde_json::from_value::<ResponseError>)
        .transpose()?;
    Ok(Message::Response(Response { id, result, error }))
}

fn request_id_from_json(value: &serde_json::Value) -> anyhow::Result<RequestId> {
    if let Some(id) = value.as_i64() {
        let id = i32::try_from(id)?;
        return Ok(RequestId::from(id));
    }
    if let Some(id) = value.as_str() {
        return Ok(RequestId::from(id.to_owned()));
    }
    anyhow::bail!("unsupported JSON-RPC response id `{value}`")
}

struct RequestProfiler {
    writer: Option<File>,
    slow_ms: u64,
}

impl RequestProfiler {
    fn from_configuration(configuration: &LaunchConfiguration) -> anyhow::Result<Self> {
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

    fn session_start(&mut self, path: &str) -> anyhow::Result<()> {
        self.write_json(serde_json::json!({
            "event": "session_start",
            "timestampMs": timestamp_ms(),
            "pid": std::process::id(),
            "profilePath": path,
            "slowMs": self.slow_ms,
            "transport": "lsp-server"
        }))
    }

    fn begin(
        &mut self,
        sequence: u64,
        metadata: &MessageMetadata,
        input_bytes: usize,
    ) -> anyhow::Result<()> {
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
    ) -> anyhow::Result<()> {
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

    fn write_json(&mut self, value: serde_json::Value) -> anyhow::Result<()> {
        let Some(writer) = self.writer.as_mut() else {
            return Ok(());
        };
        serde_json::to_writer(&mut *writer, &value)?;
        use std::io::Write as _;
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
    fn from_message(message: &Message) -> Self {
        match message {
            Message::Request(request) => Self {
                kind: "request",
                method: Some(request.method.clone()),
                id: Some(request.id.to_string()),
                document_uri: document_uri(&request.params),
            },
            Message::Notification(notification) => Self {
                kind: "notification",
                method: Some(notification.method.clone()),
                id: None,
                document_uri: document_uri(&notification.params),
            },
            Message::Response(response) => Self {
                kind: if response.error.is_some() {
                    "error_response"
                } else {
                    "response"
                },
                method: None,
                id: Some(response.id.to_string()),
                document_uri: response.result.as_ref().and_then(document_uri),
            },
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
                bytes: message.len(),
            },
            JsonRpcResult::Notification(message) => Self {
                kind: "notification",
                messages: 1,
                bytes: message.len(),
            },
            JsonRpcResult::Notifications(messages) => Self {
                kind: "notifications",
                messages: messages.len(),
                bytes: messages.iter().map(String::len).sum(),
            },
            JsonRpcResult::None => Self::none(),
        }
    }
}

fn document_uri(value: &serde_json::Value) -> Option<String> {
    value
        .pointer("/textDocument/uri")
        .or_else(|| value.pointer("/uri"))
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
    use crossbeam_channel::unbounded;
    use lsp_server::{Connection, Message};

    use crate::LaunchConfiguration;

    #[test]
    fn stdio_typed_in_memory_connection_handles_initialize_and_exit() {
        let (client_sender, server_receiver) = unbounded::<Message>();
        let (server_sender, client_receiver) = unbounded::<Message>();
        let connection = Connection {
            sender: server_sender,
            receiver: server_receiver,
        };

        client_sender
            .send(message(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "processId": null,
                    "capabilities": {}
                }
            })))
            .expect("initialize should be sent");
        client_sender
            .send(message(serde_json::json!({
                "jsonrpc": "2.0",
                "method": "exit"
            })))
            .expect("exit should be sent");
        drop(client_sender);

        super::run_connection(connection, LaunchConfiguration::new())
            .expect("typed connection should run");

        let messages = client_receiver.try_iter().collect::<Vec<_>>();
        assert_eq!(messages.len(), 1, "{messages:?}");
        let Message::Response(response) = &messages[0] else {
            panic!("initialize should receive a response: {messages:?}");
        };
        assert_eq!(response.id.to_string(), "1");
        assert!(response.error.is_none());
        let result = response
            .result
            .as_ref()
            .expect("initialize response should have a result");
        assert_eq!(result["serverInfo"]["name"], "vela_lsp_server");
        assert_eq!(result["serverInfo"]["version"], env!("CARGO_PKG_VERSION"));
    }

    fn message(value: serde_json::Value) -> Message {
        serde_json::from_value(value).expect("test message should be typed LSP")
    }
}
