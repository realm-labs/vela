use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::{LspServer, notification, notification_value, request, response_value};

mod cross_file;

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
fn lsp_hover_recovers_parameter_fact_after_body_parse_error() {
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
    let text = "\
pub fn main(amount: i64) -> i64 {
    let value = amount +
    return amount
}";
    let uri = "file:///workspace/scripts/game/main.vela";
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
    let return_line = text.lines().nth(2).expect("return line should exist");

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 2,
                "character": return_line.find("amount").unwrap_or_else(|| {
                    panic!("hover fixture should contain recovered amount use")
                })
            }
        }),
    )));

    let value = response["result"]["contents"]["value"]
        .as_str()
        .expect("hover contents should be markdown");
    assert!(value.contains("amount"), "{value}");
    assert!(value.contains("_parameter_: i64"), "{value}");
}

#[test]
fn lsp_hover_degrades_to_any_without_schema() {
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
    let text = "pub fn main(player: Player) { return player }";
    let uri = "file:///workspace/scripts/game/main.vela";
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
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 0,
                "character": text.find("Player").expect("type hint")
            }
        }),
    )));

    let value = response["result"]["contents"]["value"]
        .as_str()
        .expect("hover contents should be markdown");
    assert!(value.contains("```vela\nPlayer\n```"), "{value}");
    assert!(value.contains("_type_: Any"), "{value}");
}

#[test]
fn lsp_hover_returns_null_for_unresolved_and_dynamic_members() {
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
    let text = "\
pub fn unresolved() { return missing }
pub fn dynamic(player) { return player.level }";
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

    let unresolved_line = text
        .lines()
        .next()
        .expect("fixture should contain unresolved name");
    let unresolved = response_value(server.handle_json(&request(
        2,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 0,
                "character": unresolved_line.find("missing").unwrap_or_else(|| {
                    panic!("hover fixture should contain unresolved name")
                })
            }
        }),
    )));
    assert!(unresolved["result"].is_null(), "{unresolved:?}");

    let dynamic_line = text
        .lines()
        .nth(1)
        .expect("fixture should contain dynamic member");
    let dynamic = response_value(server.handle_json(&request(
        3,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 1,
                "character": dynamic_line.find("level").unwrap_or_else(|| {
                    panic!("hover fixture should contain dynamic member")
                })
            }
        }),
    )));
    assert!(dynamic["result"].is_null(), "{dynamic:?}");
}

#[test]
fn lsp_hover_reports_source_global_fact() {
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
    let text = "global score: i64\npub fn main() -> i64 { return score }";
    let uri = "file:///workspace/scripts/game/main.vela";
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
    let use_line = text.lines().nth(1).expect("global use line should exist");

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 1,
                "character": use_line.find("score").unwrap_or_else(|| {
                    panic!("hover fixture should contain global use")
                })
            }
        }),
    )));

    let value = response["result"]["contents"]["value"]
        .as_str()
        .expect("hover contents should be markdown");
    assert!(value.contains("game::main::score"), "{value}");
    assert!(value.contains("_global_: i64"), "{value}");
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
fn lsp_hover_reports_imported_module_path_fact() {
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
    let reward_uri = "file:///workspace/scripts/game/reward.vela";
    let main_uri = "file:///workspace/scripts/game/main.vela";
    let main_text = "use game::reward::grant\npub fn main() { return grant() }";
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": reward_uri,
                "languageId": "vela",
                "version": 1,
                "text": "pub fn grant() -> i64 { return 1 }"
            }
        }),
    )));
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": main_uri,
                "languageId": "vela",
                "version": 1,
                "text": main_text
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
                "character": main_text.find("reward").unwrap_or_else(|| {
                    panic!("hover fixture should contain module segment")
                })
            }
        }),
    )));

    let value = response["result"]["contents"]["value"]
        .as_str()
        .expect("hover contents should be markdown");
    assert!(value.contains("game::reward"), "{value}");
    assert!(value.contains("_module_: module game::reward"), "{value}");
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
fn lsp_hover_reports_source_trait_default_method_on_source_function_return_receiver() {
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
    fn preview(self, amount: i64) -> bool { return amount > 0 }
}
struct Player {
    level: i64,
}
impl Rewardable for Player {}
fn current_player() -> Player { return Player { level: 1 } }
pub fn main() {
    return current_player().preview(1)
}"#;
    let method_line = text
        .lines()
        .nth(10)
        .expect("trait default method use line should exist");
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
                "line": 10,
                "character": method_line.find("preview").unwrap_or_else(|| {
                    panic!("hover fixture should contain trait default method use")
                })
            }
        }),
    )));

    let value = response["result"]["contents"]["value"]
        .as_str()
        .expect("hover contents should be markdown");
    assert!(value.contains("game::main::Rewardable.preview"), "{value}");
    assert!(
        value.contains("_method_: (self, amount: i64) -> bool"),
        "{value}"
    );
    assert!(value.contains("Preview reward"), "{value}");
}

