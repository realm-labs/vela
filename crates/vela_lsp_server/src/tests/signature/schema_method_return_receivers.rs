use std::fs;

use super::{file_uri, temp_workspace};
use crate::tests::{
    LspServer, handle_notification, handle_request, notification_value, response_value,
};

#[test]
fn lsp_signature_help_resolves_schema_method_on_schema_method_return() {
    let root = temp_workspace();
    let schema_path = root.join("target").join("vela").join("schema.json");
    fs::create_dir_all(schema_path.parent().expect("schema should have parent"))
        .expect("schema directory should be creatable");
    fs::write(&schema_path, schema_with_inventory_method_return())
        .expect("schema artifact should be writable");

    let mut server = LspServer::new();
    let _ = response_value(handle_request(
        &mut server,
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
    ));
    let main_uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    let text = "pub fn main(player: Player) { player.inventory().grant(1, 2) }";
    let _ = notification_value(handle_notification(
        &mut server,
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": main_uri,
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    ));

    let response = response_value(handle_request(
        &mut server,
        2,
        "textDocument/signatureHelp",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 0,
                "character": text.find("2)").unwrap_or_else(|| {
                    panic!("signature fixture should contain second argument")
                })
            }
        }),
    ));

    assert_eq!(response["result"]["activeSignature"], 0);
    assert_eq!(response["result"]["activeParameter"], 1);
    assert_eq!(
        response["result"]["signatures"][0]["label"],
        "Inventory.grant(arg0: i64, arg1: i64) -> bool"
    );
    assert_eq!(
        response["result"]["signatures"][0]["parameters"][1]["label"],
        "arg1: i64"
    );
    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_signature_help_resolves_schema_trait_method_on_schema_method_return() {
    let root = temp_workspace();
    let schema_path = root.join("target").join("vela").join("schema.json");
    fs::create_dir_all(schema_path.parent().expect("schema should have parent"))
        .expect("schema directory should be creatable");
    fs::write(&schema_path, schema_with_rewardable_method_return())
        .expect("schema artifact should be writable");

    let mut server = LspServer::new();
    let _ = response_value(handle_request(
        &mut server,
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
    ));
    let main_uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    let text = "pub fn main(player: Player) { player.rewardable().preview(1, 2) }";
    let _ = notification_value(handle_notification(
        &mut server,
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": main_uri,
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    ));

    let response = response_value(handle_request(
        &mut server,
        2,
        "textDocument/signatureHelp",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 0,
                "character": text.find("2)").unwrap_or_else(|| {
                    panic!("signature fixture should contain second argument")
                })
            }
        }),
    ));

    assert_eq!(response["result"]["activeSignature"], 0);
    assert_eq!(response["result"]["activeParameter"], 1);
    assert_eq!(
        response["result"]["signatures"][0]["label"],
        "Rewardable.preview(arg0: i64, arg1: i64) -> bool"
    );
    assert_eq!(
        response["result"]["signatures"][0]["parameters"][1]["label"],
        "arg1: i64"
    );
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
                        "params": [
                            { "kind": "primitive", "name": "i64" },
                            { "kind": "primitive", "name": "i64" }
                        ],
                        "returns": { "kind": "primitive", "name": "bool" }
                    }
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
                        "params": [
                            { "kind": "primitive", "name": "i64" },
                            { "kind": "primitive", "name": "i64" }
                        ],
                        "returns": { "kind": "primitive", "name": "bool" }
                    }
                }
            ]
        }
    }"#
}
