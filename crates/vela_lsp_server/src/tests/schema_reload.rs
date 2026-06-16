use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::{LspServer, notification, notification_value, request, response_value};

#[test]
fn schema_reload_updates_host_member_completion() {
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
    fs::write(&schema_path, schema_with_player_field("level", "i64"))
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
    let main_uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    let text = "pub fn main(player: Player) { player. }";
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": main_uri,
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    )));
    let position = text.find(". }").expect("member dot should exist") + 1;

    let before = response_value(server.handle_json(&request(
        2,
        "textDocument/completion",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": { "line": 0, "character": position }
        }),
    )));
    assert_completion(&before, "level", 5, "i64");
    assert_no_completion(&before, "rank");

    fs::write(&schema_path, schema_with_player_field("rank", "string"))
        .expect("updated schema should be writable");
    let _ = server.handle_json(&notification(
        "workspace/didChangeWatchedFiles",
        serde_json::json!({
            "changes": [{ "uri": file_uri(&schema_path), "type": 2 }]
        }),
    ));

    let after = response_value(server.handle_json(&request(
        3,
        "textDocument/completion",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": { "line": 0, "character": position }
        }),
    )));
    assert_completion(&after, "rank", 5, "String");
    assert_no_completion(&after, "level");
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

fn assert_no_completion(response: &serde_json::Value, label: &str) {
    assert_eq!(response["result"]["isIncomplete"], false);
    let Some(items) = response["result"]["items"].as_array() else {
        panic!("completion response should contain items");
    };
    assert!(items.iter().all(|item| item["label"] != label), "{items:?}");
}

fn temp_workspace() -> PathBuf {
    let suffix = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(error) => panic!("system time should be after UNIX_EPOCH: {error}"),
    };
    let root = std::env::temp_dir().join(format!(
        "vela_lsp_server_schema_reload_{}_{}",
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
