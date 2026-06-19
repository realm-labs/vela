use std::fs;

use crate::tests::{
    LspServer, handle_notification, handle_request, notification_value, response_value,
};

use super::{file_uri, temp_workspace};

#[test]
fn lsp_inlay_hints_show_host_path_typefacts_on_schema_method_return_receiver() {
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
                "methods": [
                    {
                        "owner": "Player",
                        "name": "inventory",
                        "fact": {
                            "kind": "function",
                            "params": [],
                            "returns": { "kind": "host", "name": "Inventory" }
                        }
                    }
                ],
                "fields": [
                    {
                        "owner": "Inventory",
                        "name": "slots",
                        "fact": { "kind": "primitive", "name": "i64" }
                    },
                    {
                        "owner": "Inventory",
                        "name": "mystery",
                        "fact": { "kind": "any" }
                    }
                ]
            }
        }"#,
    )
    .expect("schema should be writable");

    let mut server = LspServer::new();
    let _ = response_value(handle_request(
        &mut server,
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": file_uri(&root),
            "capabilities": {}
        }),
    ));
    let _ = handle_notification(
        &mut server,
        "workspace/didChangeWatchedFiles",
        serde_json::json!({
            "changes": [{ "uri": file_uri(&config_path), "type": 1 }]
        }),
    );
    let uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    let text = r#"pub fn main(player: Player) {
    let slots = player.inventory().slots + 1;
    player.inventory().slots += slots;
    let dynamic = player.inventory().mystery;
}"#;
    let first_line = text.lines().nth(1).expect("first slots line");
    let second_line = text.lines().nth(2).expect("second slots line");
    let _ = notification_value(handle_notification(
        &mut server,
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    ));

    let response = response_value(handle_request(
        &mut server,
        2,
        "textDocument/inlayHint",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 5, "character": 0 }
            }
        }),
    ));

    assert_eq!(
        response["result"],
        serde_json::json!([
            {
                "position": { "line": 1, "character": "    let slots".len() },
                "label": ": i64",
                "kind": 1,
                "paddingRight": true
            },
            {
                "position": {
                    "line": 1,
                    "character": first_line.find("slots +").expect("first slots field")
                        + "slots".len()
                },
                "label": ": i64",
                "kind": 1,
                "paddingRight": true
            },
            {
                "position": {
                    "line": 2,
                    "character": second_line.find("slots +=").expect("second slots field")
                        + "slots".len()
                },
                "label": ": i64",
                "kind": 1,
                "paddingRight": true
            }
        ])
    );

    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}
