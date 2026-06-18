use super::{LspServer, notification, notification_value, request, response_value};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn lsp_inlay_hints_suppress_any_lambda_parameter_facts() {
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
                        "name": "values",
                        "fact": {
                            "kind": "array",
                            "element": { "kind": "any" }
                        }
                    },
                    {
                        "owner": "Player",
                        "name": "rewards",
                        "fact": {
                            "kind": "map",
                            "key": { "kind": "primitive", "name": "string" },
                            "value": { "kind": "any" }
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

    let uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    let text = r#"pub fn main(player: Player) {
    let ignored: Array<Any> = player.values.map(|value| value);
    let filtered: Map<String, Any> = player.rewards.filter(|key, value| true);
    let stable: Array<i64> = [1, 2, 3];
    let mapped: Array<i64> = stable.map(|score| score + 1);
}"#;
    let filter_line = text.lines().nth(2).expect("filter line");
    let mapped_line = text.lines().nth(4).expect("mapped line");
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
        "textDocument/inlayHint",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 6, "character": 0 }
            }
        }),
    )));

    assert_eq!(
        response["result"],
        serde_json::json!([
            {
                "position": {
                    "line": 2,
                    "character": filter_line.find("key").expect("stable map key param")
                        + "key".len()
                },
                "label": ": String",
                "kind": 1,
                "paddingRight": true
            },
            {
                "position": {
                    "line": 4,
                    "character": mapped_line.find("score").expect("stable lambda param")
                        + "score".len()
                },
                "label": ": i64",
                "kind": 1,
                "paddingRight": true
            }
        ])
    );

    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_inlay_hints_suppress_any_schema_method_parameters_on_schema_function_return_receiver() {
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
                    }
                ],
                "methods": [
                    {
                        "owner": "Player",
                        "name": "grant",
                        "fact": {
                            "kind": "function",
                            "params": [
                                { "kind": "any" },
                                { "kind": "primitive", "name": "i64" }
                            ],
                            "returns": { "kind": "primitive", "name": "i64" }
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

    let uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    let text = r#"pub fn main() {
    current_player().grant("raw", 1)
    return current_player().grant("again", 2)
}"#;
    let first_call = text.lines().nth(1).expect("first call line");
    let second_call = text.lines().nth(2).expect("second call line");
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
        "textDocument/inlayHint",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 4, "character": 0 }
            }
        }),
    )));

    assert_eq!(
        response["result"],
        serde_json::json!([
            {
                "position": {
                    "line": 1,
                    "character": first_call.find(", 1").expect("first count arg") + 2
                },
                "label": "arg1:",
                "kind": 2,
                "paddingRight": true
            },
            {
                "position": {
                    "line": 2,
                    "character": second_call.find(", 2").expect("second count arg") + 2
                },
                "label": "arg1:",
                "kind": 2,
                "paddingRight": true
            }
        ])
    );

    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_inlay_hints_suppress_any_schema_trait_method_parameters_on_schema_function_return_receiver()
{
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
                "functions": [
                    {
                        "name": "current_rewardable",
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
                            "params": [
                                { "kind": "any" },
                                { "kind": "primitive", "name": "i64" }
                            ],
                            "returns": { "kind": "primitive", "name": "string" }
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

    let uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    let text = r#"pub fn main() {
    current_rewardable().preview("raw", 1)
    let summary = current_rewardable().preview("again", 2)
}"#;
    let first_call = text.lines().nth(1).expect("first call line");
    let second_call = text.lines().nth(2).expect("second call line");
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
        "textDocument/inlayHint",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 4, "character": 0 }
            }
        }),
    )));

    assert_eq!(
        response["result"],
        serde_json::json!([
            {
                "position": {
                    "line": 1,
                    "character": first_call.find(", 1").expect("first count arg") + 2
                },
                "label": "arg1:",
                "kind": 2,
                "paddingRight": true
            },
            {
                "position": { "line": 2, "character": "    let summary".len() },
                "label": ": String",
                "kind": 1,
                "paddingRight": true
            },
            {
                "position": {
                    "line": 2,
                    "character": second_call.find(", 2").expect("second count arg") + 2
                },
                "label": "arg1:",
                "kind": 2,
                "paddingRight": true
            }
        ])
    );

    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_inlay_hints_suppress_any_source_trait_default_method_parameters_on_source_function_return_receiver()
 {
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
    let text = r#"trait Rewardable {
    fn preview(self, raw: Any, count: i64) -> String { return "ok" }
}
struct Player { level: i64 }
impl Rewardable for Player {}
fn current_player() -> Player { return Player { level: 1 } }
pub fn main() {
    current_player().preview("raw", 1)
    return current_player().preview("again", 2)
}"#;
    let first_call = text.lines().nth(7).expect("first call line");
    let second_call = text.lines().nth(8).expect("second call line");
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
        "textDocument/inlayHint",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 10, "character": 0 }
            }
        }),
    )));

    assert_eq!(
        response["result"],
        serde_json::json!([
            {
                "position": {
                    "line": 7,
                    "character": first_call.find(", 1").expect("first count arg") + 2
                },
                "label": "count:",
                "kind": 2,
                "paddingRight": true
            },
            {
                "position": {
                    "line": 8,
                    "character": second_call.find(", 2").expect("second count arg") + 2
                },
                "label": "count:",
                "kind": 2,
                "paddingRight": true
            }
        ])
    );
}

#[test]
fn lsp_inlay_hints_suppress_any_source_method_parameters_on_source_method_return_receiver() {
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
    let text = r#"struct Player { level: i64 }
struct Inventory { count: i64 }
impl Player {
    fn inventory(self) -> Inventory { return Inventory { count: 1 } }
}
impl Inventory {
    fn grant(self, raw: Any, count: i64) -> i64 { return count }
}
pub fn main(player: Player) {
    player.inventory().grant("raw", 1)
    return player.inventory().grant("again", 2)
}"#;
    let first_call = text.lines().nth(9).expect("first call line");
    let second_call = text.lines().nth(10).expect("second call line");
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
        "textDocument/inlayHint",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 12, "character": 0 }
            }
        }),
    )));

    assert_eq!(
        response["result"],
        serde_json::json!([
            {
                "position": {
                    "line": 9,
                    "character": first_call.find(", 1").expect("first count arg") + 2
                },
                "label": "count:",
                "kind": 2,
                "paddingRight": true
            },
            {
                "position": {
                    "line": 10,
                    "character": second_call.find(", 2").expect("second count arg") + 2
                },
                "label": "count:",
                "kind": 2,
                "paddingRight": true
            }
        ])
    );
}

fn temp_workspace() -> PathBuf {
    let suffix = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(error) => panic!("system time should be after UNIX_EPOCH: {error}"),
    };
    let root = std::env::temp_dir().join(format!(
        "vela_lsp_inlay_suppression_{}_{}",
        std::process::id(),
        suffix
    ));
    if let Err(error) = fs::create_dir_all(root.join("scripts").join("game")) {
        panic!("temporary workspace should be creatable: {error}");
    }
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
