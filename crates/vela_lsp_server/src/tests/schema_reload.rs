use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::tests::{
    LspServer, handle_notification, handle_request, notification_value, notification_values,
    response_value,
};

static NEXT_WORKSPACE_ID: AtomicU64 = AtomicU64::new(0);

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
    let _ = response_value(handle_request(
        &mut server,
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": file_uri(&root),
            "capabilities": {}
        }),
    ));
    let _ = handle_notification(
        &mut server,
        "workspace/didChangeWatchedFiles",
        serde_json::json!({
            "changes": [{ "uri": file_uri(&config_path), "type": 1 }]
        }),
    );
    let main_uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    let text = "pub fn main(player: Player) { player. }";
    let _ = notification_value(handle_notification(
        &mut server,
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": main_uri,
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    ));
    let position = text.find(". }").expect("member dot should exist") + 1;

    let before = response_value(handle_request(
        &mut server,
        2,
        "textDocument/completion",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": { "line": 0, "character": position }
        }),
    ));
    assert_completion(&before, "level", 5, "i64");
    assert_no_completion(&before, "rank");

    fs::write(&schema_path, schema_with_player_field("rank", "string"))
        .expect("updated schema should be writable");
    let _ = handle_notification(
        &mut server,
        "workspace/didChangeWatchedFiles",
        serde_json::json!({
            "changes": [{ "uri": file_uri(&schema_path), "type": 2 }]
        }),
    );

    let after = response_value(handle_request(
        &mut server,
        3,
        "textDocument/completion",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": { "line": 0, "character": position }
        }),
    ));
    assert_completion(&after, "rank", 5, "String");
    assert_no_completion(&after, "level");
    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn schema_delete_clears_stale_host_completion_and_publishes_diagnostic() {
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
    let _ = response_value(handle_request(
        &mut server,
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": file_uri(&root),
            "capabilities": {}
        }),
    ));
    let _ = handle_notification(
        &mut server,
        "workspace/didChangeWatchedFiles",
        serde_json::json!({
            "changes": [{ "uri": file_uri(&config_path), "type": 1 }]
        }),
    );
    let main_uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    let text = "pub fn main(player: Player) { player. }";
    let _ = notification_value(handle_notification(
        &mut server,
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": main_uri,
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    ));
    let position = text.find(". }").expect("member dot should exist") + 1;

    let before = response_value(handle_request(
        &mut server,
        2,
        "textDocument/completion",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": { "line": 0, "character": position }
        }),
    ));
    assert_completion(&before, "level", 5, "i64");

    fs::remove_file(&schema_path).expect("schema should be removable");
    let notifications = notification_values(handle_notification(
        &mut server,
        "workspace/didChangeWatchedFiles",
        serde_json::json!({
            "changes": [{ "uri": file_uri(&schema_path), "type": 3 }]
        }),
    ));
    assert_document_has_diagnostic_code(&notifications, &main_uri, "schema::unavailable");

    let after = response_value(handle_request(
        &mut server,
        3,
        "textDocument/completion",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": { "line": 0, "character": position }
        }),
    ));
    assert_no_completion(&after, "level");
    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn editor_config_maps_to_workspace_config() {
    let root = temp_workspace();
    let schema_path = root.join("target").join("vela").join("schema.json");
    fs::create_dir_all(schema_path.parent().expect("schema should have parent"))
        .expect("schema directory should be creatable");
    fs::write(&schema_path, schema_with_player_field("level", "i64"))
        .expect("schema should be writable");

    let mut server = LspServer::new();
    let _ = response_value(handle_request(
        &mut server,
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": file_uri(&root),
            "initializationOptions": {
                "workspace": {
                    "roots": [file_uri(&root.join("scripts"))]
                },
                "host": {
                    "schema": file_uri(&schema_path)
                }
            },
            "capabilities": {}
        }),
    ));

    let helper_uri = file_uri(&root.join("scripts").join("game").join("helper.vela"));
    let _ = notification_value(handle_notification(
        &mut server,
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": helper_uri,
                "languageId": "vela",
                "version": 1,
                "text": "pub fn grant() -> i64 { return 1 }"
            }
        }),
    ));

    let main_uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    let text = "\
use game::helper::grant
pub fn main(player: Player) {
    let score = grant()
    return player.level
}";
    let open = notification_value(handle_notification(
        &mut server,
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": main_uri,
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    ));
    assert_eq!(open["params"]["diagnostics"], serde_json::json!([]));

    let response = response_value(handle_request(
        &mut server,
        2,
        "textDocument/completion",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": { "line": 3, "character": "    return player.".len() }
        }),
    ));

    assert_completion(&response, "level", 5, "i64");
    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_workspace_configuration_request_updates_workspace_config() {
    let root = temp_workspace();
    let schema_path = root.join("target").join("vela").join("schema.json");
    fs::create_dir_all(schema_path.parent().expect("schema should have parent"))
        .expect("schema directory should be creatable");
    fs::write(&schema_path, schema_with_player_field("rank", "string"))
        .expect("schema should be writable");

    let mut server = LspServer::new();
    let _ = response_value(handle_request(
        &mut server,
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": file_uri(&root),
            "capabilities": {}
        }),
    ));

    let helper_uri = file_uri(&root.join("scripts").join("game").join("helper.vela"));
    let _ = notification_value(handle_notification(
        &mut server,
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": helper_uri,
                "languageId": "vela",
                "version": 1,
                "text": "pub fn grant() -> i64 { return 1 }"
            }
        }),
    ));

    let main_uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    let text = "\
use game::helper::grant
pub fn main(player: Player) {
    let score = grant()
    return player.rank
}";
    let before = notification_value(handle_notification(
        &mut server,
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": main_uri,
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    ));
    assert_has_diagnostic_code(&before, "hir::unresolved_module");

    let notifications = notification_values(handle_notification(
        &mut server,
        "workspace/didChangeConfiguration",
        serde_json::json!({
            "settings": {
                "vela": {
                    "workspace": {
                        "roots": [file_uri(&root.join("scripts"))]
                    },
                    "host": {
                        "schema": file_uri(&schema_path)
                    }
                }
            }
        }),
    ));
    assert_document_diagnostics(&notifications, &main_uri, serde_json::json!([]));

    let response = response_value(handle_request(
        &mut server,
        2,
        "textDocument/completion",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": { "line": 3, "character": "    return player.".len() }
        }),
    ));

    assert_completion(&response, "rank", 5, "String");
    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

