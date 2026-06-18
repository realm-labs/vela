use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{LspServer, notification, notification_value, request, response_value};

#[test]
fn lsp_completion_uses_short_type_labels_with_owner_details() {
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
                            "name": "game::schema::Region",
                            "fact": { "kind": "host", "name": "game::schema::Region" }
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
            "capabilities": {
                "textDocument": {
                    "completion": {
                        "completionItem": {
                            "labelDetailsSupport": true
                        }
                    }
                }
            }
        }),
    )));
    let _ = server.handle_json(&notification(
        "workspace/didChangeWatchedFiles",
        serde_json::json!({
            "changes": [{ "uri": file_uri(&config_path), "type": 1 }]
        }),
    ));
    let main_uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": main_uri,
                "languageId": "vela",
                "version": 1,
                "text": "pub fn main() { Re }"
            }
        }),
    )));
    let reward_uri = file_uri(&root.join("scripts").join("game").join("reward.vela"));
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": reward_uri,
                "languageId": "vela",
                "version": 1,
                "text": "pub struct Reward { amount: i64 }"
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/completion",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": { "line": 0, "character": "pub fn main() { Re".len() }
        }),
    )));

    assert_type_completion(
        &response,
        TypeCompletion {
            label: "Reward",
            detail: "game::reward::Reward",
            filter_text: "game::reward::Reward",
            owner: "game::reward",
            new_text: "Reward",
        },
    );
    assert_no_completion(&response, "game::reward::Reward");
    assert_type_completion(
        &response,
        TypeCompletion {
            label: "Region",
            detail: "game::schema::Region",
            filter_text: "game::schema::Region",
            owner: "game::schema",
            new_text: "Region",
        },
    );
    assert_no_completion(&response, "game::schema::Region");
    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

struct TypeCompletion<'a> {
    label: &'a str,
    detail: &'a str,
    filter_text: &'a str,
    owner: &'a str,
    new_text: &'a str,
}

fn assert_type_completion(response: &serde_json::Value, expected: TypeCompletion<'_>) {
    let item = completion_item(response, expected.label);
    assert_eq!(item["kind"], 22);
    assert_eq!(item["detail"], expected.detail);
    assert_eq!(item["filterText"], expected.filter_text);
    assert_eq!(
        item["labelDetails"],
        serde_json::json!({
            "detail": expected.detail,
            "description": expected.owner
        })
    );
    assert_eq!(item["textEdit"]["newText"], expected.new_text);
    assert_eq!(
        item["textEdit"]["range"],
        serde_json::json!({
            "start": { "line": 0, "character": 16 },
            "end": { "line": 0, "character": 18 }
        })
    );
}

fn completion_item<'a>(response: &'a serde_json::Value, label: &str) -> &'a serde_json::Value {
    let Some(items) = response["result"]["items"].as_array() else {
        panic!("completion response should contain items");
    };
    items
        .iter()
        .find(|item| item["label"] == label)
        .unwrap_or_else(|| panic!("completion response should include {label}: {items:?}"))
}

fn assert_no_completion(response: &serde_json::Value, label: &str) {
    let Some(items) = response["result"]["items"].as_array() else {
        panic!("completion response should contain items");
    };
    assert!(items.iter().all(|item| item["label"] != label), "{items:?}");
}

fn temp_workspace() -> PathBuf {
    let mut path = std::env::temp_dir();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    path.push(format!("vela_lsp_completion_type_{nanos}"));
    path
}

fn file_uri(path: &std::path::Path) -> String {
    format!("file://{}", path.display())
}
