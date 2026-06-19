use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn stdio_binary_uses_typed_transport_for_initialize_and_exit() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_vela_lsp_server"))
        .arg("--stdio")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("vela_lsp_server binary should start");

    {
        let stdin = child.stdin.as_mut().expect("child stdin should be piped");
        stdin
            .write_all(frame(&initialize_request()).as_bytes())
            .expect("initialize should write to child stdin");
        stdin
            .write_all(frame(&exit_notification()).as_bytes())
            .expect("exit should write to child stdin");
    }
    drop(child.stdin.take());

    let output = child
        .wait_with_output()
        .expect("vela_lsp_server should exit after exit notification");
    assert!(
        output.status.success(),
        "server exited with {:?}, stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let messages = framed_messages(&output.stdout);
    assert_eq!(messages.len(), 1, "{messages:?}");
    let response = json_value(&messages[0]);
    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], 1);
    assert_eq!(response["result"]["serverInfo"]["name"], "vela_lsp_server");
    assert_eq!(
        response["result"]["serverInfo"]["version"],
        env!("CARGO_PKG_VERSION")
    );
}

fn initialize_request() -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "processId": null,
            "capabilities": {}
        }
    })
    .to_string()
}

fn exit_notification() -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "method": "exit"
    })
    .to_string()
}

fn frame(message: &str) -> String {
    format!("Content-Length: {}\r\n\r\n{message}", message.len())
}

fn framed_messages(output: &[u8]) -> Vec<String> {
    let text = String::from_utf8(output.to_vec()).expect("stdio output should be UTF-8");
    let mut remaining = text.as_str();
    let mut messages = Vec::new();
    while !remaining.is_empty() {
        let (headers, after_headers) = remaining
            .split_once("\r\n\r\n")
            .expect("framed message should contain a header terminator");
        let content_length = headers
            .lines()
            .find_map(|line| {
                line.strip_prefix("Content-Length:")
                    .and_then(|value| value.trim().parse::<usize>().ok())
            })
            .expect("framed message should include Content-Length");
        let (message, rest) = after_headers.split_at(content_length);
        messages.push(message.to_owned());
        remaining = rest;
    }
    messages
}

fn json_value(source: &str) -> serde_json::Value {
    serde_json::from_str(source).expect("message should be valid JSON")
}
