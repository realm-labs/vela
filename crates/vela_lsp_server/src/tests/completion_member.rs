use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::{LspServer, notification, notification_value, request, response_value};

#[test]
fn lsp_member_completion_uses_host_schema_facts() {
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
                            "fact": { "kind": "primitive", "name": "i64" },
                            "docs": "Current player level."
                        }
                    ],
                    "methods": [
                        {
                            "owner": "Player",
                            "name": "level_up",
                            "fact": {
                                "kind": "function",
                                "params": [{ "kind": "primitive", "name": "i64" }],
                                "returns": { "kind": "primitive", "name": "bool" }
                            },
                            "docs": "Increase the player level."
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
    let main_uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    let text = "pub fn main(player: Player) { player.le }";
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
        "textDocument/completion",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 0,
                "character": text.find("le }").expect("member prefix should exist") + 2
            }
        }),
    )));

    assert_completion(&response, "level", 5, "i64");
    assert_completion(&response, "level_up", 2, "Function(i64) -> bool");
    assert_no_completion_documentation(&response, "level");
    assert_no_completion_documentation(&response, "level_up");
    let level = completion_item(&response, "level");
    let level_up = completion_item(&response, "level_up");
    let resolved_level = resolve_completion(&mut server, 3, level);
    let resolved_level_up = resolve_completion(&mut server, 4, level_up);
    assert_completion_documentation(&resolved_level, "Current player level.");
    assert_completion_documentation(&resolved_level_up, "Increase the player level.");
    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_member_completion_uses_schema_trait_method_facts() {
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
    let main_uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    let text = "pub fn main(rewardable: Rewardable) { rewardable.pr }";
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
        "textDocument/completion",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 0,
                "character": text.find("pr }").expect("member prefix should exist") + 2
            }
        }),
    )));

    assert_completion(&response, "preview", 2, "Function(i64) -> bool");
    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_member_completion_uses_schema_function_return_receiver_facts() {
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
                    "functions": [
                        {
                            "name": "current_player",
                            "fact": {
                                "kind": "function",
                                "params": [],
                                "returns": { "kind": "host", "name": "Player" }
                            }
                        },
                        {
                            "name": "global_grant",
                            "fact": {
                                "kind": "function",
                                "params": [],
                                "returns": { "kind": "primitive", "name": "bool" }
                            }
                        }
                    ],
                    "fields": [
                        {
                            "owner": "Player",
                            "name": "level",
                            "fact": { "kind": "primitive", "name": "i64" }
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
    let main_uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    let text = "pub fn main() { current_player(). }";
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
        "textDocument/completion",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 0,
                "character": text.find(". }").expect("member dot") + 1
            }
        }),
    )));

    assert_completion(&response, "level", 5, "i64");
    assert_completion(&response, "grant", 2, "Function(i64) -> bool");
    assert_no_completion(&response, "current_player");
    assert_no_completion(&response, "global_grant");
    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_member_completion_uses_schema_method_return_receiver_facts() {
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
                        },
                        {
                            "name": "Inventory",
                            "fact": { "kind": "host", "name": "Inventory" }
                        }
                    ],
                    "functions": [
                        {
                            "name": "global_grant",
                            "fact": {
                                "kind": "function",
                                "params": [],
                                "returns": { "kind": "primitive", "name": "bool" }
                            }
                        }
                    ],
                    "fields": [
                        {
                            "owner": "Inventory",
                            "name": "count",
                            "fact": { "kind": "primitive", "name": "i64" }
                        }
                    ],
                    "methods": [
                        {
                            "owner": "Player",
                            "name": "inventory",
                            "fact": {
                                "kind": "function",
                                "params": [],
                                "returns": { "kind": "host", "name": "Inventory" }
                            }
                        },
                        {
                            "owner": "Inventory",
                            "name": "grant",
                            "fact": {
                                "kind": "function",
                                "params": [{ "kind": "primitive", "name": "i64" }],
                                "returns": { "kind": "primitive", "name": "bool" }
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
    let main_uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    let text = "pub fn main(player: Player) { player.inventory(). }";
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
        "textDocument/completion",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 0,
                "character": text.find(". }").expect("member dot") + 1
            }
        }),
    )));

    assert_completion(&response, "count", 5, "i64");
    assert_completion(&response, "grant", 2, "Function(i64) -> bool");
    assert_no_completion(&response, "inventory");
    assert_no_completion(&response, "global_grant");
    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_member_completion_uses_schema_trait_method_return_receiver_facts() {
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
                    "traits": [
                        {
                            "name": "Rewardable",
                            "fact": { "kind": "trait", "name": "Rewardable" }
                        }
                    ],
                    "functions": [
                        {
                            "name": "global_preview",
                            "fact": {
                                "kind": "function",
                                "params": [],
                                "returns": { "kind": "primitive", "name": "bool" }
                            }
                        }
                    ],
                    "methods": [
                        {
                            "owner": "Player",
                            "name": "rewardable",
                            "fact": {
                                "kind": "function",
                                "params": [],
                                "returns": { "kind": "trait", "name": "Rewardable" }
                            }
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
    let main_uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    let text = "pub fn main(player: Player) { player.rewardable(). }";
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
        "textDocument/completion",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 0,
                "character": text.find(". }").expect("member dot") + 1
            }
        }),
    )));

    assert_completion(&response, "preview", 2, "Function(i64) -> bool");
    assert_no_completion(&response, "rewardable");
    assert_no_completion(&response, "global_preview");
    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_member_completion_uses_source_function_return_receiver_facts() {
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
    let text = r#"
struct Player { level: i64 }
impl Player {
    fn grant(self, amount: i64) -> bool { return amount > 0 }
}
fn current_player() -> Player { return Player { level: 1 } }
fn global_grant() -> bool { return true }
pub fn main() { current_player(). }"#;
    let main_line = text.lines().nth(7).expect("main line");
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
        "textDocument/completion",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 7,
                "character": main_line.find(". }").expect("dot") + 1
            }
        }),
    )));

    assert_completion(&response, "level", 5, "i64");
    assert_completion(&response, "grant", 2, "Function(i64) -> bool");
    assert_no_completion(&response, "current_player");
    assert_no_completion(&response, "global_grant");
}

