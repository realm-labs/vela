use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::{LspServer, notification, notification_value, request, response_value};

#[test]
fn lsp_signature_help_tracks_active_parameter() {
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
    let text = "pub fn grant(amount: i64, bonus: i64) -> bool { return true } pub fn main() { grant(1, 2) }";
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
        "textDocument/signatureHelp",
        serde_json::json!({
            "textDocument": { "uri": "file:///workspace/scripts/game/main.vela" },
            "position": {
                "line": 0,
                "character": text.find("2)").unwrap_or_else(|| {
                    panic!("signature fixture should contain second argument")
                })
            }
        }),
    )));

    assert_eq!(response["result"]["activeSignature"], 0);
    assert_eq!(response["result"]["activeParameter"], 1);
    assert_eq!(
        response["result"]["signatures"][0]["label"],
        "grant(amount: i64, bonus: i64) -> bool"
    );
    assert_eq!(
        response["result"]["signatures"][0]["parameters"][1]["label"],
        "bonus: i64"
    );
}

#[test]
fn lsp_signature_help_resolves_imported_function_with_defaulted_parameter() {
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
    let main_text = "\
use game::rewards::reward_bonus
pub fn main(amount: i64) -> i64 {
    return reward_bonus(amount, 2)
}";
    let rewards_text = "\
pub fn reward_bonus(amount: i64, scale: i64 = 1) -> i64 {
    return amount * scale
}";
    let main_uri = "file:///workspace/scripts/game/main.vela";
    let rewards_uri = "file:///workspace/scripts/game/rewards.vela";
    for (uri, text) in [(rewards_uri, rewards_text), (main_uri, main_text)] {
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
    }

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/signatureHelp",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 2,
                "character": line(main_text, 2)
                    .find("2)")
                    .expect("signature fixture should contain second argument")
            }
        }),
    )));

    assert_eq!(response["result"]["activeSignature"], 0);
    assert_eq!(response["result"]["activeParameter"], 1);
    assert_eq!(
        response["result"]["signatures"][0]["label"],
        "reward_bonus(amount: i64, scale: i64 (defaulted)) -> i64"
    );
    assert_eq!(
        response["result"]["signatures"][0]["parameters"][1]["label"],
        "scale: i64 (defaulted)"
    );
}

#[test]
fn lsp_signature_help_returns_null_for_unknown_and_dynamic_calls() {
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
pub fn unresolved() { missing(1, 2) }
pub fn dynamic(player) { player.grant(1, 2) }";
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
        .expect("fixture should contain unresolved call");
    let unresolved = response_value(server.handle_json(&request(
        2,
        "textDocument/signatureHelp",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 0,
                "character": unresolved_line.find("2)").unwrap_or_else(|| {
                    panic!("signature fixture should contain unresolved second argument")
                })
            }
        }),
    )));
    assert!(unresolved["result"].is_null(), "{unresolved:?}");

    let dynamic_line = text
        .lines()
        .nth(1)
        .expect("fixture should contain dynamic receiver call");
    let dynamic = response_value(server.handle_json(&request(
        3,
        "textDocument/signatureHelp",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 1,
                "character": dynamic_line.find("2)").unwrap_or_else(|| {
                    panic!("signature fixture should contain dynamic second argument")
                })
            }
        }),
    )));
    assert!(dynamic["result"].is_null(), "{dynamic:?}");
}

#[test]
fn lsp_signature_help_resolves_script_method_call() {
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
struct Player { level: i64 }
impl Player {
    fn grant(self, amount: i64, bonus: i64) -> i64 { return amount + bonus }
}
pub fn main(player: Player) { player.grant(1, 2) }";
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

    let call_line = text
        .lines()
        .nth(4)
        .expect("fixture should contain method call");
    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/signatureHelp",
        serde_json::json!({
            "textDocument": { "uri": "file:///workspace/scripts/game/main.vela" },
            "position": {
                "line": 4,
                "character": call_line.find("2)").unwrap_or_else(|| {
                    panic!("signature fixture should contain second argument")
                })
            }
        }),
    )));

    assert_eq!(response["result"]["activeSignature"], 0);
    assert_eq!(response["result"]["activeParameter"], 1);
    assert_eq!(
        response["result"]["signatures"][0]["label"],
        "Player.grant(amount: i64, bonus: i64) -> i64"
    );
    assert_eq!(
        response["result"]["signatures"][0]["parameters"][1]["label"],
        "bonus: i64"
    );
}

#[test]
fn lsp_signature_help_resolves_source_trait_default_method_call() {
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
trait Rewardable {
    fn grant(self, amount: i64, bonus: i64) -> i64 { return amount + bonus }
}
struct Player { level: i64 }
impl Rewardable for Player {}
pub fn main(player: Player) { player.grant(1, 2) }";
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

    let call_line = text
        .lines()
        .nth(5)
        .expect("fixture should contain trait default method call");
    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/signatureHelp",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 5,
                "character": call_line.find("2)").unwrap_or_else(|| {
                    panic!("signature fixture should contain second argument")
                })
            }
        }),
    )));

    assert_eq!(response["result"]["activeSignature"], 0);
    assert_eq!(response["result"]["activeParameter"], 1);
    assert_eq!(
        response["result"]["signatures"][0]["label"],
        "game::main::Rewardable.grant(amount: i64, bonus: i64) -> i64"
    );
    assert_eq!(
        response["result"]["signatures"][0]["parameters"][1]["label"],
        "bonus: i64"
    );
}

