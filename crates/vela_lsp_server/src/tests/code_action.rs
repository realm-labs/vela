use super::{LspServer, notification, notification_value, request, response_value};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn lsp_code_action_fixes_unknown_field_typo() {
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
    let text = "pub fn main(scores: Array<i64>) { return scores.frist() }";
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
        "textDocument/codeAction",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 47 },
                "end": { "line": 0, "character": 52 }
            },
            "context": { "diagnostics": [] }
        }),
    )));
    let actions = response["result"]
        .as_array()
        .expect("codeAction should return an array");
    let action = actions
        .iter()
        .find(|action| action["title"] == "Replace with `first`")
        .expect("candidate quick fix should be returned");

    assert_eq!(action["kind"], "quickfix");
    let edit = &action["edit"]["changes"][uri][0];
    assert_eq!(edit["range"]["start"]["line"], 0);
    assert_eq!(
        edit["range"]["start"]["character"],
        text.find("frist").expect("member token")
    );
    assert_eq!(edit["range"]["end"]["line"], 0);
    assert_eq!(
        edit["range"]["end"]["character"],
        text.find("frist").expect("member token") + "frist".len()
    );
    assert_eq!(edit["newText"], "first");
}

#[test]
fn lsp_code_action_inserts_missing_import() {
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
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": reward_uri,
                "languageId": "vela",
                "version": 1,
                "text": "pub fn grant() { return 1 }"
            }
        }),
    )));
    let text = "pub fn main() { return grant }";
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

    let grant_start = text.find("grant").expect("unresolved symbol");
    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/codeAction",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": grant_start },
                "end": { "line": 0, "character": grant_start + "grant".len() }
            },
            "context": { "diagnostics": [] }
        }),
    )));
    let actions = response["result"]
        .as_array()
        .expect("codeAction should return an array");
    let action = actions
        .iter()
        .find(|action| action["title"] == "Import `game::reward::grant`")
        .expect("import quick fix should be returned");

    assert_eq!(action["kind"], "quickfix");
    let edit = &action["edit"]["changes"][uri][0];
    assert_eq!(edit["range"]["start"]["line"], 0);
    assert_eq!(edit["range"]["start"]["character"], 0);
    assert_eq!(edit["range"]["end"]["line"], 0);
    assert_eq!(edit["range"]["end"]["character"], 0);
    assert_eq!(edit["newText"], "use game::reward::grant\n");
}

#[test]
fn lsp_code_action_removes_unused_import() {
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
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": reward_uri,
                "languageId": "vela",
                "version": 1,
                "text": "pub fn grant() { return 1 }"
            }
        }),
    )));
    let text = "use game::reward::grant\npub fn main() { return 1 }";
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
        "textDocument/codeAction",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 18 },
                "end": { "line": 0, "character": 23 }
            },
            "context": { "diagnostics": [] }
        }),
    )));
    let actions = response["result"]
        .as_array()
        .expect("codeAction should return an array");
    let action = actions
        .iter()
        .find(|action| action["title"] == "Remove unused import")
        .expect("unused import quick fix should be returned");

    assert_eq!(action["kind"], "quickfix");
    let edit = &action["edit"]["changes"][uri][0];
    assert_eq!(edit["range"]["start"]["line"], 0);
    assert_eq!(edit["range"]["start"]["character"], 0);
    assert_eq!(edit["range"]["end"]["line"], 1);
    assert_eq!(edit["range"]["end"]["character"], 0);
    assert_eq!(edit["newText"], "");
}

#[test]
fn lsp_code_action_fills_enum_match_arms() {
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
pub fn main(maybe_name: Option<String>) {
    match maybe_name {
        Option::Some(name) => name,
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

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/codeAction",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 1, "character": 4 },
                "end": { "line": 3, "character": 5 }
            },
            "context": { "diagnostics": [] }
        }),
    )));
    let actions = response["result"]
        .as_array()
        .expect("codeAction should return an array");
    let action = actions
        .iter()
        .find(|action| action["title"] == "Add missing match arms for `Option`")
        .expect("match-arm quick fix should be returned");

    assert_eq!(action["kind"], "quickfix");
    let edit = &action["edit"]["changes"][uri][0];
    assert_eq!(edit["range"]["start"]["line"], 3);
    assert_eq!(edit["range"]["start"]["character"], 4);
    assert_eq!(edit["range"]["end"]["line"], 3);
    assert_eq!(edit["range"]["end"]["character"], 4);
    assert_eq!(edit["newText"], "    Option::None => null,\n    ");
}

#[test]
fn lsp_code_action_adds_missing_record_fields() {
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
    amount: i64,
    reason: String = \"quest\",
}