#[test]
fn lsp_hover_reports_source_trait_fact() {
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
    let text = r#"#[doc("Rewardable script trait")]
trait Rewardable {
    fn preview(amount: i64) -> bool
}
pub fn main(rewardable: Rewardable) {
    return rewardable
}"#;
    let uri = "file:///workspace/scripts/game/main.vela";
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
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 1,
                "character": text.lines().nth(1).unwrap_or_default().find("Rewardable").unwrap_or(0)
            }
        }),
    )));

    let value = response["result"]["contents"]["value"]
        .as_str()
        .expect("hover contents should be markdown");
    assert!(value.contains("game::main::Rewardable"), "{value}");
    assert!(value.contains("_trait_: game::main::Rewardable"), "{value}");
    assert!(value.contains("Rewardable script trait"), "{value}");
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
fn lsp_hover_reports_effects_and_permissions() {
    let root = temp_workspace();
    let schema_path = root.join("target").join("vela").join("schema.json");
    fs::create_dir_all(schema_path.parent().expect("schema should have parent"))
        .expect("schema directory should be creatable");
    fs::write(&schema_path, schema_with_player_grant_method())
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
    let text = "pub fn main(player: Player) { player.grant(1) }";
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
                "character": text.find("grant").unwrap_or_else(|| {
                    panic!("hover fixture should contain schema method")
                })
            }
        }),
    )));

    let value = response["result"]["contents"]["value"]
        .as_str()
        .expect("hover contents should be markdown");
    assert!(value.contains("Player.grant"), "{value}");
    assert!(value.contains("_method_: Function(i64) -> bool"), "{value}");
    assert!(value.contains("effects: writes_host"), "{value}");
    assert!(value.contains("permissions: player.reward"), "{value}");
    assert!(value.contains("Grant player rewards."), "{value}");
    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
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
    assert!(value.contains("Preview a reward."), "{value}");
    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_hover_reports_schema_trait_fact() {
    let root = temp_workspace();
    let schema_path = root.join("target").join("vela").join("schema.json");
    fs::create_dir_all(schema_path.parent().expect("schema should have parent"))
        .expect("schema directory should be creatable");
    fs::write(&schema_path, schema_with_rewardable_trait())
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
    let uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    let text = "pub fn main(rewardable: Rewardable) { return rewardable }";
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
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 0,
                "character": text.find("Rewardable").unwrap_or_else(|| {
                    panic!("hover fixture should contain schema trait")
                })
            }
        }),
    )));

    let value = response["result"]["contents"]["value"]
        .as_str()
        .expect("hover contents should be markdown");
    assert!(value.contains("Rewardable"), "{value}");
    assert!(value.contains("_trait_: Rewardable"), "{value}");
    assert!(value.contains("Rewardable host trait."), "{value}");
    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_hover_reports_schema_enum_variant_fact() {
    let root = temp_workspace();
    let schema_path = root.join("target").join("vela").join("schema.json");
    fs::create_dir_all(schema_path.parent().expect("schema should have parent"))
        .expect("schema directory should be creatable");
    fs::write(&schema_path, schema_with_quest_state_variant())
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
    let text = "pub fn main() { return QuestState::Active }";
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
                "character": text.find("Active").unwrap_or_else(|| {
                    panic!("hover fixture should contain schema variant")
                })
            }
        }),
    )));

    let value = response["result"]["contents"]["value"]
        .as_str()
        .expect("hover contents should be markdown");
    assert!(value.contains("QuestState::Active"), "{value}");
    assert!(value.contains("_variant_: QuestState::Active"), "{value}");
    assert!(value.contains("Active quest state."), "{value}");
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

fn schema_with_player_grant_method() -> &'static str {
    r#"{
        "formatVersion": 1,
        "facts": {
            "types": [
                {
                    "name": "Player",
                    "fact": { "kind": "host", "name": "Player" }
                }
            ],
            "methods": [
                {
                    "owner": "Player",
                    "name": "grant",
                    "fact": {
                        "kind": "function",
                        "params": [{ "kind": "primitive", "name": "i64" }],
                        "returns": { "kind": "primitive", "name": "bool" }
                    },
                    "docs": "Grant player rewards."
                }
            ],
            "methodEffects": [
                {
                    "owner": "Player",
                    "name": "grant",
                    "effect": {
                        "readsHost": false,
                        "writesHost": true,
                        "emitsEvents": false,
                        "readsTime": false,
                        "usesRandom": false,
                        "readsIo": false,
                        "writesIo": false,
                        "readsReflection": false,
                        "writesReflection": false,
                        "callsReflection": false
                    }
                }
            ],
            "methodAccess": [
                {
                    "owner": "Player",
                    "name": "grant",
                    "public": true,
                    "reflect_callable": true,
                    "required_permissions": ["player.reward"]
                }
            ]
        }
    }"#
}

fn schema_with_rewardable_trait_method() -> &'static str {
    r#"{
        "formatVersion": 1,
        "facts": {
            "traits": [
                {
                    "name": "Rewardable",
                    "fact": { "kind": "trait", "name": "Rewardable" },
                    "docs": "Rewardable host trait."
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
                    },
                    "docs": "Preview a reward."
                }
            ]
        }
    }"#
}

fn schema_with_rewardable_trait() -> &'static str {
    r#"{
        "formatVersion": 1,
        "facts": {
            "traits": [
                {
                    "name": "Rewardable",
                    "fact": { "kind": "trait", "name": "Rewardable" },
                    "docs": "Rewardable host trait."
                }
            ]
        }
    }"#
}

fn schema_with_quest_state_variant() -> &'static str {
    r#"{
        "formatVersion": 1,
        "facts": {
            "types": [
                {
                    "name": "QuestState",
                    "fact": { "kind": "enum", "name": "QuestState", "variant": null }
                }
            ],
            "variants": [
                {
                    "owner": "QuestState",
                    "name": "Active",
                    "fact": { "kind": "enum", "name": "QuestState", "variant": "Active" },
                    "docs": "Active quest state."
                }
            ]
        }
    }"#
}
