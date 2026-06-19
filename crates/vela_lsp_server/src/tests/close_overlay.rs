use super::{JsonRpcResult, LspServer, notification, notification_value, request, response_value};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_WORKSPACE_ID: AtomicU64 = AtomicU64::new(0);

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

#[test]
fn lsp_did_close_restores_disk_snapshot_type_definition_queries() {
    let root = temp_workspace();
    let source_path = root.join("scripts").join("game").join("main.vela");
    let disk_source = r#"struct DiskInventory {
    slots: i64,
}

struct Player {
    inventory: DiskInventory,
}

fn main(player: Player) {
    return player.inventory;
}"#;
    let overlay_source = r#"struct OverlayInventory {
    slots: i64,
}

struct Player {
    inventory: OverlayInventory,
}

fn main(player: Player) {
    return player.inventory;
}"#;
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

    let overlay_definition = response_value(server.handle_json(&type_definition_request(
        2,
        &source_uri,
        overlay_source,
    )));
    assert_type_definition_range(&overlay_definition, &source_uri, 7, 23);

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

    let disk_definition =
        response_value(server.handle_json(&type_definition_request(3, &source_uri, disk_source)));
    assert_type_definition_range(&disk_definition, &source_uri, 7, 20);

    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_did_close_restores_disk_snapshot_hover_queries() {
    let root = temp_workspace();
    let source_path = root.join("scripts").join("game").join("main.vela");
    let disk_source = r#"#[doc("Disk function")]
pub fn disk_only() -> i64 { return 1 }
pub fn main() -> i64 {
    return disk_only()
}"#;
    let overlay_source = r#"#[doc("Overlay function")]
pub fn overlay_only() -> i64 { return 2 }
pub fn main() -> i64 {
    return overlay_only()
}"#;
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

    let overlay_hover = hover_value(&response_value(server.handle_json(&hover_request(
        2,
        &source_uri,
        overlay_source,
        "overlay_only",
    ))));
    assert!(
        overlay_hover.contains("game::main::overlay_only"),
        "{overlay_hover}"
    );
    assert!(
        overlay_hover.contains("Overlay function"),
        "{overlay_hover}"
    );
    assert!(!overlay_hover.contains("disk_only"), "{overlay_hover}");

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

    let disk_hover = hover_value(&response_value(server.handle_json(&hover_request(
        3,
        &source_uri,
        disk_source,
        "disk_only",
    ))));
    assert!(disk_hover.contains("game::main::disk_only"), "{disk_hover}");
    assert!(disk_hover.contains("Disk function"), "{disk_hover}");
    assert!(!disk_hover.contains("overlay_only"), "{disk_hover}");

    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_did_close_restores_disk_snapshot_reference_queries() {
    let root = temp_workspace();
    let source_path = root.join("scripts").join("game").join("main.vela");
    let disk_source = r#"pub fn disk_only() -> i64 { return 1 }
pub fn main() -> i64 {
    return disk_only()
}"#;
    let overlay_source = r#"pub fn overlay_only() -> i64 { return 2 }
pub fn main() -> i64 {
    return overlay_only()
}"#;
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

    let overlay_references = response_value(server.handle_json(&references_request(
        2,
        &source_uri,
        overlay_source,
        "overlay_only",
    )));
    assert_reference_ranges(&overlay_references, &source_uri, &[(0, 7, 19), (2, 11, 23)]);

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

    let disk_references = response_value(server.handle_json(&references_request(
        3,
        &source_uri,
        disk_source,
        "disk_only",
    )));
    assert_reference_ranges(&disk_references, &source_uri, &[(0, 7, 16), (2, 11, 20)]);

    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_did_close_restores_disk_snapshot_document_highlight_queries() {
    let root = temp_workspace();
    let source_path = root.join("scripts").join("game").join("main.vela");
    let disk_source = r#"pub fn disk_only() -> i64 { return 1 }
pub fn main() -> i64 {
    return disk_only()
}"#;
    let overlay_source = r#"pub fn overlay_only() -> i64 { return 2 }
pub fn main() -> i64 {
    return overlay_only()
}"#;
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

    let overlay_highlights = response_value(server.handle_json(&document_highlight_request(
        2,
        &source_uri,
        overlay_source,
        "overlay_only",
    )));
    assert_document_highlight_ranges(&overlay_highlights, &[(0, 7, 19), (2, 11, 23)]);

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

    let disk_highlights = response_value(server.handle_json(&document_highlight_request(
        3,
        &source_uri,
        disk_source,
        "disk_only",
    )));
    assert_document_highlight_ranges(&disk_highlights, &[(0, 7, 16), (2, 11, 20)]);

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

fn type_definition_request(id: i64, uri: &str, source: &str) -> String {
    request(
        id,
        "textDocument/typeDefinition",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 9,
                "character": type_definition_character(source)
            }
        }),
    )
}

