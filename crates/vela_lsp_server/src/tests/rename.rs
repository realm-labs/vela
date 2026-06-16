use super::{LspServer, notification, notification_value, request, response_value};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn lsp_prepare_rename_rejects_keywords_and_literals() {
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
    return amount + 1
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

    let keyword_response = response_value(server.handle_json(&request(
        2,
        "textDocument/prepareRename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 1,
                "character": line(text, 1).find("return").expect("return keyword")
            }
        }),
    )));
    assert_eq!(keyword_response["result"], serde_json::Value::Null);

    let literal_response = response_value(server.handle_json(&request(
        3,
        "textDocument/prepareRename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 1,
                "character": line(text, 1).find('1').expect("literal")
            }
        }),
    )));
    assert_eq!(literal_response["result"], serde_json::Value::Null);
}

#[test]
fn lsp_local_rename_updates_all_function_uses() {
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
    let next = amount + 1
    next += amount
    return next
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

    let prepare = response_value(server.handle_json(&request(
        2,
        "textDocument/prepareRename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 2,
                "character": line(text, 2).find("next").expect("next write")
            }
        }),
    )));
    assert_eq!(prepare["result"]["placeholder"], "next");
    assert_eq!(prepare["result"]["range"]["start"]["line"], 2);
    assert_eq!(prepare["result"]["range"]["start"]["character"], 4);

    let rename = response_value(server.handle_json(&request(
        3,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 2,
                "character": line(text, 2).find("next").expect("next write")
            },
            "newName": "score"
        }),
    )));
    let edits = rename["result"]["changes"][uri]
        .as_array()
        .expect("rename should return text edits for the document");

    assert_eq!(edits.len(), 3);
    assert_text_edit(edits, 1, 8, "score");
    assert_text_edit(edits, 2, 4, "score");
    assert_text_edit(edits, 3, 11, "score");
}

#[test]
fn lsp_rename_returns_versioned_document_changes() {
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
    let next = amount + 1
    return next
}";
    let uri = "file:///workspace/scripts/game/main.vela";
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 2,
                "text": text
            }
        }),
    )));

    let rename = response_value(server.handle_json(&request(
        2,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 2,
                "character": line(text, 2).find("next").expect("next use")
            },
            "newName": "score"
        }),
    )));
    let document_changes = rename["result"]["documentChanges"]
        .as_array()
        .expect("rename should return versioned document changes");
    assert_eq!(document_changes.len(), 1);
    assert_eq!(document_changes[0]["textDocument"]["uri"], uri);
    assert_eq!(document_changes[0]["textDocument"]["version"], 2);
    let edits = document_changes[0]["edits"]
        .as_array()
        .expect("document changes should contain text edits");
    assert_eq!(edits.len(), 2);
    assert_text_edit(edits, 1, 8, "score");
    assert_text_edit(edits, 2, 11, "score");
}

#[test]
fn lsp_private_function_rename_updates_imports() {
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
use game::reward::grant
pub fn main(amount: i64) -> i64 {
    return grant(amount)
}";
    let helper_text = "pub fn grant(amount: i64) -> i64 { return amount }";
    let main_uri = "file:///workspace/scripts/game/main.vela";
    let helper_uri = "file:///workspace/scripts/game/reward.vela";
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": helper_uri,
                "languageId": "vela",
                "version": 1,
                "text": helper_text
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

    let prepare = response_value(server.handle_json(&request(
        2,
        "textDocument/prepareRename",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 2,
                "character": line(main_text, 2).find("grant").expect("grant call")
            }
        }),
    )));
    assert_eq!(prepare["result"]["placeholder"], "grant");
    assert_eq!(prepare["result"]["range"]["start"]["line"], 2);
    assert_eq!(prepare["result"]["range"]["start"]["character"], 11);

    let rename = response_value(server.handle_json(&request(
        3,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 2,
                "character": line(main_text, 2).find("grant").expect("grant call")
            },
            "newName": "award"
        }),
    )));
    let main_edits = rename["result"]["changes"][main_uri]
        .as_array()
        .expect("rename should return main edits");
    let helper_edits = rename["result"]["changes"][helper_uri]
        .as_array()
        .expect("rename should return helper edits");

    assert_eq!(main_edits.len(), 2);
    assert_text_edit(main_edits, 0, 18, "award");
    assert_text_edit(main_edits, 2, 11, "award");
    assert_eq!(helper_edits.len(), 1);
    assert_text_edit(helper_edits, 0, 7, "award");
}

