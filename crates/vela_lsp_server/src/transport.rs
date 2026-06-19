use std::fs::{File, OpenOptions};
use std::io::{self, BufReader};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use crossbeam_channel::{Receiver, Sender, bounded};
use lsp_server::{Connection, Message, Notification, RequestId, Response, ResponseError};

use crate::{JsonRpcResult, LaunchConfiguration};

pub fn listen_tcp_once(address: &str, configuration: LaunchConfiguration) -> anyhow::Result<()> {
    let listener = bind_loopback_tcp_listener(address)?;
    eprintln!("vela_lsp_server listening on {}", listener.local_addr()?);
    run_tcp_listener(listener, configuration)
}

pub fn bind_loopback_tcp_listener(address: &str) -> io::Result<TcpListener> {
    let addrs = address.to_socket_addrs()?.collect::<Vec<_>>();
    if addrs.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("`{address}` did not resolve to a socket address"),
        ));
    }
    if let Some(addr) = addrs.iter().find(|addr| !addr.ip().is_loopback()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("TCP LSP debug transport only accepts loopback bind addresses, got `{addr}`"),
        ));
    }
    TcpListener::bind(addrs.as_slice())
}

pub fn run_tcp_listener(
    listener: TcpListener,
    configuration: LaunchConfiguration,
) -> anyhow::Result<()> {
    let (stream, peer_addr) = listener.accept()?;
    eprintln!("vela_lsp_server accepted TCP LSP client {peer_addr}");
    let (connection, io_threads) = tcp_connection(stream)?;
    let result = run_connection(connection, configuration);
    io_threads.join()?;
    result
}

fn tcp_connection(stream: TcpStream) -> io::Result<(Connection, TcpIoThreads)> {
    let (reader_sender, reader_receiver) = bounded::<Message>(0);
    let (writer_sender, writer_receiver) = bounded::<Message>(0);
    let (drop_sender, drop_receiver) = bounded::<Message>(0);

    let reader_stream = stream.try_clone()?;
    let reader = thread::Builder::new()
        .name("VelaLspTcpReader".to_owned())
        .spawn(move || tcp_reader(reader_stream, reader_sender))?;
    let writer = thread::Builder::new()
        .name("VelaLspTcpWriter".to_owned())
        .spawn(move || tcp_writer(stream, writer_receiver, drop_sender))?;
    let dropper = thread::Builder::new()
        .name("VelaLspTcpDropper".to_owned())
        .spawn(move || drop_receiver.into_iter().for_each(drop))?;

    Ok((
        Connection {
            sender: writer_sender,
            receiver: reader_receiver,
        },
        TcpIoThreads {
            reader,
            writer,
            dropper,
        },
    ))
}

fn tcp_reader(stream: TcpStream, sender: Sender<Message>) -> io::Result<()> {
    let mut reader = BufReader::new(stream);
    while let Some(message) = Message::read(&mut reader)? {
        let is_exit = matches!(&message, Message::Notification(notification) if notification.method == "exit");
        if sender.send(message).is_err() {
            break;
        }
        if is_exit {
            break;
        }
    }
    Ok(())
}

fn tcp_writer(
    mut stream: TcpStream,
    receiver: Receiver<Message>,
    drop_sender: Sender<Message>,
) -> io::Result<()> {
    for message in receiver {
        let result = message.write(&mut stream);
        let _ = drop_sender.send(message);
        result?;
    }
    Ok(())
}

struct TcpIoThreads {
    reader: thread::JoinHandle<io::Result<()>>,
    writer: thread::JoinHandle<io::Result<()>>,
    dropper: thread::JoinHandle<()>,
}

impl TcpIoThreads {
    fn join(self) -> io::Result<()> {
        match self.reader.join() {
            Ok(result) => result?,
            Err(error) => std::panic::panic_any(error),
        }
        match self.writer.join() {
            Ok(result) => result?,
            Err(error) => std::panic::panic_any(error),
        }
        match self.dropper.join() {
            Ok(()) => Ok(()),
            Err(error) => std::panic::panic_any(error),
        }
    }
}

pub fn run_connection(
    connection: Connection,
    configuration: LaunchConfiguration,
) -> anyhow::Result<()> {
    crate::main_loop::run(connection, configuration)
}