#[test]
fn lsp_signature_help_resolves_schema_method_call() {
    let root = temp_workspace();
    let schema_path = root.join("target").join("vela").join("schema.json");
    fs::create_dir_all(schema_path.parent().expect("schema should have parent"))
        .expect("schema directory should be creatable");
    fs::write(&schema_path, schema_with_player_method())
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
    let text = "pub fn main(player: Player) { player.grant(1, 2) }";
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
        "textDocument/signatureHelp",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 0,
                "character": text.find("2)").unwrap_or_else(|| {
                    panic!("signature fixture should contain second argument")
                })
            }
        }),
    )));

    assert_eq!(response["result"]["activeSignature"], 0);
    assert_eq!(response["result"]["activeParameter"], 1);
    assert_eq!(
        response["result"]["signatures"][0]["label"],
        "Player.grant(arg0: i64, arg1: i64) -> bool"
    );
    assert_eq!(
        response["result"]["signatures"][0]["parameters"][1]["label"],
        "arg1: i64"
    );
    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_signature_help_resolves_schema_enum_variant_call() {
    let root = temp_workspace();
    let schema_path = root.join("target").join("vela").join("schema.json");
    fs::create_dir_all(schema_path.parent().expect("schema should have parent"))
        .expect("schema directory should be creatable");
    fs::write(&schema_path, schema_with_quest_state_tuple_variant())
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
    let text = r#"pub fn main() { QuestState::Active("quest-1", 3) }"#;
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
        "textDocument/signatureHelp",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 0,
                "character": text.find(", 3").expect("second argument") + 2
            }
        }),
    )));

    assert_eq!(response["result"]["activeSignature"], 0);
    assert_eq!(response["result"]["activeParameter"], 1);
    assert_eq!(
        response["result"]["signatures"][0]["label"],
        "QuestState::Active(arg0: String, arg1: i64) -> QuestState::Active"
    );
    assert_eq!(
        response["result"]["signatures"][0]["parameters"][1]["label"],
        "arg1: i64"
    );
}

#[test]
fn lsp_signature_help_resolves_schema_trait_method_call() {
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
    let text = "pub fn main(rewardable: Rewardable) { rewardable.preview(1, 2) }";
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
        "textDocument/signatureHelp",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 0,
                "character": text.find("2)").unwrap_or_else(|| {
                    panic!("signature fixture should contain second argument")
                })
            }
        }),
    )));

    assert_eq!(response["result"]["activeSignature"], 0);
    assert_eq!(response["result"]["activeParameter"], 1);
    assert_eq!(
        response["result"]["signatures"][0]["label"],
        "Rewardable.preview(arg0: i64, arg1: i64) -> bool"
    );
    assert_eq!(
        response["result"]["signatures"][0]["parameters"][1]["label"],
        "arg1: i64"
    );
    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_signature_help_resolves_stdlib_callback_method_call() {
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
pub fn main(scores: Array<i64>) {
    scores.filter(|score| score > 0)
}";
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

    let call_line = text
        .lines()
        .nth(1)
        .expect("fixture should contain filter call");
    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/signatureHelp",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 1,
                "character": call_line.find("score >").unwrap_or_else(|| {
                    panic!("signature fixture should contain lambda body")
                })
            }
        }),
    )));

    assert_eq!(response["result"]["activeSignature"], 0);
    assert_eq!(response["result"]["activeParameter"], 0);
    assert_eq!(
        response["result"]["signatures"][0]["label"],
        "Array(i64).filter(callback: Function(i64) -> bool) -> Array(i64)"
    );
    assert_eq!(
        response["result"]["signatures"][0]["parameters"][0]["label"],
        "callback: Function(i64) -> bool"
    );
}

#[test]
fn lsp_signature_help_resolves_stdlib_function_call() {
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
    let text = "pub fn main() { math::max(1, 2) }";
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
        "textDocument/signatureHelp",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 0,
                "character": text.find("2)").unwrap_or_else(|| {
                    panic!("signature fixture should contain second argument")
                })
            }
        }),
    )));

    assert_eq!(response["result"]["activeSignature"], 0);
    assert_eq!(response["result"]["activeParameter"], 1);
    assert_eq!(
        response["result"]["signatures"][0]["label"],
        "math::max(arg0: i64 | f64, arg1: i64 | f64) -> i64 | f64"
    );
    assert_eq!(
        response["result"]["signatures"][0]["parameters"][1]["label"],
        "arg1: i64 | f64"
    );
}

fn temp_workspace() -> PathBuf {
    let suffix = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(error) => panic!("system time should be after UNIX_EPOCH: {error}"),
    };
    let root = std::env::temp_dir().join(format!(
        "vela_lsp_signature_{}_{}",
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

fn schema_with_player_method() -> &'static str {
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
                        "params": [
                            { "kind": "primitive", "name": "i64" },
                            { "kind": "primitive", "name": "i64" }
                        ],
                        "returns": { "kind": "primitive", "name": "bool" }
                    }
                }
            ]
        }
    }"#
}

fn schema_with_quest_state_tuple_variant() -> &'static str {
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
                    "fact": { "kind": "enum", "name": "QuestState", "variant": "Active" }
                }
            ],
            "fields": [
                {
                    "owner": "QuestState::Active",
                    "name": "0",
                    "fact": { "kind": "primitive", "name": "string" }
                },
                {
                    "owner": "QuestState::Active",
                    "name": "1",
                    "fact": { "kind": "primitive", "name": "i64" }
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
                    "fact": { "kind": "trait", "name": "Rewardable" }
                }
            ],
            "traitMethods": [
                {
                    "owner": "Rewardable",
                    "name": "preview",
                    "fact": {
                        "kind": "function",
                        "params": [
                            { "kind": "primitive", "name": "i64" },
                            { "kind": "primitive", "name": "i64" }
                        ],
                        "returns": { "kind": "primitive", "name": "bool" }
                    }
                }
            ]
        }
    }"#
}

fn line(text: &str, line: usize) -> &str {
    text.lines().nth(line).expect("line should exist")
}
