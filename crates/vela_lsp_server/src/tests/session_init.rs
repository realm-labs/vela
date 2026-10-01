use std::{fs, path::PathBuf, thread, time::Duration};

use crossbeam_channel::{bounded, unbounded};
use lsp_server::{Connection, Message};
use serde_json::{Value, json};

use super::{TestServer, message_value, support::unique_temp_root};
use crate::{
    LaunchConfiguration,
    matrix_fixture::{FixtureWorkspace, Spec},
};

struct Layout {
    parent: PathBuf,
    root: PathBuf,
    fixture: FixtureWorkspace,
}
impl Layout {
    fn new(spec: &Spec) -> Self {
        let parent = unique_temp_root("session-init");
        let root = parent.join("中文 % session");
        let fixture = FixtureWorkspace::new(spec).expect("authored inputs");
        fixture.materialize(&root).expect("owned new files");
        Self {
            parent,
            root,
            fixture,
        }
    }
    fn uri(&self, file: &str) -> String {
        lsp_types::Url::from_file_path(self.root.join(file))
            .expect("file URI")
            .to_string()
    }
    fn disk(&self) {
        for (file, source) in &self.fixture.disk {
            assert_eq!(
                fs::read_to_string(self.root.join(file)).expect("disk input"),
                source.text
            );
        }
    }
    fn params(&self, capabilities: Value) -> Value {
        json!({"processId":null,"rootUri":self.uri(""),"workspaceFolders":[
            {"uri":self.uri(""),"name":"duplicate 中😀"},{"uri":self.uri(""),"name":"duplicate again"}],"capabilities":capabilities})
    }
}
impl Drop for Layout {
    fn drop(&mut self) {
        let parent = self.parent.canonicalize().expect("owned parent");
        assert_eq!(
            parent.parent(),
            Some(
                std::env::temp_dir()
                    .canonicalize()
                    .expect("temp root")
                    .as_path()
            )
        );
        assert!(
            parent
                .file_name()
                .expect("name")
                .to_string_lossy()
                .starts_with("vela-lsp-session-init-")
        );
        fs::remove_dir_all(parent).expect("remove owned inputs");
    }
}
fn wire(value: Value) -> Message {
    serde_json::from_value(value).expect("authored message")
}
fn request(server: &mut TestServer, id: Value, method: &str, params: Value) -> Vec<Value> {
    send(
        server,
        json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}),
    )
}
fn send(server: &mut TestServer, value: Value) -> Vec<Value> {
    server
        .send_protocol_message(wire(value))
        .iter()
        .map(message_value)
        .collect()
}
fn notify(server: &mut TestServer, method: &str, params: Value) -> Vec<Value> {
    send(
        server,
        json!({"jsonrpc":"2.0","method":method,"params":params}),
    )
}
fn flags(server: &TestServer) -> [bool; 3] {
    [
        server.state().is_initialized(),
        server.state().is_shutdown_requested(),
        server.state().is_exited(),
    ]
}
fn response(id: Value, result: Value) -> Vec<Value> {
    vec![json!({"jsonrpc":"2.0","id":id,"result":result})]
}
fn error(messages: &[Value], id: Value, code: i32, message: &str, prefix: bool) {
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["jsonrpc"], "2.0");
    assert_eq!(messages[0]["id"], id);
    assert_eq!(messages[0].as_object().expect("response").len(), 3);
    let e = &messages[0]["error"];
    assert_eq!(e.as_object().expect("error").len(), 2);
    assert_eq!(e["code"], code);
    let actual = e["message"].as_str().expect("message");
    if prefix {
        assert!(actual.starts_with(message), "{actual}");
    } else {
        assert_eq!(actual, message);
    }
}
fn expected(profile: &Value) -> Value {
    let mut result = profile["result"].clone();
    result["serverInfo"]["version"] = json!(env!("CARGO_PKG_VERSION"));
    result
}
fn hover(server: &mut TestServer, layout: &Layout, spec: &Spec, id: Value) -> Vec<Value> {
    let marker = layout.fixture.disk["scripts/main.vela"].markers["call"];
    let messages = request(
        server,
        id.clone(),
        "textDocument/hover",
        json!({"textDocument":{"uri":layout.uri("scripts/main.vela")},
        "position":{"line":marker.start.line,"character":marker.start.character}}),
    );
    assert_eq!(
        messages,
        response(
            id,
            json!({"contents":{"kind":"markdown","value":spec.oracle["hover"]},"range":{
        "start":{"line":marker.start.line,"character":marker.start.character},"end":{"line":marker.end.line,"character":marker.end.character}}})
        )
    );
    messages
}
fn registration(layout: &Layout) -> Vec<Value> {
    vec![
        json!({"jsonrpc":"2.0","id":"vela/watched-files","method":"client/registerCapability","params":{"registrations":[
        {"id":"vela/watched-files","method":"workspace/didChangeWatchedFiles","registerOptions":{"watchers":[
            {"globPattern":{"baseUri":layout.uri("scripts"),"pattern":"**/*.vela"},"kind":7},
            {"globPattern":{"baseUri":layout.uri(""),"pattern":"vela.toml"},"kind":7}
        ]}}]}}),
    ]
}

