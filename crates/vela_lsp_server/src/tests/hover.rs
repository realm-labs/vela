use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::{LspServer, notification, notification_value, request, response_value};

#[test]
fn lsp_hover_reports_open_overlay_parameter_fact() {
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
    let text = "pub fn main(amount: i64) -> i64 { return amount }";
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": "file:///workspace/scripts/game/main.vela",
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": "file:///workspace/scripts/game/main.vela" },
            "position": {
                "line": 0,
                "character": text.rfind("amount").unwrap_or_else(|| {
                    panic!("hover fixture should contain amount use")
                })
            }
        }),
    )));

    assert_eq!(response["result"]["range"]["start"]["line"], 0);
    assert_eq!(
        response["result"]["contents"]["kind"],
        serde_json::json!("markdown")
    );
    let value = response["result"]["contents"]["value"]
        .as_str()
        .expect("hover contents should be markdown");
    assert!(value.contains("amount"), "{value}");
    assert!(value.contains("_parameter_: i64"), "{value}");
}

#[test]
fn lsp_hover_reports_stdlib_function_fact() {
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
    let text = "pub fn main() { math::max(1, 2) }";
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": "file:///workspace/scripts/game/main.vela",
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": "file:///workspace/scripts/game/main.vela" },
            "position": {
                "line": 0,
                "character": text.find("max").unwrap_or_else(|| {
                    panic!("hover fixture should contain max")
                })
            }
        }),
    )));

    let value = response["result"]["contents"]["value"]
        .as_str()
        .expect("hover contents should be markdown");
    assert!(value.contains("math::max"), "{value}");
    assert!(
        value.contains("_function_: Function(i64 | f64, i64 | f64) -> i64 | f64"),
        "{value}"
    );
}

#[test]
fn lsp_hover_reports_stdlib_method_fact() {
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
    let text = "pub fn main(scores: Array<i64>) { scores.filter(|score| score > 0) }";
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": "file:///workspace/scripts/game/main.vela",
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": "file:///workspace/scripts/game/main.vela" },
            "position": {
                "line": 0,
                "character": text.find("filter").unwrap_or_else(|| {
                    panic!("hover fixture should contain filter")
                })
            }
        }),
    )));

    let value = response["result"]["contents"]["value"]
        .as_str()
        .expect("hover contents should be markdown");
    assert!(value.contains("Array(i64).filter"), "{value}");
    assert!(
        value.contains("_method_: Function(Function(i64) -> bool) -> Array(i64)"),
        "{value}"
    );
}

#[test]
fn lsp_hover_reports_source_struct_field_fact() {
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
    let text = r#"struct Player {
    #[doc("Current level")]
    level: i64,
}
pub fn main(player: Player) {
    return player.level
}"#;
    let field_line = text.lines().nth(5).expect("field use line should exist");
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": "file:///workspace/scripts/game/main.vela",
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": "file:///workspace/scripts/game/main.vela" },
            "position": {
                "line": 5,
                "character": field_line.find("level").unwrap_or_else(|| {
                    panic!("hover fixture should contain field use")
                })
            }
        }),
    )));

    let value = response["result"]["contents"]["value"]
        .as_str()
        .expect("hover contents should be markdown");
    assert!(value.contains("game::main::Player.level"), "{value}");
    assert!(value.contains("_field_: i64"), "{value}");
    assert!(value.contains("Current level"), "{value}");
}

#[test]
fn lsp_hover_reports_source_method_fact() {
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
    let text = r#"struct Player {
    level: i64,
}
impl Player {
    fn grant(amount: i64) -> bool {
        return amount > 0
    }
}
pub fn main(player: Player) {
    return player.grant(3)
}"#;
    let method_line = text.lines().nth(9).expect("method use line should exist");
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": "file:///workspace/scripts/game/main.vela",
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": "file:///workspace/scripts/game/main.vela" },
            "position": {
                "line": 9,
                "character": method_line.find("grant").unwrap_or_else(|| {
                    panic!("hover fixture should contain method use")
                })
            }
        }),
    )));

    let value = response["result"]["contents"]["value"]
        .as_str()
        .expect("hover contents should be markdown");
    assert!(value.contains("game::main::Player.grant"), "{value}");
    assert!(value.contains("_method_: (amount: i64) -> bool"), "{value}");
}

