use super::{JsonRpcResult, LspServer, notification, notification_value, request, response_value};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_WORKSPACE_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy)]
struct SemanticTokenRange {
    line: u64,
    character: u64,
    length: u64,
    token_type: u64,
}

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
fn lsp_did_close_restores_disk_snapshot_semantic_tokens() {
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
    let initialize = response_value(server.handle_json(&request(
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": file_uri(&root.join("scripts")),
            "capabilities": {}
        }),
    )));
    let function = semantic_token_type_index(&initialize, "function");
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

    let overlay_tokens = semantic_tokens(&response_value(server.handle_json(&request(
        2,
        "textDocument/semanticTokens/full",
        serde_json::json!({
            "textDocument": { "uri": source_uri }
        }),
    ))));
    assert_semantic_token_for_target(&overlay_tokens, overlay_source, "overlay_only", function);

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

    let disk_tokens = semantic_tokens(&response_value(server.handle_json(&request(
        3,
        "textDocument/semanticTokens/full",
        serde_json::json!({
            "textDocument": { "uri": source_uri }
        }),
    ))));
    assert_semantic_token_for_target(&disk_tokens, disk_source, "disk_only", function);

    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_did_close_restores_disk_snapshot_inlay_hints() {
    let root = temp_workspace();
    let source_path = root.join("scripts").join("game").join("main.vela");
    let disk_source = r#"pub fn disk_grant(amount: i64) -> i64 { return amount }
pub fn main() -> i64 {
    return disk_grant(7)
}"#;
    let overlay_source = r#"pub fn overlay_grant(reason: String) -> String { return reason }
pub fn main() -> String {
    return overlay_grant("quest")
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

    let overlay_hints =
        response_value(server.handle_json(&inlay_hint_request(2, &source_uri, overlay_source)));
    assert_inlay_hint_for_target(&overlay_hints, overlay_source, "\"quest\"", "reason:");

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

    let disk_hints =
        response_value(server.handle_json(&inlay_hint_request(3, &source_uri, disk_source)));
    assert_inlay_hint_for_target(&disk_hints, disk_source, "7", "amount:");

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

fn semantic_tokens(response: &serde_json::Value) -> Vec<SemanticTokenRange> {
    let data = response["result"]["data"]
        .as_array()
        .expect("semantic token response should include data");
    let mut line = 0;
    let mut character = 0;
    data.chunks_exact(5)
        .map(|chunk| {
            let delta_line = chunk[0].as_u64().expect("line delta should be numeric");
            let delta_start = chunk[1].as_u64().expect("start delta should be numeric");
            line += delta_line;
            if delta_line == 0 {
                character += delta_start;
            } else {
                character = delta_start;
            }
            SemanticTokenRange {
                line,
                character,
                length: chunk[2].as_u64().expect("length should be numeric"),
                token_type: chunk[3].as_u64().expect("token type should be numeric"),
            }
        })
        .collect()
}

fn semantic_token_type_index(initialize: &serde_json::Value, name: &str) -> u64 {
    initialize["result"]["capabilities"]["semanticTokensProvider"]["legend"]["tokenTypes"]
        .as_array()
        .expect("semantic token legend should list token types")
        .iter()
        .position(|token_type| token_type == name)
        .expect("semantic token type should exist") as u64
}

fn assert_semantic_token_for_target(
    tokens: &[SemanticTokenRange],
    source: &str,
    target: &str,
    token_type: u64,
) {
    let (line, character) = source_position(source, target);
    let length = target.len() as u64;
    assert!(
        tokens.iter().any(|token| {
            token.line == line
                && token.character == character
                && token.length == length
                && token.token_type == token_type
        }),
        "missing semantic token for {target:?} at ({line}, {character}) in {tokens:?}"
    );
}

fn source_position(source: &str, target: &str) -> (u64, u64) {
    let offset = source.find(target).expect("target should exist in source");
    let mut line = 0;
    let mut line_start = 0;
    for (index, byte) in source.bytes().enumerate().take(offset) {
        if byte == b'\n' {
            line += 1;
            line_start = index + 1;
        }
    }
    (line, (offset - line_start) as u64)
}

fn inlay_hint_request(id: i64, uri: &str, source: &str) -> String {
    let last_line = source
        .lines()
        .count()
        .checked_sub(1)
        .expect("source should contain at least one line");
    let end_character = source
        .lines()
        .last()
        .expect("source should contain a final line")
        .len();
    request(
        id,
        "textDocument/inlayHint",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": last_line, "character": end_character }
            }
        }),
    )
}

fn assert_inlay_hint_for_target(
    response: &serde_json::Value,
    source: &str,
    target: &str,
    label: &str,
) {
    let (line, character) = source_position(source, target);
    let hints = response["result"]
        .as_array()
        .expect("inlayHint should return an array");
    assert!(
        hints.iter().any(|hint| {
            hint["position"]["line"] == line
                && hint["position"]["character"] == character
                && hint["label"] == label
                && hint["kind"] == 2
                && hint["paddingRight"] == true
        }),
        "missing inlay hint {label:?} at ({line}, {character}) in {hints:?}"
    );
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