#[test]
fn lsp_private_value_declaration_rename_updates_uses() {
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
const BONUS: i64 = 5
pub fn main() -> i64 {
    return BONUS + BONUS
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

    let prepare = response_value(server.handle_json(&request(
        2,
        "textDocument/prepareRename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 2,
                "character": line(text, 2).find("BONUS").expect("BONUS read")
            }
        }),
    )));
    assert_eq!(prepare["result"]["placeholder"], "BONUS");
    assert_eq!(prepare["result"]["range"]["start"]["line"], 2);
    assert_eq!(prepare["result"]["range"]["start"]["character"], 11);

    let rename = response_value(server.handle_json(&request(
        3,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 2,
                "character": line(text, 2).find("BONUS").expect("BONUS read")
            },
            "newName": "BASE"
        }),
    )));
    let edits = rename["result"]["changes"][uri]
        .as_array()
        .expect("rename should return text edits for the document");

    assert_eq!(edits.len(), 3);
    assert_text_edit(edits, 0, 6, "BASE");
    assert_text_edit(edits, 2, 11, "BASE");
    assert_text_edit(edits, 2, 19, "BASE");
}

#[test]
fn lsp_private_type_declaration_rename_updates_type_hints() {
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
struct Reward {
    amount: i64
}

fn grant(reward: Reward) -> Reward {
    let next: Reward = reward
    return next
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

    let prepare = response_value(server.handle_json(&request(
        2,
        "textDocument/prepareRename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 4,
                "character": line(text, 4).rfind("Reward").expect("return type")
            }
        }),
    )));
    assert_eq!(prepare["result"]["placeholder"], "Reward");
    assert_eq!(prepare["result"]["range"]["start"]["line"], 4);
    assert_eq!(prepare["result"]["range"]["start"]["character"], 28);

    let rename = response_value(server.handle_json(&request(
        3,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 4,
                "character": line(text, 4).rfind("Reward").expect("return type")
            },
            "newName": "Prize"
        }),
    )));
    let edits = rename["result"]["changes"][uri]
        .as_array()
        .expect("rename should return text edits for the document");

    assert_eq!(edits.len(), 4);
    assert_text_edit(edits, 0, 7, "Prize");
    assert_text_edit(edits, 4, 17, "Prize");
    assert_text_edit(edits, 4, 28, "Prize");
    assert_text_edit(edits, 5, 14, "Prize");
}

#[test]
fn lsp_private_struct_field_rename_updates_member_uses() {
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
struct Player {
    level: i64
    xp: i64
}

fn bump(player: Player) -> i64 {
    player.level += 1
    return player.level + player.xp
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

    let prepare = response_value(server.handle_json(&request(
        2,
        "textDocument/prepareRename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 6,
                "character": line(text, 6).find("level").expect("level write")
            }
        }),
    )));
    assert_eq!(prepare["result"]["placeholder"], "level");
    assert_eq!(prepare["result"]["range"]["start"]["line"], 6);
    assert_eq!(prepare["result"]["range"]["start"]["character"], 11);

    let rename = response_value(server.handle_json(&request(
        3,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 6,
                "character": line(text, 6).find("level").expect("level write")
            },
            "newName": "rank"
        }),
    )));
    let edits = rename["result"]["changes"][uri]
        .as_array()
        .expect("rename should return text edits for the document");

    assert_eq!(edits.len(), 3);
    assert_text_edit(edits, 1, 4, "rank");
    assert_text_edit(edits, 6, 11, "rank");
    assert_text_edit(edits, 7, 18, "rank");
}

#[test]
fn lsp_private_method_rename_updates_typed_receiver_calls() {
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
struct Reward {
    amount: i64
}

impl Reward {
    fn grant(self, amount: i64) -> i64 { return amount }
    fn preview(self) -> i64 { return self.grant(1) }
}

fn main(reward: Reward) -> i64 {
    let first = reward.grant(1)
    return reward.grant(first)
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

    let prepare = response_value(server.handle_json(&request(
        2,
        "textDocument/prepareRename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 10,
                "character": line(text, 10).find("grant").expect("grant call")
            }
        }),
    )));
    assert_eq!(prepare["result"]["placeholder"], "grant");
    assert_eq!(prepare["result"]["range"]["start"]["line"], 10);
    assert_eq!(prepare["result"]["range"]["start"]["character"], 23);

    let rename = response_value(server.handle_json(&request(
        3,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 10,
                "character": line(text, 10).find("grant").expect("grant call")
            },
            "newName": "award"
        }),
    )));
    let edits = rename["result"]["changes"][uri]
        .as_array()
        .expect("rename should return text edits for the document");

    assert_eq!(edits.len(), 4);
    assert_text_edit(edits, 5, 7, "award");
    assert_text_edit(edits, 6, 42, "award");
    assert_text_edit(edits, 10, 23, "award");
    assert_text_edit(edits, 11, 18, "award");
}

