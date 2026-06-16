use super::{LspServer, notification, notification_value, request, response_value};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn lsp_source_backed_schema_function_rename_updates_call_sites() {
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

    let schema_text = "pub fn grant() { return 1 }";
    let target_start = schema_text.find("grant").expect("schema marker");
    fs::write(
        &schema_path,
        serde_json::json!({
            "formatVersion": 1,
            "facts": {
                "functions": [
                    {
                        "name": "game::reward::grant",
                        "fact": {
                            "kind": "function",
                            "params": [{ "kind": "primitive", "name": "i64" }],
                            "returns": { "kind": "primitive", "name": "i64" }
                        },
                        "sourceSpan": {
                            "source": 1,
                            "start": target_start,
                            "end": target_start + "grant".len()
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
pub fn main(amount: i64) -> i64 {
    let first = grant(amount)
    return game::reward::grant(first)
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
                "line": 1,
                "character": line(text, 1).find("grant").expect("short call")
            }
        }),
    )));
    assert_eq!(prepare["result"]["placeholder"], "game::reward::grant");
    assert_eq!(prepare["result"]["range"]["start"]["line"], 1);
    assert_eq!(prepare["result"]["range"]["start"]["character"], 16);

    let rename = response_value(server.handle_json(&request(
        3,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 1,
                "character": line(text, 1).find("grant").expect("short call")
            },
            "newName": "award"
        }),
    )));
    let main_edits = rename["result"]["changes"][uri.as_str()]
        .as_array()
        .expect("function rename should return main edits");
    let schema_edits = rename["result"]["changes"][schema_uri.as_str()]
        .as_array()
        .expect("function rename should return schema edits");

    assert_eq!(main_edits.len(), 2);
    assert_text_edit(main_edits, 1, 16, "award");
    assert_text_edit(main_edits, 2, 25, "award");
    assert_eq!(schema_edits.len(), 1);
    assert_text_edit(schema_edits, 0, 7, "award");

    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_source_backed_schema_variant_rename_updates_constructors_and_patterns() {
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

    let schema_text = "pub fn Active() { return 1 }\npub fn Done() { return 2 }";
    let target_start = schema_text.find("Active").expect("schema marker");
    let done_start = schema_text.find("Done").expect("schema Done marker");
    fs::write(
        &schema_path,
        serde_json::json!({
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
                        "fact": {
                            "kind": "enum",
                            "name": "QuestState",
                            "variant": "Active"
                        },
                        "sourceSpan": {
                            "source": 1,
                            "start": target_start,
                            "end": target_start + "Active".len()
                        }
                    },
                    {
                        "owner": "QuestState",
                        "name": "Done",
                        "fact": {
                            "kind": "enum",
                            "name": "QuestState",
                            "variant": "Done"
                        },
                        "sourceSpan": {
                            "source": 1,
                            "start": done_start,
                            "end": done_start + "Done".len()
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
pub fn main(state: QuestState) -> i64 {
    let next = QuestState::Active
    match state {
        QuestState::Active => { return 1 }
        QuestState::Done => { return 2 }
    }
    return 0
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
                "line": 1,
                "character": line(text, 1).find("Active").expect("constructor")
            }
        }),
    )));
    assert_eq!(prepare["result"]["placeholder"], "Active");
    assert_eq!(prepare["result"]["range"]["start"]["line"], 1);
    assert_eq!(prepare["result"]["range"]["start"]["character"], 27);

    let rename = response_value(server.handle_json(&request(
        3,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 1,
                "character": line(text, 1).find("Active").expect("constructor")
            },
            "newName": "Running"
        }),
    )));
    let main_edits = rename["result"]["changes"][uri.as_str()]
        .as_array()
        .expect("variant rename should return main edits");
    let schema_edits = rename["result"]["changes"][schema_uri.as_str()]
        .as_array()
        .expect("variant rename should return schema edits");

    assert_eq!(main_edits.len(), 2);
    assert_text_edit(main_edits, 1, 27, "Running");
    assert_text_edit(main_edits, 3, 20, "Running");
    assert_eq!(schema_edits.len(), 1);
    assert_text_edit(schema_edits, 0, 7, "Running");

    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
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
    let root = std::env::temp_dir().join(format!(
        "vela_lsp_rename_schema_{}_{}",
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
