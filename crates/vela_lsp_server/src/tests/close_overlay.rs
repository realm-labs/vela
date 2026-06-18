use super::{JsonRpcResult, LspServer, notification, notification_value, request, response_value};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn lsp_did_close_restores_disk_snapshot_completion_queries() {
    let root = temp_workspace();
    let source_path = root.join("scripts").join("game").join("main.vela");
    let disk_source = "pub fn disk_only() { return 1 }\npub fn main() { di }";
    let overlay_source = "pub fn overlay_only() { return 2 }\npub fn main() { ov }";
    fs::write(&source_path, disk_source).expect("disk source should be writable");
    let source_uri = file_uri(&source_path);

    let mut server = LspServer::new();
    let _ = response_value(server.handle_json(&request(
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": file_uri(&root.join("scripts")),
            "capabilities": {}
        }),
    )));
    assert_eq!(
        server.handle_json(&notification(
            "workspace/didChangeWatchedFiles",
            serde_json::json!({
                "changes": [{ "uri": source_uri, "type": 1 }]
            }),
        )),
        JsonRpcResult::None
    );
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": source_uri,
                "languageId": "vela",
                "version": 1,
                "text": overlay_source
            }
        }),
    )));

    let overlay_completion = completion_labels(&response_value(server.handle_json(&request(
        2,
        "textDocument/completion",
        serde_json::json!({
            "textDocument": { "uri": source_uri },
            "position": {
                "line": 1,
                "character": completion_character(overlay_source, "ov")
            }
        }),
    ))));
    assert!(
        overlay_completion
            .iter()
            .any(|label| label == "overlay_only"),
        "{overlay_completion:?}"
    );
    assert!(
        overlay_completion.iter().all(|label| label != "disk_only"),
        "{overlay_completion:?}"
    );

    let close = notification_value(server.handle_json(&notification(
        "textDocument/didClose",
        serde_json::json!({
            "textDocument": {
                "uri": source_uri
            }
        }),
    )));
    assert_eq!(close["method"], "textDocument/publishDiagnostics");
    assert_eq!(close["params"]["uri"], source_uri);

    let disk_completion = completion_labels(&response_value(server.handle_json(&request(
        3,
        "textDocument/completion",
        serde_json::json!({
            "textDocument": { "uri": source_uri },
            "position": {
                "line": 1,
                "character": completion_character(disk_source, "di")
            }
        }),
    ))));
    assert!(
        disk_completion.iter().any(|label| label == "disk_only"),
        "{disk_completion:?}"
    );
    assert!(
        disk_completion.iter().all(|label| label != "overlay_only"),
        "{disk_completion:?}"
    );

    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

fn completion_labels(response: &serde_json::Value) -> Vec<String> {
    response["result"]["items"]
        .as_array()
        .expect("completion response should contain items")
        .iter()
        .filter_map(|item| item["label"].as_str())
        .map(str::to_owned)
        .collect()
}

fn completion_character(source: &str, prefix: &str) -> usize {
    let line = source.lines().nth(1).expect("completion line should exist");
    line.find(prefix).expect("completion prefix should exist") + prefix.len()
}

fn temp_workspace() -> PathBuf {
    let suffix = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(error) => panic!("system time should be after UNIX_EPOCH: {error}"),
    };
    let root = std::env::temp_dir().join(format!(
        "vela_lsp_close_overlay_{}_{}",
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