fn type_definition_character(source: &str) -> usize {
    let line = source
        .lines()
        .nth(9)
        .expect("type-definition line should exist");
    line.find("inventory")
        .expect("type-definition target should exist")
}

fn hover_request(id: i64, uri: &str, source: &str, target: &str) -> String {
    request(
        id,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 3,
                "character": hover_character(source, target)
            }
        }),
    )
}

fn hover_character(source: &str, target: &str) -> usize {
    let line = source.lines().nth(3).expect("hover line should exist");
    line.find(target).expect("hover target should exist")
}

fn hover_value(response: &serde_json::Value) -> String {
    response["result"]["contents"]["value"]
        .as_str()
        .expect("hover response should contain markdown")
        .to_owned()
}

fn references_request(id: i64, uri: &str, source: &str, target: &str) -> String {
    request(
        id,
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 2,
                "character": references_character(source, target)
            },
            "context": { "includeDeclaration": true }
        }),
    )
}

fn references_character(source: &str, target: &str) -> usize {
    let line = source.lines().nth(2).expect("references line should exist");
    line.find(target).expect("references target should exist")
}

fn document_highlight_request(id: i64, uri: &str, source: &str, target: &str) -> String {
    request(
        id,
        "textDocument/documentHighlight",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 2,
                "character": references_character(source, target)
            }
        }),
    )
}

fn assert_reference_ranges(
    response: &serde_json::Value,
    expected_uri: &str,
    expected_ranges: &[(usize, usize, usize)],
) {
    let references = response["result"]
        .as_array()
        .expect("references response should contain an array");
    assert_eq!(references.len(), expected_ranges.len(), "{references:?}");
    for (line, start, end) in expected_ranges {
        assert!(
            references.iter().any(|reference| {
                reference["uri"] == expected_uri
                    && reference["range"]["start"]["line"] == *line
                    && reference["range"]["start"]["character"] == *start
                    && reference["range"]["end"]["line"] == *line
                    && reference["range"]["end"]["character"] == *end
            }),
            "missing reference range ({line}, {start}, {end}) in {references:?}"
        );
    }
}

fn assert_document_highlight_ranges(
    response: &serde_json::Value,
    expected_ranges: &[(usize, usize, usize)],
) {
    let highlights = response["result"]
        .as_array()
        .expect("documentHighlight response should contain an array");
    assert_eq!(highlights.len(), expected_ranges.len(), "{highlights:?}");
    for (line, start, end) in expected_ranges {
        assert!(
            highlights.iter().any(|highlight| {
                highlight["range"]["start"]["line"] == *line
                    && highlight["range"]["start"]["character"] == *start
                    && highlight["range"]["end"]["line"] == *line
                    && highlight["range"]["end"]["character"] == *end
                    && highlight["kind"] == 1
            }),
            "missing document highlight range ({line}, {start}, {end}) in {highlights:?}"
        );
    }
}

fn assert_type_definition_range(
    response: &serde_json::Value,
    expected_uri: &str,
    expected_start: usize,
    expected_end: usize,
) {
    assert_eq!(response["result"]["uri"], expected_uri);
    assert_eq!(response["result"]["range"]["start"]["line"], 0);
    assert_eq!(
        response["result"]["range"]["start"]["character"],
        expected_start
    );
    assert_eq!(response["result"]["range"]["end"]["line"], 0);
    assert_eq!(
        response["result"]["range"]["end"]["character"],
        expected_end
    );
}

fn temp_workspace() -> PathBuf {
    let suffix = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(error) => panic!("system time should be after UNIX_EPOCH: {error}"),
    };
    let sequence = NEXT_WORKSPACE_ID.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "vela_lsp_close_overlay_{}_{}_{}",
        std::process::id(),
        suffix,
        sequence
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
