use super::{LspServer, notification, notification_value, request, response_value};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn lsp_rename_rejects_module_declaration_collision() {
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
pub fn grant(amount: i64) -> i64 { return amount }
pub fn award(amount: i64) -> i64 { return amount + 1 }";
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
                "line": 0,
                "character": line(text, 0).find("grant").expect("grant declaration")
            }
        }),
    )));
    assert_eq!(prepare["result"]["placeholder"], "grant");

    let rename = response_value(server.handle_json(&request(
        3,
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
    assert_eq!(rename["result"], serde_json::Value::Null);
}

#[test]
fn lsp_rename_rejects_import_alias_collision() {
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
use game::bonus::score as award
pub fn main() -> i64 {
    return grant() + award()
}";
    let reward_text = "pub fn grant() -> i64 { return 1 }";
    let bonus_text = "pub fn score() -> i64 { return 2 }";
    let main_uri = "file:///workspace/scripts/game/main.vela";
    let reward_uri = "file:///workspace/scripts/game/reward.vela";
    let bonus_uri = "file:///workspace/scripts/game/bonus.vela";
    for (uri, text) in [
        (reward_uri, reward_text),
        (bonus_uri, bonus_text),
        (main_uri, main_text),
    ] {
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

    let rename = response_value(server.handle_json(&request(
        2,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 3,
                "character": line(main_text, 3).find("grant").expect("grant call")
            },
            "newName": "award"
        }),
    )));
    assert_eq!(rename["result"], serde_json::Value::Null);
}

#[test]
fn lsp_source_backed_schema_member_rename_rejects_same_kind_collisions() {
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
pub fn rank() { return 2 }
pub fn grant() { return 3 }
pub fn award() { return 4 }";
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
                    source_backed_member("Player", "level", schema_text),
                    {
                        "owner": "Player",
                        "name": "rank",
                        "fact": { "kind": "primitive", "name": "i64" }
                    }
                ],
                "methods": [
                    source_backed_method("Player", "grant", schema_text),
                    {
                        "owner": "Player",
                        "name": "award",
                        "fact": {
                            "kind": "function",
                            "params": [{ "kind": "primitive", "name": "i64" }],
                            "returns": { "kind": "primitive", "name": "i64" }
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
    assert_eq!(field_rename["result"], serde_json::Value::Null);

    let method_rename = response_value(server.handle_json(&request(
        3,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 2,
                "character": line(text, 2).find("grant").expect("method call")
            },
            "newName": "award"
        }),
    )));
    assert_eq!(method_rename["result"], serde_json::Value::Null);

    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

fn source_backed_member(owner: &str, name: &str, schema_text: &str) -> serde_json::Value {
    let start = schema_text.find(name).expect("schema member should exist");
    serde_json::json!({
        "owner": owner,
        "name": name,
        "fact": { "kind": "primitive", "name": "i64" },
        "sourceSpan": {
            "source": 1,
            "start": start,
            "end": start + name.len()
        }
    })
}

fn source_backed_method(owner: &str, name: &str, schema_text: &str) -> serde_json::Value {
    let start = schema_text.find(name).expect("schema method should exist");
    serde_json::json!({
        "owner": owner,
        "name": name,
        "fact": {
            "kind": "function",
            "params": [{ "kind": "primitive", "name": "i64" }],
            "returns": { "kind": "primitive", "name": "i64" }
        },
        "sourceSpan": {
            "source": 1,
            "start": start,
            "end": start + name.len()
        }
    })
}

fn line(text: &str, line: usize) -> &str {
    text.lines().nth(line).expect("line should exist")
}

fn temp_workspace() -> PathBuf {
    let suffix = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(error) => panic!("system time should be after UNIX_EPOCH: {error}"),
    };
    let root = std::env::temp_dir().join(format!(
        "vela_lsp_rename_collision_{}_{}",
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
