use super::{LspServer, notification, notification_value, request, response_value};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn lsp_inlay_hints_show_parameter_names() {
    let mut server = LspServer::new();
    let _ = response_value(server.handle_json(&request(
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": "file:///workspace/scripts",
            "capabilities": {}
        }),
    )));
    let uri = "file:///workspace/scripts/game/main.vela";
    let text = "pub fn grant(amount: i64, reason: String) -> i64 { return amount }\npub fn main() { return grant(10, \"quest\") }";
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/inlayHint",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 1, "character": 0 },
                "end": { "line": 1, "character": 80 }
            }
        }),
    )));
    let hints = response["result"]
        .as_array()
        .expect("inlayHint should return an array");

    assert_eq!(hints.len(), 2);
    assert_eq!(hints[0]["position"]["line"], 1);
    assert_eq!(hints[0]["position"]["character"], 29);
    assert_eq!(hints[0]["label"], "amount:");
    assert_eq!(hints[0]["kind"], 2);
    assert_eq!(hints[0]["paddingRight"], true);
    assert_eq!(hints[1]["position"]["character"], 33);
    assert_eq!(hints[1]["label"], "reason:");
}

#[test]
fn lsp_inlay_hints_respect_requested_range() {
    let mut server = LspServer::new();
    let _ = response_value(server.handle_json(&request(
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": "file:///workspace/scripts",
            "capabilities": {}
        }),
    )));
    let uri = "file:///workspace/scripts/game/main.vela";
    let text = "pub fn grant(amount: i64, reason: String) -> i64 { return amount }\npub fn main() { return grant(10, \"quest\") }";
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/inlayHint",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 1, "character": 31 },
                "end": { "line": 1, "character": 80 }
            }
        }),
    )));

    assert_eq!(
        response["result"],
        serde_json::json!([{
            "position": { "line": 1, "character": 33 },
            "label": "reason:",
            "kind": 2,
            "paddingRight": true
        }])
    );
}

#[test]
fn lsp_inlay_hints_show_local_typefacts() {
    let mut server = LspServer::new();
    let _ = response_value(server.handle_json(&request(
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": "file:///workspace/scripts",
            "capabilities": {}
        }),
    )));
    let uri = "file:///workspace/scripts/game/main.vela";
    let text = r#"const BONUS: i64 = 10
pub fn main() {
    let total = 1 + 2;
    let next = total + 1;
    let scripted = BONUS;
    let explicit: i64 = 3;
}"#;
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/inlayHint",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 6, "character": 0 }
            }
        }),
    )));

    assert_eq!(
        response["result"],
        serde_json::json!([
            {
                "position": { "line": 2, "character": 13 },
                "label": ": i64",
                "kind": 1,
                "paddingRight": true
            },
            {
                "position": { "line": 3, "character": 12 },
                "label": ": i64",
                "kind": 1,
                "paddingRight": true
            },
            {
                "position": { "line": 4, "character": 16 },
                "label": ": i64",
                "kind": 1,
                "paddingRight": true
            }
        ])
    );
}

#[test]
fn lsp_inlay_hints_show_lambda_parameter_facts() {
    let mut server = LspServer::new();
    let _ = response_value(server.handle_json(&request(
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": "file:///workspace/scripts",
            "capabilities": {}
        }),
    )));
    let uri = "file:///workspace/scripts/game/main.vela";
    let text = r#"pub fn main() {
    let scores: Array<i64> = [1, 2, 3];
    let doubled: Array<i64> = scores.map(|score| score + 1);
    let rewards: Map<String, i64> = {"gold": 1};
    let mapped: Map<String, i64> = rewards.map_values(|value| value + 1);
    let filtered: Map<String, i64> = rewards.filter(|key, value| key.len() > value);
}"#;
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/inlayHint",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 7, "character": 0 }
            }
        }),
    )));

    assert_eq!(
        response["result"],
        serde_json::json!([
            {
                "position": { "line": 2, "character": 47 },
                "label": ": i64",
                "kind": 1,
                "paddingRight": true
            },
            {
                "position": { "line": 4, "character": 60 },
                "label": ": i64",
                "kind": 1,
                "paddingRight": true
            },
            {
                "position": { "line": 5, "character": 56 },
                "label": ": String",
                "kind": 1,
                "paddingRight": true
            },
            {
                "position": { "line": 5, "character": 63 },
                "label": ": i64",
                "kind": 1,
                "paddingRight": true
            }
        ])
    );
}

#[test]
fn lsp_inlay_hints_show_host_path_typefacts() {
    let root = temp_workspace();
    let config_path = root.join("vela.toml");
    let schema_path = root.join("target").join("vela").join("schema.json");
    fs::create_dir_all(schema_path.parent().expect("schema should have parent"))
        .expect("schema directory should be creatable");
    fs::write(
        &config_path,
        r#"
            [workspace]
            roots = ["scripts"]

            [host]
            schema = "target/vela/schema.json"
        "#,
    )
    .expect("vela.toml should be writable");
    fs::write(
        &schema_path,
        r#"{
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
                        "name": "level",
                        "fact": { "kind": "primitive", "name": "i64" }
                    },
                    {
                        "owner": "Player",
                        "name": "mystery",
                        "fact": { "kind": "any" }
                    }
                ],
                "methods": [
                    {
                        "owner": "Player",
                        "name": "grant",
                        "fact": {
                            "kind": "function",
                            "params": [{ "kind": "primitive", "name": "i64" }],
                            "returns": { "kind": "primitive", "name": "i64" }
                        }
                    }
                ]
            }
        }"#,
    )
    .expect("schema should be writable");

    let mut server = LspServer::new();
    let _ = response_value(server.handle_json(&request(
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": file_uri(&root),
            "capabilities": {}
        }),
    )));
    let _ = server.handle_json(&notification(
        "workspace/didChangeWatchedFiles",
        serde_json::json!({
            "changes": [{ "uri": file_uri(&config_path), "type": 1 }]
        }),
    ));
    let uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    let text = r#"pub fn main(player: Player) {
    let next = player.level + 1;
    player.level += next;
    let dynamic = player.mystery;
    player.grant(next);
}"#;
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/inlayHint",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 6, "character": 0 }
            }
        }),
    )));

    assert_eq!(
        response["result"],
        serde_json::json!([
            {
                "position": { "line": 1, "character": 12 },
                "label": ": i64",
                "kind": 1,
                "paddingRight": true
            },
            {
                "position": { "line": 1, "character": 27 },
                "label": ": i64",
                "kind": 1,
                "paddingRight": true
            },
            {
                "position": { "line": 2, "character": 16 },
                "label": ": i64",
                "kind": 1,
                "paddingRight": true
            }
        ])
    );

    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

fn temp_workspace() -> PathBuf {
    let suffix = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(error) => panic!("system time should be after UNIX_EPOCH: {error}"),
    };
    let root =
        std::env::temp_dir().join(format!("vela_lsp_inlay_{}_{}", std::process::id(), suffix));
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
