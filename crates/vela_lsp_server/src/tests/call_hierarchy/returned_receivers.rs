use std::fs;

use crate::tests::{LspServer, notification, notification_value, request, response_value};

use super::{assert_call_range, assert_outgoing_call, file_uri, line, temp_workspace};

#[test]
fn lsp_call_hierarchy_uses_schema_method_calls_on_schema_function_return_receivers() {
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
    let initialize = response_value(server.handle_json(&request(
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": file_uri(&root),
            "capabilities": {}
        }),
    )));
    assert_eq!(
        initialize["result"]["capabilities"]["callHierarchyProvider"],
        true
    );
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
pub fn main() -> i64 {
    let first = current_player().grant(1)
    return current_player().grant(first)
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

    let prepare_grant = response_value(server.handle_json(&request(
        2,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": schema_uri },
            "position": {
                "line": 0,
                "character": line(schema_text, 0).find("grant").expect("grant declaration")
            }
        }),
    )));
    let grant_items = prepare_grant["result"]
        .as_array()
        .expect("prepareCallHierarchy response should be an array");
    assert_eq!(grant_items.len(), 1);
    assert_eq!(grant_items[0]["name"], "grant");
    assert_eq!(grant_items[0]["uri"], schema_uri);

    let prepare_grant_call = response_value(server.handle_json(&request(
        3,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 1,
                "character": line(text, 1).find("grant").expect("grant call")
            }
        }),
    )));
    assert_eq!(
        prepare_grant_call["result"]
            .as_array()
            .expect("prepareCallHierarchy response should be an array"),
        grant_items
    );

    let incoming = response_value(server.handle_json(&request(
        4,
        "callHierarchy/incomingCalls",
        serde_json::json!({ "item": grant_items[0].clone() }),
    )));
    let incoming_calls = incoming["result"]
        .as_array()
        .expect("incomingCalls response should be an array");
    assert_eq!(incoming_calls.len(), 1, "{incoming_calls:?}");
    assert_eq!(incoming_calls[0]["from"]["name"], "main");
    let incoming_ranges = incoming_calls[0]["fromRanges"]
        .as_array()
        .expect("incoming call should include ranges");
    assert_call_range(
        incoming_ranges,
        1,
        line(text, 1).find("grant").expect("first grant call"),
    );
    assert_call_range(
        incoming_ranges,
        2,
        line(text, 2).find("grant").expect("second grant call"),
    );

    let prepare_main = response_value(server.handle_json(&request(
        5,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 0,
                "character": line(text, 0).find("main").expect("main declaration")
            }
        }),
    )));
    let main_items = prepare_main["result"]
        .as_array()
        .expect("prepareCallHierarchy response should be an array");
    assert_eq!(main_items.len(), 1);

    let outgoing = response_value(server.handle_json(&request(
        6,
        "callHierarchy/outgoingCalls",
        serde_json::json!({ "item": main_items[0].clone() }),
    )));
    let outgoing_calls = outgoing["result"]
        .as_array()
        .expect("outgoingCalls response should be an array");
    assert_eq!(outgoing_calls.len(), 1, "{outgoing_calls:?}");
    assert_outgoing_call(
        outgoing_calls,
        "grant",
        &schema_uri,
        1,
        line(text, 1).find("grant").expect("first grant call"),
    );
    assert_outgoing_call(
        outgoing_calls,
        "grant",
        &schema_uri,
        2,
        line(text, 2).find("grant").expect("second grant call"),
    );

    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_call_hierarchy_uses_schema_trait_method_calls_on_schema_function_return_receivers() {
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

    let schema_text = "pub fn preview() { return 1 }";
    let preview_start = schema_text.find("preview").expect("preview marker");
    fs::write(
        &schema_path,
        serde_json::json!({
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
                        "name": "current_reward",
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
                            "returns": { "kind": "primitive", "name": "i64" }
                        },
                        "sourceSpan": {
                            "source": 1,
                            "start": preview_start,
                            "end": preview_start + "preview".len()
                        }
                    }
                ]
            }
        })
        .to_string(),
    )
    .expect("schema should be writable");

    let mut server = LspServer::new();
    let initialize = response_value(server.handle_json(&request(
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": file_uri(&root),
            "capabilities": {}
        }),
    )));
    assert_eq!(
        initialize["result"]["capabilities"]["callHierarchyProvider"],
        true
    );
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
pub fn main() -> i64 {
    let first = current_reward().preview(1)
    return current_reward().preview(first)
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

    let prepare_preview = response_value(server.handle_json(&request(
        2,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": schema_uri },
            "position": {
                "line": 0,
                "character": line(schema_text, 0)
                    .find("preview")
                    .expect("preview declaration")
            }
        }),
    )));
    let preview_items = prepare_preview["result"]
        .as_array()
        .expect("prepareCallHierarchy response should be an array");
    assert_eq!(preview_items.len(), 1);
    assert_eq!(preview_items[0]["name"], "preview");
    assert_eq!(preview_items[0]["uri"], schema_uri);

    let prepare_preview_call = response_value(server.handle_json(&request(
        3,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 1,
                "character": line(text, 1).find("preview").expect("preview call")
            }
        }),
    )));
    assert_eq!(
        prepare_preview_call["result"]
            .as_array()
            .expect("prepareCallHierarchy response should be an array"),
        preview_items
    );

    let incoming = response_value(server.handle_json(&request(
        4,
        "callHierarchy/incomingCalls",
        serde_json::json!({ "item": preview_items[0].clone() }),
    )));
    let incoming_calls = incoming["result"]
        .as_array()
        .expect("incomingCalls response should be an array");
    assert_eq!(incoming_calls.len(), 1, "{incoming_calls:?}");
    assert_eq!(incoming_calls[0]["from"]["name"], "main");
    let incoming_ranges = incoming_calls[0]["fromRanges"]
        .as_array()
        .expect("incoming call should include ranges");
    assert_call_range(
        incoming_ranges,
        1,
        line(text, 1).find("preview").expect("first preview call"),
    );
    assert_call_range(
        incoming_ranges,
        2,
        line(text, 2).find("preview").expect("second preview call"),
    );

    let prepare_main = response_value(server.handle_json(&request(
        5,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 0,
                "character": line(text, 0).find("main").expect("main declaration")
            }
        }),
    )));
    let main_items = prepare_main["result"]
        .as_array()
        .expect("prepareCallHierarchy response should be an array");
    assert_eq!(main_items.len(), 1);

    let outgoing = response_value(server.handle_json(&request(
        6,
        "callHierarchy/outgoingCalls",
        serde_json::json!({ "item": main_items[0].clone() }),
    )));
    let outgoing_calls = outgoing["result"]
        .as_array()
        .expect("outgoingCalls response should be an array");
    assert_eq!(outgoing_calls.len(), 1, "{outgoing_calls:?}");
    assert_outgoing_call(
        outgoing_calls,
        "preview",
        &schema_uri,
        1,
        line(text, 1).find("preview").expect("first preview call"),
    );
    assert_outgoing_call(
        outgoing_calls,
        "preview",
        &schema_uri,
        2,
        line(text, 2).find("preview").expect("second preview call"),
    );

    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}