#[test]
fn lsp_member_completion_uses_source_method_return_receiver_facts() {
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
    let text = r#"
struct Player { level: i64 }
struct Inventory { count: i64 }
impl Player {
    fn inventory(self) -> Inventory { return Inventory { count: 1 } }
}
impl Inventory {
    fn grant(self, amount: i64) -> bool { return amount > 0 }
}
fn global_grant() -> bool { return true }
pub fn main(player: Player) { player.inventory(). }"#;
    let main_line = text.lines().nth(10).expect("main line");
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
        "textDocument/completion",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 10,
                "character": main_line.find(". }").expect("dot") + 1
            }
        }),
    )));

    assert_completion(&response, "count", 5, "i64");
    assert_completion(&response, "grant", 2, "Function(i64) -> bool");
    assert_no_completion(&response, "inventory");
    assert_no_completion(&response, "global_grant");
}

#[test]
fn lsp_member_completion_triggers_after_empty_dot_for_builtin_methods() {
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
    let text = "pub fn main(scores: Array<String>) { scores. }";
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
        "textDocument/completion",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 0,
                "character": text.find(". }").expect("dot") + 1
            }
        }),
    )));

    assert_completion(&response, "first", 2, "Function() -> Option(String)");
    assert_completion(&response, "join", 2, "Function(String) -> String");
    assert_completion(
        &response,
        "map",
        2,
        "Function(Function(String) -> Any) -> Array(Any)",
    );
    assert_no_completion(&response, "Array");
}