fn assert_has_diagnostic_code(notification: &serde_json::Value, code: &str) {
    let Some(diagnostics) = notification["params"]["diagnostics"].as_array() else {
        panic!("publishDiagnostics should contain diagnostics");
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic["code"] == code),
        "{diagnostics:?}"
    );
}

fn assert_document_diagnostics(
    notifications: &[serde_json::Value],
    uri: &str,
    expected: serde_json::Value,
) {
    let Some(notification) = notifications
        .iter()
        .find(|notification| notification["params"]["uri"] == uri)
    else {
        panic!("expected diagnostics for {uri}");
    };
    assert_eq!(notification["params"]["diagnostics"], expected);
}

fn assert_document_has_diagnostic_code(
    notifications: &[serde_json::Value],
    uri: &str,
    expected_code: &str,
) {
    let Some(notification) = notifications
        .iter()
        .find(|notification| notification["params"]["uri"] == uri)
    else {
        panic!("expected diagnostics for {uri}");
    };
    let Some(diagnostics) = notification["params"]["diagnostics"].as_array() else {
        panic!("publishDiagnostics should contain diagnostics");
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic["code"] == expected_code),
        "{diagnostics:?}"
    );
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
    let sequence = NEXT_WORKSPACE_ID.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "vela_lsp_server_schema_reload_{}_{}_{}",
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
