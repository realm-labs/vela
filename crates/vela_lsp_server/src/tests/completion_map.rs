use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::{LspServer, handle_notification, handle_request, notification_value, response_value};

#[test]
fn lsp_map_key_completion_suggests_schema_enum_variants() {
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
                        "name": "QuestState",
                        "fact": { "kind": "enum", "name": "QuestState" }
                    }
                ],
                "variants": [
                    {
                        "owner": "QuestState",
                        "name": "Started",
                        "fact": {
                            "kind": "enum",
                            "name": "QuestState",
                            "variant": "Started"
                        }
                    },
                    {
                        "owner": "QuestState",
                        "name": "Completed",
                        "fact": {
                            "kind": "enum",
                            "name": "QuestState",
                            "variant": "Completed"
                        }
                    }
                ]
            }
        }"#,
    )
    .expect("schema should be writable");

    let main_text = r#"pub fn main() {
    let rewards: Map<QuestState, i64> = {
        Started: 1,
        Co: 2,
    }
}"#;
    let main_uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
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
    let _ = notification_value(handle_notification(
        &mut server,
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": main_uri,
                "languageId": "vela",
                "version": 1,
                "text": main_text
            }
        }),
    ));

    let response = response_value(handle_request(
        &mut server,
        2,
        "textDocument/completion",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 3,
                "character": main_text
                    .lines()
                    .nth(3)
                    .expect("map key line")
                    .find("Co:")
                    .expect("map key prefix") + "Co".len()
            }
        }),
    ));

    let items = response["result"]["items"]
        .as_array()
        .expect("completion response should contain items");
    assert!(items.iter().any(|item| {
        item["label"] == "Completed" && item["kind"] == 20 && item["detail"] == "QuestState"
    }));
    assert!(items.iter().all(|item| item["label"] != "Started"));
    fs::remove_dir_all(root).expect("temporary workspace should be removable");
}

fn temp_workspace() -> PathBuf {
    let suffix = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(error) => panic!("system time should be after UNIX_EPOCH: {error}"),
    };
    let root = std::env::temp_dir().join(format!(
        "vela_lsp_server_completion_map_{}_{}",
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
