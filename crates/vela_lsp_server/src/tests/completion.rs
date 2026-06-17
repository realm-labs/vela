use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::{LspServer, notification, notification_value, request, response_value};

#[test]
fn lsp_completion_uses_open_overlay_declarations() {
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
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": "file:///workspace/scripts/game/main.vela",
                "languageId": "vela",
                "version": 1,
                "text": "pub fn overlay_only() { return 2 }"
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/completion",
        serde_json::json!({
            "textDocument": { "uri": "file:///workspace/scripts/game/main.vela" },
            "position": { "line": 0, "character": 7 }
        }),
    )));

    assert_completion(&response, "overlay_only", 3, "Function() -> unknown");
}

#[test]
fn lsp_module_path_completion_snippets_stdlib_functions() {
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
    let text = "pub fn main() { math:: }";
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
                "character": text.find(" }").expect("completion point")
            }
        }),
    )));

    assert_completion_snippet(
        &response,
        "max",
        3,
        "Function(i64 | f64, i64 | f64) -> i64 | f64",
        "max($0)",
    );
    assert_no_completion(&response, "math::max");
}

#[test]
fn lsp_completion_uses_loaded_schema_facts() {
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
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": main_uri,
                "languageId": "vela",
                "version": 1,
                "text": "pub fn main() { Pla }"
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/completion",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": { "line": 0, "character": 18 }
        }),
    )));

    assert_completion(&response, "Player", 22, "Player");
    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_item_boundary_completion_projects_keyword_items() {
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
    let text = "f";
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
            "position": { "line": 0, "character": text.len() }
        }),
    )));

    assert_completion_insert_text(&response, "fn", 14, "function declaration", "fn ");
    assert_completion_projection(
        &response,
        "fn",
        serde_json::json!({
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 0, "character": 1 }
            },
            "newText": "fn "
        }),
        "fn",
        "0000_00_fn",
        serde_json::json!({ "detail": "function declaration" }),
        true,
    );
}

#[test]
fn lsp_statement_completion_projects_statement_keywords() {
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
    let text = "pub fn helper() { return 1 }\npub fn main() { return 1 }";
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
    let main_line = text.lines().nth(1).expect("main line should exist");

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/completion",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 1,
                "character": main_line.find("return").expect("statement start should exist")
            }
        }),
    )));

    assert_completion_insert_text(&response, "let", 14, "local binding", "let ");
    assert_completion_insert_text(&response, "return", 14, "return statement", "return ");
    assert_completion(&response, "helper", 3, "Function() -> unknown");
    assert_no_completion(&response, "fn");
}

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
                            "fact": { "kind": "primitive", "name": "i64" }
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
fn lsp_record_field_completion_uses_known_constructor() {
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
    let text = "pub struct Player { id: String level: i64 }\npub fn main() { let player = Player { id: \"p1\", le } }";
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
                    "line": 1,
                    "character": text.lines().nth(1).expect("second line").find("le }").expect("record prefix") + 2
                }
            }),
        )));

    assert_completion(&response, "level", 5, "i64");
    assert_no_completion(&response, "id");
}

#[test]
fn lsp_named_argument_completion_suggests_unused_script_parameters() {
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
pub fn grant(player: Player, amount: i64, reason: String = "quest") -> bool { return true }
pub fn main(player: Player) { grant(player: player, ) }
"#;
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
    let main_line = text.lines().nth(2).expect("main line should exist");

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/completion",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 2,
                "character": main_line
                    .find(", )")
                    .expect("call should contain empty argument") + ", ".len()
            }
        }),
    )));

    assert_completion_insert_text(&response, "amount", 6, "i64", "amount: ");
    assert_completion_insert_text(&response, "reason", 6, "String (defaulted)", "reason: ");
    assert_no_completion(&response, "player");
}

#[test]
fn lsp_lambda_parameter_completion_uses_pipe_trigger_context() {
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
    let text = "pub fn main(scores: Array<i64>) { scores.filter(|) }";
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
                "character": text.find("|)").expect("lambda pipe should exist") + "|".len()
            },
            "context": {
                "triggerKind": 2,
                "triggerCharacter": "|"
            }
        }),
    )));

    assert_completion(&response, "item", 6, "i64");
}

#[test]
fn lsp_type_hint_completion_uses_colon_trigger_context() {
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
    let text = "pub struct Player { level: i64 }\npub fn helper() { return 1 }\npub fn main(player: Pl) { return 1 }";
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
    let main_line = text.lines().nth(2).expect("main line should exist");

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/completion",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 2,
                "character": main_line.find("Pl)").expect("type prefix") + "Pl".len()
            },
            "context": {
                "triggerKind": 2,
                "triggerCharacter": ":"
            }
        }),
    )));

    assert_completion(&response, "game::main::Player", 22, "game::main::Player");
    assert_no_completion(&response, "game::main::helper");
}

#[test]
fn lsp_pattern_completion_projects_enum_variants() {
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
pub enum QuestState {
    Started
    Completed
}
pub fn helper() { return 1 }
pub fn main(state: QuestState) {
    match state {
        Co
    }
}
"#;
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
    let pattern_line = text.lines().nth(8).expect("pattern line should exist");

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/completion",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 8,
                "character": pattern_line.find("Co").expect("pattern prefix") + "Co".len()
            }
        }),
    )));

    assert_completion(&response, "Completed", 20, "QuestState");
    assert_no_completion(&response, "helper");
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

fn assert_completion_insert_text(
    response: &serde_json::Value,
    label: &str,
    kind: u8,
    detail: &str,
    insert_text: &str,
) {
    assert_eq!(response["result"]["isIncomplete"], false);
    let Some(items) = response["result"]["items"].as_array() else {
        panic!("completion response should contain items");
    };
    assert!(
        items.iter().any(|item| {
            item["label"] == label
                && item["kind"] == kind
                && item["detail"] == detail
                && item["insertText"] == insert_text
        }),
        "{items:?}"
    );
}

fn assert_completion_snippet(
    response: &serde_json::Value,
    label: &str,
    kind: u8,
    detail: &str,
    insert_text: &str,
) {
    assert_eq!(response["result"]["isIncomplete"], false);
    let Some(items) = response["result"]["items"].as_array() else {
        panic!("completion response should contain items");
    };
    assert!(
        items.iter().any(|item| {
            item["label"] == label
                && item["kind"] == kind
                && item["detail"] == detail
                && item["insertText"] == insert_text
                && item["insertTextFormat"] == 2
        }),
        "{items:?}"
    );
}

fn assert_completion_projection(
    response: &serde_json::Value,
    label: &str,
    text_edit: serde_json::Value,
    filter_text: &str,
    sort_text: &str,
    label_details: serde_json::Value,
    preselect: bool,
) {
    let item = completion_item(response, label);
    assert_eq!(item["textEdit"], text_edit);
    assert_eq!(item["filterText"], filter_text);
    assert_eq!(item["sortText"], sort_text);
    assert_eq!(item["labelDetails"], label_details);
    assert_eq!(item["preselect"], preselect);
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
    let root =
        std::env::temp_dir().join(format!("vela_lsp_server_{}_{}", std::process::id(), suffix));
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