pub(crate) fn messages_from_result(result: JsonRpcResult) -> anyhow::Result<Vec<Message>> {
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

pub(crate) fn serialize_json_rpc_message(message: &Message) -> anyhow::Result<String> {
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

pub(crate) struct RequestProfiler {
    writer: Option<File>,
    slow_ms: u64,
}

impl RequestProfiler {
    pub(crate) fn from_configuration(configuration: &LaunchConfiguration) -> anyhow::Result<Self> {
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

    pub(crate) fn begin(
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

    pub(crate) fn end(
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
pub(crate) struct MessageMetadata {
    kind: &'static str,
    method: Option<String>,
    id: Option<String>,
    document_uri: Option<String>,
}

impl MessageMetadata {
    pub(crate) fn from_message(message: &Message) -> Self {
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

pub(crate) struct ResultSummary {
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

    pub(crate) fn from_result(result: &JsonRpcResult) -> Self {
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

fn timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis())
}

#[cfg(test)]
mod tests {
    use std::thread;
    use std::time::Duration;

    use crossbeam_channel::unbounded;
    use lsp_server::{Connection, Message, Notification, Request, RequestId};

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

    #[test]
    fn typed_dispatcher_reports_unsupported_requests() {
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
                "id": 2,
                "method": "textDocument/implementation",
                "params": {
                    "textDocument": { "uri": "file:///workspace/scripts/main.vela" },
                    "position": { "line": 0, "character": 0 }
                }
            })))
            .expect("unsupported request should be sent");
        client_sender
            .send(message(serde_json::json!({
                "jsonrpc": "2.0",
                "method": "exit"
            })))
            .expect("exit should be sent");
        drop(client_sender);

        super::run_connection(connection, LaunchConfiguration::new())
            .expect("typed connection should run");

        let responses = client_receiver
            .try_iter()
            .filter_map(|message| match message {
                Message::Response(response) => Some(response),
                Message::Request(_) | Message::Notification(_) => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(responses.len(), 2, "{responses:?}");
        assert_eq!(responses[0].id.to_string(), "1");
        assert!(responses[0].error.is_none());
        assert_eq!(responses[1].id.to_string(), "2");
        let error = responses[1]
            .error
            .as_ref()
            .expect("unsupported request should produce an error");
        assert_eq!(error.code, -32601);
        assert_eq!(
            error.message,
            "method `textDocument/implementation` is not implemented"
        );
    }

    #[test]
    fn tcp_rejects_non_loopback_bind_address() {
        let error = super::bind_loopback_tcp_listener("0.0.0.0:0")
            .expect_err("non-loopback bind should be rejected");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("loopback"));
    }

    #[test]
    fn tcp_loopback_connection_handles_initialize_and_exit() {
        let listener = super::bind_loopback_tcp_listener("127.0.0.1:0")
            .expect("loopback listener should bind");
        let address = listener
            .local_addr()
            .expect("listener should report local address");
        let server = thread::spawn(move || {
            super::run_tcp_listener(listener, LaunchConfiguration::new())
                .expect("TCP listener should run");
        });
        let (client, client_io) =
            Connection::connect(address).expect("client should connect to loopback listener");

        client
            .sender
            .send(Message::Request(Request {
                id: RequestId::from(1),
                method: "initialize".to_owned(),
                params: serde_json::json!({
                    "processId": null,
                    "capabilities": {}
                }),
            }))
            .expect("initialize should send over TCP");
        let response = client
            .receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("initialize should receive a TCP response");
        let Message::Response(response) = response else {
            panic!("initialize should receive a response");
        };
        assert_eq!(response.id.to_string(), "1");
        assert!(response.error.is_none());
        assert_eq!(
            response
                .result
                .as_ref()
                .expect("initialize should return result")["serverInfo"]["name"],
            "vela_lsp_server"
        );

        client
            .sender
            .send(Message::Notification(Notification {
                method: "exit".to_owned(),
                params: serde_json::Value::Null,
            }))
            .expect("exit should send over TCP");
        drop(client.sender);
        client_io.join().expect("client IO threads should join");
        server.join().expect("server thread should join");
    }

    fn message(value: serde_json::Value) -> Message {
        serde_json::from_value(value).expect("test message should be typed LSP")
    }
}
