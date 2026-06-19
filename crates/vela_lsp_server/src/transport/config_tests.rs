use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crossbeam_channel::unbounded;
use lsp_server::{Connection, Message};

use crate::LaunchConfiguration;

static NEXT_WORKSPACE_ID: AtomicU64 = AtomicU64::new(0);

#[test]
fn typed_dispatcher_routes_configuration_changes_through_global_state() {
    let root = temp_workspace("typed_global_state_config");
    let schema_path = root.join("target").join("vela").join("schema.json");
    fs::create_dir_all(schema_path.parent().expect("schema should have parent"))
        .expect("schema directory should be creatable");
    fs::write(&schema_path, schema_with_player_field("rank", "string"))
        .expect("schema should be writable");
    let main_uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    let text = "pub fn main(player: Player) { player. }";
    let completion_position = text.find(". }").expect("member dot should exist") + 1;

    let (client_sender, server_receiver) = unbounded::<Message>();
    let (server_sender, client_receiver) = unbounded::<Message>();
    let connection = Connection {
        sender: server_sender,
        receiver: server_receiver,
    };

    client_sender
        .send(message(serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "processId": null,
                "rootUri": file_uri(&root),
                "capabilities": {}
            }
        })))
        .expect("initialize should be sent");
    client_sender
        .send(message(serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": main_uri,
                    "languageId": "vela",
                    "version": 1,
                    "text": text
                }
            }
        })))
        .expect("didOpen should be sent");
    client_sender
        .send(message(serde_json::json!({
            "jsonrpc": "2.0",
            "method": "workspace/didChangeConfiguration",
            "params": {
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
            }
        })))
        .expect("didChangeConfiguration should be sent");
    client_sender
        .send(message(serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": main_uri },
                "position": { "line": 0, "character": completion_position }
            }
        })))
        .expect("completion should be sent");
    client_sender
        .send(message(serde_json::json!({
            "jsonrpc": "2.0",
            "method": "exit"
        })))
        .expect("exit should be sent");
    drop(client_sender);

    super::run_connection(connection, LaunchConfiguration::new())
        .expect("typed connection should run");

    let responses = client_receiver
        .try_iter()
        .filter_map(|message| match message {
            Message::Response(response) => Some(response),
            Message::Request(_) | Message::Notification(_) => None,
        })
        .collect::<Vec<_>>();
    let completion = responses
        .iter()
        .find(|response| response.id.to_string() == "2")
        .unwrap_or_else(|| panic!("completion response should be present: {responses:?}"));
    assert!(completion.error.is_none(), "{completion:?}");
    assert_completion(
        completion
            .result
            .as_ref()
            .expect("completion should produce a result"),
        "rank",
        5,
        "String",
    );

    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn typed_dispatcher_routes_watched_config_changes_through_global_state() {
    let root = temp_workspace("typed_global_state_watched_config");
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
    let main_uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    let text = "pub fn main(player: Player) { player. }";
    let completion_position = text.find(". }").expect("member dot should exist") + 1;

    let (client_sender, server_receiver) = unbounded::<Message>();
    let (server_sender, client_receiver) = unbounded::<Message>();
    let connection = Connection {
        sender: server_sender,
        receiver: server_receiver,
    };

    client_sender
        .send(message(serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "processId": null,
                "rootUri": file_uri(&root),
                "capabilities": {}
            }
        })))
        .expect("initialize should be sent");
    client_sender
        .send(message(serde_json::json!({
            "jsonrpc": "2.0",
            "method": "workspace/didChangeWatchedFiles",
            "params": {
                "changes": [{ "uri": file_uri(&config_path), "type": 1 }]
            }
        })))
        .expect("watched config change should be sent");
    client_sender
        .send(message(serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": main_uri,
                    "languageId": "vela",
                    "version": 1,
                    "text": text
                }
            }
        })))
        .expect("didOpen should be sent");
    client_sender
        .send(message(serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": main_uri },
                "position": { "line": 0, "character": completion_position }
            }
        })))
        .expect("completion should be sent");
    client_sender
        .send(message(serde_json::json!({
            "jsonrpc": "2.0",
            "method": "exit"
        })))
        .expect("exit should be sent");
    drop(client_sender);

    super::run_connection(connection, LaunchConfiguration::new())
        .expect("typed connection should run");

    let responses = client_receiver
        .try_iter()
        .filter_map(|message| match message {
            Message::Response(response) => Some(response),
            Message::Request(_) | Message::Notification(_) => None,
        })
        .collect::<Vec<_>>();
    let completion = responses
        .iter()
        .find(|response| response.id.to_string() == "2")
        .unwrap_or_else(|| panic!("completion response should be present: {responses:?}"));
    assert!(completion.error.is_none(), "{completion:?}");
    assert_completion(
        completion
            .result
            .as_ref()
            .expect("completion should produce a result"),
        "level",
        5,
        "i64",
    );

    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

fn message(value: serde_json::Value) -> Message {
    serde_json::from_value(value).expect("test message should be typed LSP")
}

fn assert_completion(response: &serde_json::Value, label: &str, kind: u8, detail: &str) {
    assert_eq!(response["isIncomplete"], false);
    let Some(items) = response["items"].as_array() else {
        panic!("completion response should contain items");
    };
    assert!(
        items
            .iter()
            .any(|item| item["label"] == label && item["kind"] == kind && item["detail"] == detail),
        "{items:?}"
    );
}

fn temp_workspace(name: &str) -> PathBuf {
    let suffix = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(error) => panic!("system time should be after UNIX_EPOCH: {error}"),
    };
    let sequence = NEXT_WORKSPACE_ID.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "vela_lsp_transport_{name}_{}_{}_{}",
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
