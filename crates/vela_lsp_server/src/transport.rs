use std::io::{self, BufReader};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::thread;

use crossbeam_channel::{Receiver, Sender, bounded};
use lsp_server::{Connection, Message};

use crate::{LaunchConfiguration, profile, rpc};

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
    crate::main_loop::run_on_latency_thread(connection, configuration)
}

pub(crate) fn serialize_json_rpc_message(message: &Message) -> anyhow::Result<String> {
    Ok(rpc::serialize_message(message))
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
                    "json_rpc_error"
                } else {
                    "response"
                },
                method: None,
                id: Some(response.id.to_string()),
                document_uri: response.result.as_ref().and_then(document_uri),
            },
        }
    }

    pub(crate) const fn kind(&self) -> &'static str {
        self.kind
    }

    pub(crate) fn method(&self) -> Option<&str> {
        self.method.as_deref()
    }

    pub(crate) fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    pub(crate) fn document_uri(&self) -> Option<&str> {
        self.document_uri.as_deref()
    }
}

impl profile::ProfileMetadata for MessageMetadata {
    fn kind(&self) -> &'static str {
        self.kind()
    }

    fn method(&self) -> Option<&str> {
        self.method()
    }

    fn id(&self) -> Option<&str> {
        self.id()
    }

    fn document_uri(&self) -> Option<&str> {
        self.document_uri()
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

    pub(crate) fn from_messages(messages: &[Message]) -> Self {
        match messages {
            [] => Self::none(),
            [Message::Response(message)] => Self {
                kind: "response",
                messages: 1,
                bytes: rpc::serialize_message(&Message::Response(message.clone())).len(),
            },
            [message @ Message::Notification(_)] => Self {
                kind: "notification",
                messages: 1,
                bytes: rpc::serialize_message(message).len(),
            },
            [message @ Message::Request(_)] => Self {
                kind: "request",
                messages: 1,
                bytes: rpc::serialize_message(message).len(),
            },
            messages => Self {
                kind: "messages",
                messages: messages.len(),
                bytes: messages
                    .iter()
                    .map(|message| rpc::serialize_message(message).len())
                    .sum(),
            },
        }
    }

    pub(crate) const fn kind(&self) -> &'static str {
        self.kind
    }

    pub(crate) const fn messages(&self) -> usize {
        self.messages
    }

    pub(crate) const fn bytes(&self) -> usize {
        self.bytes
    }
}

impl profile::ProfileSummary for ResultSummary {
    fn kind(&self) -> &'static str {
        self.kind()
    }

    fn messages(&self) -> usize {
        self.messages()
    }

    fn bytes(&self) -> usize {
        self.bytes()
    }
}

