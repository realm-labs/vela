use std::io::Cursor;

use super::{json_value, request};

#[test]
fn lsp_server_stdio_smoke_test() {
    let initialize = request(
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "capabilities": {}
        }),
    );
    let exit = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "exit"
    })
    .to_string();
    let input = format!("{}{}", frame(&initialize), frame(&exit));
    let mut output = Vec::new();

    crate::stdio::run_stdio(Cursor::new(input.into_bytes()), &mut output)
        .expect("stdio transport should handle framed JSON-RPC messages");

    let messages = framed_messages(&output);
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
