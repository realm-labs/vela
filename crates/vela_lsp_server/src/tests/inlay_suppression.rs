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
