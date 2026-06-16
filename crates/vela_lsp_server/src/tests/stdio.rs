use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::{json_value, notification, request};
use crate::LaunchConfiguration;

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

#[test]
fn cli_config_flags_seed_workspace_config() {
    let root = temp_workspace();
    let schema_path = root.join("target").join("vela").join("schema.json");
    fs::create_dir_all(schema_path.parent().expect("schema should have parent"))
        .expect("schema directory should be creatable");
    fs::write(&schema_path, schema_with_player_field("level", "i64"))
        .expect("schema should be writable");

    let helper_uri = file_uri(&root.join("scripts").join("game").join("helper.vela"));
    let main_uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    let main_text = "\
use game::helper::grant
pub fn main(player: Player) {
    let score = grant()
    return player.level
}";
    let initialize = request(
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "capabilities": {}
        }),
    );
    let open_helper = notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": helper_uri,
                "languageId": "vela",
                "version": 1,
                "text": "pub fn grant() -> i64 { return 1 }"
            }
        }),
    );
    let open_main = notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": main_uri,
                "languageId": "vela",
                "version": 1,
                "text": main_text
            }
        }),
    );
    let completion = request(
        2,
        "textDocument/completion",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": { "line": 3, "character": "    return player.".len() }
        }),
    );
    let exit = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "exit"
    })
    .to_string();
    let input = format!(
        "{}{}{}{}{}",
        frame(&initialize),
        frame(&open_helper),
        frame(&open_main),
        frame(&completion),
        frame(&exit)
    );
    let mut output = Vec::new();
    let mut configuration = LaunchConfiguration::new();
    configuration.add_workspace_root(file_uri(&root.join("scripts")));
    configuration.set_host_schema(file_uri(&schema_path));

    crate::stdio::run_stdio_with_configuration(
        Cursor::new(input.into_bytes()),
        &mut output,
        configuration,
    )
    .expect("stdio transport should use launch configuration");

    let messages = framed_messages(&output)
        .iter()
        .map(|message| json_value(message))
        .collect::<Vec<_>>();
    let main_diagnostics = messages
        .iter()
        .find(|message| {
            message["method"] == "textDocument/publishDiagnostics"
                && message["params"]["uri"] == main_uri
        })
        .expect("main document diagnostics should be published");
    assert_eq!(
        main_diagnostics["params"]["diagnostics"],
        serde_json::json!([])
    );

    let completion_response = messages
        .iter()
        .find(|message| message["id"] == 2)
        .expect("completion request should receive a response");
    assert_completion(completion_response, "level", 5, "i64");

    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

fn assert_completion(response: &serde_json::Value, label: &str, kind: u8, detail: &str) {
    assert_eq!(response["result"]["isIncomplete"], false);
    let Some(items) = response["result"]["items"].as_array() else {
        panic!("completion response should contain items");
    };
    assert!(
        items
            .iter()
            .any(|item| item["label"] == label && item["kind"] == kind && item["detail"] == detail),
        "{items:?}"
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

fn temp_workspace() -> PathBuf {
    let suffix = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(error) => panic!("system time should be after UNIX_EPOCH: {error}"),
    };
    let root = std::env::temp_dir().join(format!(
        "vela_lsp_server_stdio_config_{}_{}",
        std::process::id(),
        suffix
    ));
    fs::create_dir_all(root.join("scripts").join("game"))
        .expect("temporary workspace should be creatable");
    root
}

fn file_uri(path: &Path) -> String {
    let path = path.display().to_string().replace('\\', "/");
    if path.starts_with('/') {
        format!("file://{path}")
    } else {
        format!("file:///{path}")
    }
}

fn schema_with_player_field(name: &str, kind: &str) -> String {
    serde_json::json!({
        "formatVersion": 1,
        "facts": {
            "types": [
                {
                    "name": "Player",
                    "fact": { "kind": "host", "name": "Player" }
                }
            ],
            "fields": [
                {
                    "owner": "Player",
                    "name": name,
                    "fact": { "kind": "primitive", "name": kind }
                }
            ]
        }
    })
    .to_string()
}