#[test]
fn lsp_private_enum_variant_rename_updates_constructors_and_patterns() {
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
enum QuestState {
    Active { count: i64 },
    Done
}

fn active(count: i64) -> QuestState {
    return QuestState::Active { count: count }
}

fn main(state: QuestState) -> i64 {
    match state {
        QuestState::Active { count } => { return count }
        QuestState::Done => { return 0 }
    }
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

    let prepare = response_value(server.handle_json(&request(
        2,
        "textDocument/prepareRename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 6,
                "character": line(text, 6).find("Active").expect("constructor")
            }
        }),
    )));
    assert_eq!(prepare["result"]["placeholder"], "Active");
    assert_eq!(prepare["result"]["range"]["start"]["line"], 6);
    assert_eq!(prepare["result"]["range"]["start"]["character"], 23);

    let rename = response_value(server.handle_json(&request(
        3,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 6,
                "character": line(text, 6).find("Active").expect("constructor")
            },
            "newName": "Running"
        }),
    )));
    let edits = rename["result"]["changes"][uri]
        .as_array()
        .expect("rename should return text edits for the document");

    assert_eq!(edits.len(), 3);
    assert_text_edit(edits, 1, 4, "Running");
    assert_text_edit(edits, 6, 23, "Running");
    assert_text_edit(edits, 11, 20, "Running");
}