#[test]
fn lsp_member_completion_includes_source_and_builtin_methods() {
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
    let text = r#"
struct Player { level: i64 }
trait Rewardable {
    fn preview(self, amount: i64) -> bool { return amount > 0 }
    fn grant(self, amount: i64) -> bool { return amount > 0 }
}
impl Player {
    fn level_up(self, amount: i64) -> bool { return amount > 0 }
}
impl Rewardable for Player {
    fn grant(self, amount: i64) -> bool { return amount > 0 }
}
pub fn main(player: Player) {
    player.
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

    let player_response = response_value(server.handle_json(&request(
        2,
        "textDocument/completion",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": position_for(text, "player.")
        }),
    )));

    assert_completion(&player_response, "level", 5, "i64");
    assert_completion(&player_response, "level_up", 2, "Function(i64) -> bool");
    assert_completion(&player_response, "grant", 2, "Function(i64) -> bool");
    assert_completion(&player_response, "preview", 2, "Function(i64) -> bool");
    assert_no_completion(&player_response, "Rewardable");

    let builtin_uri = "file:///workspace/scripts/game/arrays.vela";
    let builtin_text = "pub fn main(scores: Array<String>) { scores. }";
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": builtin_uri,
                "languageId": "vela",
                "version": 1,
                "text": builtin_text
            }
        }),
    )));

    let builtin_response = response_value(server.handle_json(&request(
        3,
        "textDocument/completion",
        serde_json::json!({
            "textDocument": { "uri": builtin_uri },
            "position": position_for(builtin_text, "scores.")
        }),
    )));

    assert_completion(
        &builtin_response,
        "first",
        2,
        "Function() -> Option(String)",
    );
    assert_completion(&builtin_response, "join", 2, "Function(String) -> String");
    assert_completion(
        &builtin_response,
        "map",
        2,
        "Function(Function(String) -> Any) -> Array(Any)",
    );
    assert_no_completion(&builtin_response, "Array");
}

fn assert_completion(response: &serde_json::Value, label: &str, kind: u8, detail: &str) {
    assert_eq!(response["result"]["isIncomplete"], false);
    let Some(items) = response["result"]["items"].as_array() else {
        panic!("completion response should contain items");
    };
    assert!(
        items.iter().any(|item| {
            item["label"] == label && item["kind"] == kind && item["detail"] == detail
        }),
        "{items:?}"
    );
}

fn position_for(text: &str, needle: &str) -> serde_json::Value {
    let offset = text.find(needle).expect("needle should exist") + needle.len();
    let mut line = 0usize;
    let mut character = 0usize;
    for ch in text[..offset].chars() {
        if ch == '\n' {
            line += 1;
            character = 0;
        } else {
            character += ch.len_utf8();
        }
    }
    serde_json::json!({ "line": line, "character": character })
}

fn resolve_completion(
    server: &mut LspServer,
    id: i64,
    item: &serde_json::Value,
) -> serde_json::Value {
    response_value(server.handle_json(&request(id, "completionItem/resolve", item.clone())))
}

fn assert_completion_documentation(response: &serde_json::Value, expected: &str) {
    assert_eq!(response["result"]["documentation"]["kind"], "markdown");
    assert_eq!(response["result"]["documentation"]["value"], expected);
}

fn assert_no_completion_documentation(response: &serde_json::Value, label: &str) {
    let item = completion_item(response, label);
    assert!(item.get("documentation").is_none(), "{item:?}");
}

fn assert_no_completion(response: &serde_json::Value, label: &str) {
    assert_eq!(response["result"]["isIncomplete"], false);
    let Some(items) = response["result"]["items"].as_array() else {
        panic!("completion response should contain items");
    };
    assert!(items.iter().all(|item| item["label"] != label), "{items:?}");
}

fn completion_item<'a>(response: &'a serde_json::Value, label: &str) -> &'a serde_json::Value {
    assert_eq!(response["result"]["isIncomplete"], false);
    let Some(items) = response["result"]["items"].as_array() else {
        panic!("completion response should contain items");
    };
    items
        .iter()
        .find(|item| item["label"] == label)
        .unwrap_or_else(|| panic!("completion {label} should exist in {items:?}"))
}

fn temp_workspace() -> PathBuf {
    let suffix = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(error) => panic!("system time should be after UNIX_EPOCH: {error}"),
    };
    let root = std::env::temp_dir().join(format!(
        "vela_lsp_server_member_{}_{}",
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