pub fn main() {
    return Reward { reason: \"bonus\" }
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

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/codeAction",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 6, "character": 11 },
                "end": { "line": 6, "character": 37 }
            },
            "context": { "diagnostics": [] }
        }),
    )));
    let actions = response["result"]
        .as_array()
        .expect("codeAction should return an array");
    let action = actions
        .iter()
        .find(|action| action["title"] == "Add missing field `amount`")
        .expect("missing record field quick fix should be returned");

    assert_eq!(action["kind"], "quickfix");
    let edit = &action["edit"]["changes"][uri][0];
    assert_eq!(edit["range"]["start"]["line"], 6);
    assert_eq!(edit["range"]["start"]["character"], 36);
    assert_eq!(edit["range"]["end"]["line"], 6);
    assert_eq!(edit["range"]["end"]["character"], 36);
    assert_eq!(edit["newText"], ", amount: null");
}

#[test]
fn lsp_code_action_rejects_ambiguous_import_fix() {
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
    for (uri, text) in [
        (
            "file:///workspace/scripts/game/reward.vela",
            "pub fn grant() { return 1 }",
        ),
        (
            "file:///workspace/scripts/game/bonus.vela",
            "pub fn grant() { return 2 }",
        ),
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
    let text = "pub fn main() { return grant }";
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

    let grant_start = text.find("grant").expect("unresolved symbol");
    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/codeAction",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": grant_start },
                "end": { "line": 0, "character": grant_start + "grant".len() }
            },
            "context": { "diagnostics": [] }
        }),
    )));

    assert_eq!(response["result"], serde_json::json!([]));
}

#[test]
fn lsp_code_action_rejects_dynamic_receiver_typo_fix() {
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
    let text = "pub fn main(player) { return player.levle }";
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

    let typo_start = text.find("levle").expect("dynamic receiver typo");
    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/codeAction",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": typo_start },
                "end": { "line": 0, "character": typo_start + "levle".len() }
            },
            "context": { "diagnostics": [] }
        }),
    )));

    assert_eq!(response["result"], serde_json::json!([]));
}

#[test]
fn lsp_code_action_rejects_source_any_return_receiver_typo_fix() {
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
fn source_any() -> Any { return 1 }
pub fn main() {
    return source_any().levle
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

    let typo_line = text.lines().nth(2).expect("typo line");
    let typo_start = typo_line.find("levle").expect("source Any receiver typo");
    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/codeAction",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 2, "character": typo_start },
                "end": { "line": 2, "character": typo_start + "levle".len() }
            },
            "context": { "diagnostics": [] }
        }),
    )));

    assert_eq!(response["result"], serde_json::json!([]));
}

#[test]
fn lsp_code_action_ranges_follow_open_overlay_text() {
    let root = temp_workspace();
    let config_path = root.join("vela.toml");
    let schema_path = root.join("target").join("vela").join("schema.json");
    let main_path = root.join("scripts").join("game").join("main.vela");
    fs::create_dir_all(schema_path.parent().expect("schema should have parent"))
        .expect("schema directory should be creatable");
    fs::create_dir_all(main_path.parent().expect("source should have parent"))
        .expect("source directory should be creatable");
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
    fs::write(&schema_path, schema_with_player_level_field()).expect("schema should be writable");
    fs::write(
        &main_path,
        "pub fn main(player: Player) { return player.level }",
    )
    .expect("disk source should be writable");

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
            "changes": [
                { "uri": file_uri(&config_path), "type": 1 },
                { "uri": file_uri(&main_path), "type": 1 }
            ]
        }),
    ));

    let overlay_text = "\npub fn main(player: Player) {\n    return player.levle\n}";
    let main_uri = file_uri(&main_path);
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": main_uri,
                "languageId": "vela",
                "version": 1,
                "text": overlay_text
            }
        }),
    )));
    let typo_line = overlay_text.lines().nth(2).expect("typo line");
    let typo_start = typo_line.find("levle").expect("overlay typo");

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/codeAction",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "range": {
                "start": { "line": 2, "character": typo_start },
                "end": { "line": 2, "character": typo_start + "levle".len() }
            },
            "context": { "diagnostics": [] }
        }),
    )));
    let actions = response["result"]
        .as_array()
        .expect("codeAction should return an array");
    let action = actions
        .iter()
        .find(|action| action["title"] == "Replace with `level`")
        .expect("overlay-backed typo quick fix should be returned");

    let edit = &action["edit"]["changes"][main_uri][0];
    assert_eq!(edit["range"]["start"]["line"], 2);
    assert_eq!(edit["range"]["start"]["character"], typo_start);
    assert_eq!(edit["range"]["end"]["line"], 2);
    assert_eq!(
        edit["range"]["end"]["character"],
        typo_start + "levle".len()
    );
    assert_eq!(edit["newText"], "level");

    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

fn schema_with_player_level_field() -> &'static str {
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

fn temp_workspace() -> PathBuf {
    let suffix = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(error) => panic!("system time should be after UNIX_EPOCH: {error}"),
    };
    let root = std::env::temp_dir().join(format!(
        "vela_lsp_code_action_{}_{}",
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