pub(crate) fn document_uri(value: &serde_json::Value) -> Option<String> {
    value
        .pointer("/textDocument/uri")
        .or_else(|| value.pointer("/uri"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}

#[cfg(test)]
mod config_tests;

#[cfg(test)]
mod tests {
    use std::time::Duration;
    use std::{fs, path::PathBuf, thread};

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
    fn typed_main_loop_writes_trace_log_when_configured() {
        let trace_path = temp_trace_path("typed_main_loop");
        let (client_sender, server_receiver) = unbounded::<Message>();
        let (server_sender, _client_receiver) = unbounded::<Message>();
        let connection = Connection {
            sender: server_sender,
            receiver: server_receiver,
        };
        let mut configuration = LaunchConfiguration::new();
        configuration.set_trace_log_path(trace_path.display().to_string());

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

        super::run_connection(connection, configuration).expect("typed connection should run");

        let trace = fs::read_to_string(&trace_path).expect("trace log should be readable");
        let events = trace
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("valid JSONL"))
            .collect::<Vec<_>>();
        assert_eq!(events[0]["event"], "session_start");
        assert!(
            events.iter().any(|event| {
                event["event"] == "message_received"
                    && event["method"] == "initialize"
                    && event["id"] == "1"
                    && event["lane"] == "main"
            }),
            "{events:?}"
        );
        assert!(
            events.iter().any(|event| {
                event["event"] == "response_sent"
                    && event["method"] == "initialize"
                    && event["resultKind"] == "response"
                    && event["outputMessages"] == 1
            }),
            "{events:?}"
        );
        fs::remove_file(&trace_path).expect("trace log should be removable");
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
    fn typed_dispatcher_ignores_unknown_cancel_before_later_request() {
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
                "method": "$/cancelRequest",
                "params": { "id": 2 }
            })))
            .expect("cancel notification should be sent");
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
            .expect("later request should be sent");
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
    fn typed_dispatcher_rejects_malformed_initialize_without_initializing() {
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
                    "capabilities": []
                }
            })))
            .expect("malformed initialize should be sent");
        client_sender
            .send(message(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "initialize",
                "params": {
                    "processId": null,
                    "capabilities": {}
                }
            })))
            .expect("valid initialize should be sent");
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
        let error = responses[0]
            .error
            .as_ref()
            .expect("malformed initialize should produce an error");
        assert_eq!(error.code, -32602);
        assert!(
            error.message.contains("invalid initialize params"),
            "unexpected message: {}",
            error.message
        );
        assert_eq!(responses[1].id.to_string(), "2");
        assert!(responses[1].error.is_none(), "{responses:?}");
        assert_eq!(
            responses[1]
                .result
                .as_ref()
                .expect("initialize should produce a result")["serverInfo"]["name"],
            "vela_lsp_server"
        );
    }

    #[test]
    fn typed_dispatcher_preserves_lifecycle_request_gates() {
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
                "method": "shutdown",
                "params": null
            })))
            .expect("early shutdown should be sent");
        client_sender
            .send(message(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 2,
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
                "id": 3,
                "method": "shutdown",
                "params": null
            })))
            .expect("shutdown should be sent");
        client_sender
            .send(message(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "textDocument/hover",
                "params": {
                    "textDocument": { "uri": "file:///workspace/scripts/main.vela" },
                    "position": { "line": 0, "character": 0 }
                }
            })))
            .expect("post-shutdown hover should be sent");
        client_sender
            .send(message(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 5,
                "method": "exit",
                "params": null
            })))
            .expect("request-shaped exit should be sent");
        client_sender
            .send(message(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 6,
                "method": "textDocument/hover",
                "params": {
                    "textDocument": { "uri": "file:///workspace/scripts/main.vela" },
                    "position": { "line": 0, "character": 0 }
                }
            })))
            .expect("post-exit hover should be sent");
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
        assert_eq!(responses.len(), 5, "{responses:?}");
        assert_eq!(responses[0].id.to_string(), "1");
        assert_eq!(
            responses[0]
                .error
                .as_ref()
                .expect("early shutdown should error")
                .code,
            -32002
        );
        assert_eq!(responses[1].id.to_string(), "2");
        assert!(responses[1].error.is_none());
        assert_eq!(responses[2].id.to_string(), "3");
        assert!(responses[2].error.is_none());
        assert_eq!(responses[3].id.to_string(), "4");
        let shutdown_error = responses[3]
            .error
            .as_ref()
            .expect("post-shutdown request should error");
        assert_eq!(shutdown_error.code, -32600);
        assert_eq!(shutdown_error.message, "server has shut down");
        assert_eq!(responses[4].id.to_string(), "5");
        let exit_error = responses[4]
            .error
            .as_ref()
            .expect("request-shaped exit should error");
        assert_eq!(exit_error.code, -32600);
        assert_eq!(exit_error.message, "`exit` must be sent as a notification");
    }

    #[test]
    fn typed_dispatcher_repeated_initialize_keeps_original_watcher_roots() {
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
                    "rootUri": "file:///workspace",
                    "initializationOptions": {
                        "host": {
                            "schema": "target/vela/schema.json"
                        }
                    },
                    "capabilities": {
                        "workspace": {
                            "didChangeWatchedFiles": {
                                "dynamicRegistration": true
                            }
                        }
                    }
                }
            })))
            .expect("initialize should be sent");
        client_sender
            .send(message(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "initialize",
                "params": {
                    "processId": null,
                    "rootUri": "file:///other",
                    "capabilities": {}
                }
            })))
            .expect("repeated initialize should be sent");
        client_sender
            .send(message(serde_json::json!({
                "jsonrpc": "2.0",
                "method": "initialized",
                "params": {}
            })))
            .expect("initialized should be sent");
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
        let responses = messages
            .iter()
            .filter_map(|message| match message {
                Message::Response(response) => Some(response),
                Message::Request(_) | Message::Notification(_) => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(responses.len(), 2, "{responses:?}");
        assert_eq!(responses[0].id.to_string(), "1");
        assert!(responses[0].error.is_none());
        assert_eq!(responses[1].id.to_string(), "2");
        let repeated_error = responses[1]
            .error
            .as_ref()
            .expect("repeated initialize should error");
        assert_eq!(repeated_error.code, -32600);
        assert_eq!(repeated_error.message, "server is already initialized");

        let registration = messages
            .iter()
            .find_map(|message| match message {
                Message::Request(request) if request.method == "client/registerCapability" => {
                    Some(&request.params)
                }
                Message::Notification(notification)
                    if notification.method == "client/registerCapability" =>
                {
                    Some(&notification.params)
                }
                Message::Request(_) | Message::Response(_) | Message::Notification(_) => None,
            })
            .unwrap_or_else(|| panic!("initialized should register watched files: {messages:?}"));
        let watchers = registration["registrations"][0]["registerOptions"]["watchers"]
            .as_array()
            .expect("watcher registration should include watchers");
        assert!(watchers.iter().any(|watcher| {
            watcher["globPattern"]["baseUri"] == "file:///workspace"
                && watcher["globPattern"]["pattern"] == "**/*.vela"
        }));
        assert!(!watchers.iter().any(|watcher| {
            watcher["globPattern"]["baseUri"] == "file:///other"
                && watcher["globPattern"]["pattern"] == "**/*.vela"
        }));
    }

    #[test]
    fn typed_dispatcher_skips_watcher_registration_when_disabled() {
        let (client_sender, server_receiver) = unbounded::<Message>();
        let (server_sender, client_receiver) = unbounded::<Message>();
        let connection = Connection {
            sender: server_sender,
            receiver: server_receiver,
        };
        let mut configuration = LaunchConfiguration::new();
        configuration.set_watch_files_enabled(false);

        client_sender
            .send(message(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "processId": null,
                    "rootUri": "file:///workspace",
                    "capabilities": {
                        "workspace": {
                            "didChangeWatchedFiles": {
                                "dynamicRegistration": true
                            }
                        }
                    }
                }
            })))
            .expect("initialize should be sent");
        client_sender
            .send(message(serde_json::json!({
                "jsonrpc": "2.0",
                "method": "initialized",
                "params": {}
            })))
            .expect("initialized should be sent");
        client_sender
            .send(message(serde_json::json!({
                "jsonrpc": "2.0",
                "method": "exit"
            })))
            .expect("exit should be sent");
        drop(client_sender);

        super::run_connection(connection, configuration).expect("typed connection should run");

        let messages = client_receiver.try_iter().collect::<Vec<_>>();
        assert!(messages.iter().any(|message| matches!(
            message,
            Message::Response(response)
                if response.id.to_string() == "1" && response.error.is_none()
        )));
        assert!(!messages.iter().any(|message| matches!(
            message,
            Message::Request(request) if request.method == "client/registerCapability"
        )));
        assert!(!messages.iter().any(|message| matches!(
            message,
            Message::Notification(notification)
                if notification.method == "client/registerCapability"
        )));
    }

    #[test]
    fn typed_dispatcher_ignores_empty_host_schema_for_watcher_registration() {
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
                    "rootUri": "file:///workspace",
                    "initializationOptions": {
                        "host": {
                            "schema": ""
                        }
                    },
                    "capabilities": {
                        "workspace": {
                            "didChangeWatchedFiles": {
                                "dynamicRegistration": true
                            }
                        }
                    }
                }
            })))
            .expect("initialize should be sent");
        client_sender
            .send(message(serde_json::json!({
                "jsonrpc": "2.0",
                "method": "initialized",
                "params": {}
            })))
            .expect("initialized should be sent");
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
        let registration = messages
            .iter()
            .find_map(|message| match message {
                Message::Request(request) if request.method == "client/registerCapability" => {
                    Some(&request.params)
                }
                Message::Notification(notification)
                    if notification.method == "client/registerCapability" =>
                {
                    Some(&notification.params)
                }
                Message::Request(_) | Message::Response(_) | Message::Notification(_) => None,
            })
            .unwrap_or_else(|| panic!("initialized should register watched files: {messages:?}"));
        let watchers = registration["registrations"][0]["registerOptions"]["watchers"]
            .as_array()
            .expect("watcher registration should include watchers");
        assert!(watchers.iter().any(|watcher| {
            watcher["globPattern"]["baseUri"] == "file:///workspace"
                && watcher["globPattern"]["pattern"] == "**/*.vela"
        }));
        assert!(watchers.iter().all(|watcher| {
            !watcher["globPattern"]
                .as_str()
                .is_some_and(|pattern| pattern.ends_with("schema.json"))
        }));
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

    fn temp_trace_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "vela_lsp_trace_{name}_{}_{}.jsonl",
            std::process::id(),
            crate::profile::timestamp_ms()
        ))
    }
}