#[test]
fn lsp_hover_reports_source_trait_receiver_method_fact() {
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
    let text = r#"trait Rewardable {
    #[doc("Preview reward")]
    fn preview(amount: i64) -> bool
}
pub fn main(rewardable: Rewardable) {
    return rewardable.preview(1)
}"#;
    let method_line = text
        .lines()
        .nth(5)
        .expect("trait method use line should exist");
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": "file:///workspace/scripts/game/main.vela",
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": "file:///workspace/scripts/game/main.vela" },
            "position": {
                "line": 5,
                "character": method_line.find("preview").unwrap_or_else(|| {
                    panic!("hover fixture should contain trait method use")
                })
            }
        }),
    )));

    let value = response["result"]["contents"]["value"]
        .as_str()
        .expect("hover contents should be markdown");
    assert!(value.contains("game::main::Rewardable.preview"), "{value}");
    assert!(value.contains("_method_: (amount: i64) -> bool"), "{value}");
    assert!(value.contains("Preview reward"), "{value}");
}

#[test]
fn lsp_hover_reports_source_enum_variant_fact() {
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
    let text = r#"enum QuestState {
    Active(quest_id: String, count: i64),
    Done,
}
pub fn main() {
    return QuestState::Active("quest-1", 3)
}"#;
    let variant_line = text.lines().nth(5).expect("variant use line should exist");
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": "file:///workspace/scripts/game/main.vela",
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": "file:///workspace/scripts/game/main.vela" },
            "position": {
                "line": 5,
                "character": variant_line.find("Active").unwrap_or_else(|| {
                    panic!("hover fixture should contain variant constructor")
                })
            }
        }),
    )));

    let value = response["result"]["contents"]["value"]
        .as_str()
        .expect("hover contents should be markdown");
    assert!(value.contains("game::main::QuestState::Active"), "{value}");
    assert!(
        value.contains("_variant_: game::main::QuestState::Active(quest_id, count)"),
        "{value}"
    );
}

#[test]
fn lsp_hover_reports_schema_trait_method_fact() {
    let root = temp_workspace();
    let schema_path = root.join("target").join("vela").join("schema.json");
    fs::create_dir_all(schema_path.parent().expect("schema should have parent"))
        .expect("schema directory should be creatable");
    fs::write(&schema_path, schema_with_rewardable_trait_method())
        .expect("schema artifact should be writable");

    let mut server = LspServer::new();
    let _ = response_value(server.handle_json(&request(
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
    )));
    let main_uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    let text = "pub fn main(rewardable: Rewardable) { rewardable.preview(1) }";
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

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 0,
                "character": text.find("preview").unwrap_or_else(|| {
                    panic!("hover fixture should contain trait method")
                })
            }
        }),
    )));

    let value = response["result"]["contents"]["value"]
        .as_str()
        .expect("hover contents should be markdown");
    assert!(value.contains("Rewardable.preview"), "{value}");
    assert!(value.contains("_method_: Function(i64) -> bool"), "{value}");
    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

fn temp_workspace() -> PathBuf {
    let suffix = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(error) => panic!("system time should be after UNIX_EPOCH: {error}"),
    };
    let root =
        std::env::temp_dir().join(format!("vela_lsp_hover_{}_{}", std::process::id(), suffix));
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

fn schema_with_rewardable_trait_method() -> &'static str {
    r#"{
        "formatVersion": 1,
        "facts": {
            "traits": [
                {
                    "name": "Rewardable",
                    "fact": { "kind": "trait", "name": "Rewardable" }
                }
            ],
            "traitMethods": [
                {
                    "owner": "Rewardable",
                    "name": "preview",
                    "fact": {
                        "kind": "function",
                        "params": [{ "kind": "primitive", "name": "i64" }],
                        "returns": { "kind": "primitive", "name": "bool" }
                    }
                }
            ]
        }
    }"#
}