#[test]
fn session_initialization_matrix_preserves_complete_capabilities_and_registration_policy() {
    let spec = crate::matrix_fixture::load("session-init");
    assert_eq!(
        spec.oracle["profiles"].as_array().expect("profiles").len(),
        5
    );
    for profile in spec.oracle["profiles"].as_array().expect("profiles") {
        let layout = Layout::new(&spec);
        let mut config = LaunchConfiguration::new();
        config.set_watch_files_enabled(profile["watchEnabled"].as_bool().expect("watch policy"));
        let mut server = TestServer::with_launch_configuration(config);
        assert_eq!(flags(&server), [false, false, false]);
        assert!(notify(&mut server, "initialized", json!({})).is_empty());
        assert_eq!(flags(&server), [false, false, false]);
        let id = json!(format!(
            "initialize 中😀 {}",
            profile["id"].as_str().expect("profile id")
        ));
        assert_eq!(
            request(
                &mut server,
                id.clone(),
                "initialize",
                layout.params(profile["capabilities"].clone())
            ),
            response(id, expected(profile))
        );
        assert_eq!(flags(&server), [true, false, false]);
        assert_eq!(
            server.snapshot().databases().source_db().records().len(),
            2,
            "disk modules loaded without didOpen"
        );
        assert!(
            server
                .snapshot()
                .workspace()
                .document(&vela_language_service::DocumentId::from(
                    layout.uri("scripts/main.vela")
                ))
                .is_none()
        );
        hover(&mut server, &layout, &spec, json!(21));
        let generation = server.snapshot().generation();
        error(
            &request(
                &mut server,
                json!(22),
                "initialize",
                json!({"processId":null,"rootUri":layout.uri("other"),"capabilities":{}}),
            ),
            json!(22),
            -32600,
            "server is already initialized",
            false,
        );
        assert_eq!(server.snapshot().generation(), generation);
        error(
            &request(&mut server, json!(23), "initialized", json!({})),
            json!(23),
            -32600,
            "`initialized` must be sent as a notification",
            false,
        );
        assert!(notify(&mut server, "initialized", Value::Null).is_empty());
        let messages = notify(&mut server, "initialized", json!({}));
        assert_eq!(
            messages,
            if profile["registered"] == true {
                registration(&layout)
            } else {
                vec![]
            }
        );
        assert!(
            notify(&mut server, "initialized", json!({})).is_empty(),
            "register at most once"
        );
        assert!(
            send(
                &mut server,
                json!({"jsonrpc":"2.0","id":"vela/watched-files","result":null})
            )
            .is_empty()
        );
        assert_eq!(server.snapshot().generation(), generation);
        hover(&mut server, &layout, &spec, json!(24));
        assert_eq!(
            request(&mut server, json!(25), "shutdown", Value::Null),
            response(json!(25), Value::Null)
        );
        assert_eq!(flags(&server), [true, true, false]);
        assert!(notify(&mut server, "exit", Value::Null).is_empty());
        assert_eq!(flags(&server), [true, true, true]);
        layout.disk();
    }
}

#[test]
fn session_invalid_requests_preserve_lifecycle_and_source_ownership() {
    let spec = crate::matrix_fixture::load("session-init");
    let layout = Layout::new(&spec);
    assert_eq!(
        spec.oracle["invalidInitialize"]
            .as_array()
            .expect("invalid params")
            .len(),
        8
    );
    for params in spec.oracle["invalidInitialize"]
        .as_array()
        .expect("invalid params")
    {
        let mut server = TestServer::new();
        let generation = server.snapshot().generation();
        error(
            &request(&mut server, json!(-1), "initialize", params.clone()),
            json!(-1),
            -32602,
            "invalid initialize params:",
            true,
        );
        assert_eq!(flags(&server), [false, false, false]);
        assert_eq!(server.snapshot().generation(), generation);
        assert!(
            server
                .snapshot()
                .databases()
                .source_db()
                .records()
                .is_empty()
        );
        assert!(notify(&mut server, "initialize", layout.params(json!({}))).is_empty());
        assert!(notify(&mut server, "initialized", json!({})).is_empty());
        error(
            &request(&mut server, json!("early😀"), "shutdown", Value::Null),
            json!("early😀"),
            -32002,
            "server has not been initialized",
            false,
        );
        assert_eq!(flags(&server), [false, false, false]);
        let profile = &spec.oracle["profiles"][0];
        assert_eq!(
            request(
                &mut server,
                json!(1),
                "initialize",
                layout.params(json!({}))
            ),
            response(json!(1), expected(profile))
        );
        hover(&mut server, &layout, &spec, json!(2));
        assert_eq!(flags(&server), [true, false, false]);
        layout.disk();
    }
}

