use std::fs;

use super::*;

#[test]
fn lsp_hover_reports_schema_method_on_schema_method_return_receiver() {
    let root = temp_workspace();
    let schema_path = root.join("target").join("vela").join("schema.json");
    fs::create_dir_all(schema_path.parent().expect("schema should have parent"))
        .expect("schema directory should be creatable");
    fs::write(&schema_path, schema_with_inventory_method_return())
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
    let text = "pub fn main(player: Player) { player.inventory().grant(1) }";
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
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 0,
                "character": text.find("grant").unwrap_or_else(|| {
                    panic!("hover fixture should contain chained schema method")
                })
            }
        }),
    )));

    let value = response["result"]["contents"]["value"]
        .as_str()
        .expect("hover contents should be markdown");
    assert!(value.contains("Inventory.grant"), "{value}");
    assert!(value.contains("_method_: Function(i64) -> bool"), "{value}");
    assert!(value.contains("Grant inventory items."), "{value}");
    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_hover_reports_schema_trait_method_on_schema_method_return_receiver() {
    let root = temp_workspace();
    let schema_path = root.join("target").join("vela").join("schema.json");
    fs::create_dir_all(schema_path.parent().expect("schema should have parent"))
        .expect("schema directory should be creatable");
    fs::write(&schema_path, schema_with_rewardable_method_return())
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
    let text = "pub fn main(player: Player) { player.rewardable().preview(1) }";
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
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 0,
                "character": text.find("preview").unwrap_or_else(|| {
                    panic!("hover fixture should contain chained schema trait method")
                })
            }
        }),
    )));

    let value = response["result"]["contents"]["value"]
        .as_str()
        .expect("hover contents should be markdown");
    assert!(value.contains("Rewardable.preview"), "{value}");
    assert!(value.contains("_method_: Function(i64) -> bool"), "{value}");
    assert!(value.contains("Preview a reward."), "{value}");
    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

fn schema_with_inventory_method_return() -> &'static str {
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
                },
                {
                    "owner": "Inventory",
                    "name": "grant",
                    "fact": {
                        "kind": "function",
                        "params": [{ "kind": "primitive", "name": "i64" }],
                        "returns": { "kind": "primitive", "name": "bool" }
                    },
                    "docs": "Grant inventory items."
                }
            ]
        }
    }"#
}

fn schema_with_rewardable_method_return() -> &'static str {
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
                    },
                    "docs": "Preview a reward."
                }
            ]
        }
    }"#
}