#[test]
fn lsp_host_schema_rename_is_not_editable() {
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
    fs::write(&schema_path, schema_artifact()).expect("schema should be writable");

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
    let text = "pub fn main(player: Player) { return player.level }";
    let uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
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

    let hover = response_value(server.handle_json(&request(
        2,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 0,
                "character": text.find("level").expect("schema field")
            }
        }),
    )));
    let hover_value = hover["result"]["contents"]["value"]
        .as_str()
        .expect("schema hover should produce markdown");
    assert!(hover_value.contains("Player.level"), "{hover_value}");

    let type_prepare = response_value(server.handle_json(&request(
        3,
        "textDocument/prepareRename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 0,
                "character": text.find("Player").expect("schema type")
            }
        }),
    )));
    assert_eq!(type_prepare["result"], serde_json::Value::Null);

    let type_rename = response_value(server.handle_json(&request(
        4,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 0,
                "character": text.find("Player").expect("schema type")
            },
            "newName": "Actor"
        }),
    )));
    assert_eq!(type_rename["result"], serde_json::Value::Null);

    let field_prepare = response_value(server.handle_json(&request(
        5,
        "textDocument/prepareRename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 0,
                "character": text.find("level").expect("schema field")
            }
        }),
    )));
    assert_eq!(field_prepare["result"], serde_json::Value::Null);

    let field_rename = response_value(server.handle_json(&request(
        6,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 0,
                "character": text.find("level").expect("schema field")
            },
            "newName": "rank"
        }),
    )));
    assert_eq!(field_rename["result"], serde_json::Value::Null);
    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_source_backed_schema_rename_updates_member_uses() {
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

    let schema_text = "\
pub fn level() { return 1 }
pub fn grant() { return 1 }";
    let level_start = schema_text.find("level").expect("level marker");
    let grant_start = schema_text.find("grant").expect("grant marker");
    fs::write(
        &schema_path,
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
                        "name": "level",
                        "fact": { "kind": "primitive", "name": "i64" },
                        "sourceSpan": {
                            "source": 1,
                            "start": level_start,
                            "end": level_start + "level".len()
                        }
                    }
                ],
                "methods": [
                    {
                        "owner": "Player",
                        "name": "grant",
                        "fact": {
                            "kind": "function",
                            "params": [{ "kind": "primitive", "name": "i64" }],
                            "returns": { "kind": "primitive", "name": "i64" }
                        },
                        "sourceSpan": {
                            "source": 1,
                            "start": grant_start,
                            "end": grant_start + "grant".len()
                        }
                    }
                ]
            }
        })
        .to_string(),
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

    let schema_uri = file_uri(&root.join("scripts").join("_schema_defs.vela"));
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": schema_uri,
                "languageId": "vela",
                "version": 1,
                "text": schema_text
            }
        }),
    )));

    let text = "\
pub fn main(player: Player) -> i64 {
    let first = player.level
    player.level += 1
    return player.grant(first)
}";
    let uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
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

    let field_rename = response_value(server.handle_json(&request(
        2,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 1,
                "character": line(text, 1).find("level").expect("field read")
            },
            "newName": "rank"
        }),
    )));
    let field_main_edits = field_rename["result"]["changes"][uri.as_str()]
        .as_array()
        .expect("field rename should return main edits");
    let field_schema_edits = field_rename["result"]["changes"][schema_uri.as_str()]
        .as_array()
        .expect("field rename should return schema edits");

    assert_eq!(field_main_edits.len(), 2);
    assert_text_edit(field_main_edits, 1, 23, "rank");
    assert_text_edit(field_main_edits, 2, 11, "rank");
    assert_eq!(field_schema_edits.len(), 1);
    assert_text_edit(field_schema_edits, 0, 7, "rank");

    let method_rename = response_value(server.handle_json(&request(
        3,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 3,
                "character": line(text, 3).find("grant").expect("method call")
            },
            "newName": "award"
        }),
    )));
    let method_main_edits = method_rename["result"]["changes"][uri.as_str()]
        .as_array()
        .expect("method rename should return main edits");
    let method_schema_edits = method_rename["result"]["changes"][schema_uri.as_str()]
        .as_array()
        .expect("method rename should return schema edits");

    assert_eq!(method_main_edits.len(), 1);
    assert_text_edit(method_main_edits, 3, 18, "award");
    assert_eq!(method_schema_edits.len(), 1);
    assert_text_edit(method_schema_edits, 1, 7, "award");

    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_source_backed_schema_type_rename_updates_type_hints() {
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

    let schema_text = "pub fn Player() { return 1 }";
    let target_start = schema_text.find("Player").expect("schema marker");
    fs::write(
        &schema_path,
        serde_json::json!({
            "formatVersion": 1,
            "facts": {
                "types": [
                    {
                        "name": "Player",
                        "fact": { "kind": "host", "name": "Player" },
                        "sourceSpan": {
                            "source": 1,
                            "start": target_start,
                            "end": target_start + "Player".len()
                        }
                    }
                ]
            }
        })
        .to_string(),
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

    let schema_uri = file_uri(&root.join("scripts").join("_schema_defs.vela"));
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": schema_uri,
                "languageId": "vela",
                "version": 1,
                "text": schema_text
            }
        }),
    )));

    let text = "\
pub fn spawn(player: Player) -> Player {
    return player
}";
    let uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
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

    let prepare = response_value(server.handle_json(&request(
        2,
        "textDocument/prepareRename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 0,
                "character": line(text, 0).find("Player").expect("parameter type")
            }
        }),
    )));
    assert_eq!(prepare["result"]["placeholder"], "Player");
    assert_eq!(prepare["result"]["range"]["start"]["line"], 0);
    assert_eq!(prepare["result"]["range"]["start"]["character"], 21);

    let rename = response_value(server.handle_json(&request(
        3,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 0,
                "character": line(text, 0).find("Player").expect("parameter type")
            },
            "newName": "Actor"
        }),
    )));
    let main_edits = rename["result"]["changes"][uri.as_str()]
        .as_array()
        .expect("type rename should return main edits");
    let schema_edits = rename["result"]["changes"][schema_uri.as_str()]
        .as_array()
        .expect("type rename should return schema edits");

    assert_eq!(main_edits.len(), 2);
    assert_text_edit(main_edits, 0, 21, "Actor");
    assert_text_edit(main_edits, 0, 32, "Actor");
    assert_eq!(schema_edits.len(), 1);
    assert_text_edit(schema_edits, 0, 7, "Actor");

    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_public_export_rename_reports_hot_reload_risk() {
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
    let text = "pub fn grant(amount: i64) -> i64 { return amount }";
    let uri = "file:///workspace/scripts/game/reward.vela";
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

    let rename = response_value(server.handle_json(&request(
        2,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 0,
                "character": line(text, 0).find("grant").expect("grant declaration")
            },
            "newName": "award"
        }),
    )));
    let edits = rename["result"]["changes"][uri]
        .as_array()
        .expect("rename should return text edits");
    assert_eq!(edits.len(), 1);
    assert_text_edit(edits, 0, 7, "award");

    let annotations = rename["result"]["changeAnnotations"]
        .as_object()
        .expect("public export rename should include a change annotation");
    let risk = &annotations["renameRisk0"];
    assert_eq!(risk["needsConfirmation"], true);
    assert_eq!(risk["description"], "hotReloadAbi");
    assert!(
        risk["label"]
            .as_str()
            .expect("risk label should be a string")
            .contains("public function `grant`")
    );
}

fn assert_text_edit(edits: &[serde_json::Value], line: usize, character: usize, new_text: &str) {
    assert!(
        edits.iter().any(|edit| {
            edit["range"]["start"]["line"] == line
                && edit["range"]["start"]["character"] == character
                && edit["newText"] == new_text
        }),
        "{edits:?}"
    );
}

fn line(text: &str, line: usize) -> &str {
    text.lines().nth(line).expect("line should exist")
}

fn temp_workspace() -> PathBuf {
    let suffix = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(error) => panic!("system time should be after UNIX_EPOCH: {error}"),
    };
    let root =
        std::env::temp_dir().join(format!("vela_lsp_rename_{}_{}", std::process::id(), suffix));
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

fn schema_artifact() -> &'static str {
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
            ]
        }
    }"#
}