#[test]
fn session_shutdown_matrix_preserves_request_errors_and_terminal_state() {
    let spec = crate::matrix_fixture::load("session-init");
    let layout = Layout::new(&spec);
    for exit_request in [false, true] {
        let mut server = TestServer::new();
        let profile = &spec.oracle["profiles"][0];
        assert_eq!(
            request(
                &mut server,
                json!(1),
                "initialize",
                layout.params(json!({}))
            ),
            response(json!(1), expected(profile))
        );
        let generation = server.snapshot().generation();
        for params in [json!({}), json!([]), json!(false)] {
            error(
                &request(&mut server, json!(2), "shutdown", params),
                json!(2),
                -32602,
                "invalid shutdown params:",
                true,
            );
            assert_eq!(flags(&server), [true, false, false]);
            assert_eq!(server.snapshot().generation(), generation);
        }
        assert!(notify(&mut server, "shutdown", Value::Null).is_empty());
        assert_eq!(flags(&server), [true, false, false]);
        hover(&mut server, &layout, &spec, json!(3));
        assert_eq!(
            request(&mut server, json!("shutdown😀"), "shutdown", Value::Null),
            response(json!("shutdown😀"), Value::Null)
        );
        assert_eq!(flags(&server), [true, true, false]);
        for method in [
            "initialize",
            "shutdown",
            "textDocument/hover",
            "initialized",
            "does/not/exist",
        ] {
            error(
                &request(&mut server, json!(4), method, Value::Null),
                json!(4),
                -32600,
                "server has shut down",
                false,
            );
            assert_eq!(flags(&server), [true, true, false]);
            assert_eq!(server.snapshot().generation(), generation);
        }
        assert!(notify(&mut server, "exit", json!({})).is_empty());
        assert_eq!(flags(&server), [true, true, false]);
        if exit_request {
            error(
                &request(&mut server, json!(5), "exit", Value::Null),
                json!(5),
                -32600,
                "`exit` must be sent as a notification",
                false,
            );
        } else {
            assert!(notify(&mut server, "exit", Value::Null).is_empty());
        }
        assert_eq!(flags(&server), [true, true, true]);
        assert!(
            request(
                &mut server,
                json!(6),
                "initialize",
                layout.params(json!({}))
            )
            .is_empty()
        );
        assert_eq!(server.snapshot().generation(), generation);
        layout.disk();
    }
}

#[test]
fn session_real_main_loop_terminates_after_exit_without_processing_queued_messages() {
    let spec = crate::matrix_fixture::load("session-init");
    for exit_request in [false, true] {
        let (sender, receiver) = unbounded();
        let (outbound, inbound) = unbounded();
        let (done, finished) = bounded(1);
        let connection = Connection {
            sender: outbound,
            receiver,
        };
        let mut inputs = vec![
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"processId":null,"capabilities":{}}}),
            json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
            json!({"jsonrpc":"2.0","id":"shutdown😀","method":"shutdown","params":null}),
            json!({"jsonrpc":"2.0","id":3,"method":"initialize","params":{}}),
        ];
        let exit = if exit_request {
            json!({"jsonrpc":"2.0","id":4,"method":"exit","params":null})
        } else {
            json!({"jsonrpc":"2.0","method":"exit","params":null})
        };
        inputs.push(exit);
        inputs.push(json!({"jsonrpc":"2.0","id":5,"method":"initialize","params":{}}));
        inputs.push(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///session/after-exit.vela","languageId":"vela","version":1,"text":"pub struct {"}}}));
        // Enqueue the entire finite conversation before starting to read outputs.
        for input in inputs {
            sender
                .send(wire(input))
                .expect("enqueue complete conversation");
        }
        drop(sender);
        let worker = thread::spawn(move || {
            let result = crate::transport::run_connection(connection, LaunchConfiguration::new())
                .map_err(|e| e.to_string());
            let _ = done.send(result);
        });
        finished
            .recv_timeout(Duration::from_secs(5))
            .expect("actual main loop must stop within five seconds")
            .expect("connection result");
        worker.join().expect("completed main-loop thread");
        let messages = inbound
            .try_iter()
            .map(|m| message_value(&m))
            .collect::<Vec<_>>();
        let mut expected_messages = response(json!(1), expected(&spec.oracle["profiles"][0]));
        expected_messages.extend(response(json!("shutdown😀"), Value::Null));
        expected_messages.push(json!({"jsonrpc":"2.0","id":3,"error":{"code":-32600,"message":"server has shut down"}}));
        if exit_request {
            expected_messages.push(json!({"jsonrpc":"2.0","id":4,"error":{"code":-32600,"message":"`exit` must be sent as a notification"}}));
        }
        assert_eq!(
            messages, expected_messages,
            "no queued request response or diagnostic after exit"
        );
    }
}
