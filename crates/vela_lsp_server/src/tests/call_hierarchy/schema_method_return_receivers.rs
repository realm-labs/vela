use std::{fs, path::PathBuf};

use crate::tests::{LspServer, notification, notification_value, request, response_value};

use super::{assert_call_range, assert_outgoing_call, file_uri, line, temp_workspace};

#[test]
fn lsp_call_hierarchy_uses_schema_method_calls_on_schema_method_return_receivers() {
    let mut fixture = open_schema_method_return_fixture();

    let prepare_grant = response_value(fixture.server.handle_json(&request(
        2,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": fixture.schema_uri },
            "position": {
                "line": 1,
                "character": line(fixture.schema_text, 1)
                    .find("grant")
                    .expect("grant declaration")
            }
        }),
    )));
    let grant_items = prepare_grant["result"]
        .as_array()
        .expect("prepareCallHierarchy response should be an array");
    assert_eq!(grant_items.len(), 1);
    assert_eq!(grant_items[0]["name"], "grant");
    assert_eq!(grant_items[0]["uri"], fixture.schema_uri);

    let prepare_grant_call = response_value(fixture.server.handle_json(&request(
        3,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": fixture.uri },
            "position": {
                "line": 1,
                "character": line(fixture.text, 1).find("grant").expect("grant call")
            }
        }),
    )));
    assert_eq!(
        prepare_grant_call["result"]
            .as_array()
            .expect("prepareCallHierarchy response should be an array"),
        grant_items
    );

    let incoming = response_value(fixture.server.handle_json(&request(
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
        line(fixture.text, 1)
            .find("grant")
            .expect("first grant call"),
    );
    assert_call_range(
        incoming_ranges,
        2,
        line(fixture.text, 2)
            .find("grant")
            .expect("second grant call"),
    );

    let main_items = prepare_main_items(&mut fixture);
    let outgoing = response_value(fixture.server.handle_json(&request(
        5,
        "callHierarchy/outgoingCalls",
        serde_json::json!({ "item": main_items[0].clone() }),
    )));
    let outgoing_calls = outgoing["result"]
        .as_array()
        .expect("outgoingCalls response should be an array");
    assert_eq!(outgoing_calls.len(), 2, "{outgoing_calls:?}");
    assert_outgoing_call(
        outgoing_calls,
        "inventory",
        &fixture.schema_uri,
        1,
        line(fixture.text, 1)
            .find("inventory")
            .expect("first inventory call"),
    );
    assert_outgoing_call(
        outgoing_calls,
        "grant",
        &fixture.schema_uri,
        1,
        line(fixture.text, 1)
            .find("grant")
            .expect("first grant call"),
    );
    assert_outgoing_call(
        outgoing_calls,
        "inventory",
        &fixture.schema_uri,
        2,
        line(fixture.text, 2)
            .find("inventory")
            .expect("second inventory call"),
    );
    assert_outgoing_call(
        outgoing_calls,
        "grant",
        &fixture.schema_uri,
        2,
        line(fixture.text, 2)
            .find("grant")
            .expect("second grant call"),
    );

    fs::remove_dir_all(&fixture.root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_call_hierarchy_uses_schema_trait_method_calls_on_schema_method_return_receivers() {
    let mut fixture = open_schema_trait_method_return_fixture();

    let prepare_preview = response_value(fixture.server.handle_json(&request(
        2,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": fixture.schema_uri },
            "position": {
                "line": 1,
                "character": line(fixture.schema_text, 1)
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
    assert_eq!(preview_items[0]["uri"], fixture.schema_uri);

    let prepare_preview_call = response_value(fixture.server.handle_json(&request(
        3,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": fixture.uri },
            "position": {
                "line": 1,
                "character": line(fixture.text, 1)
                    .find("preview")
                    .expect("preview call")
            }
        }),
    )));
    assert_eq!(
        prepare_preview_call["result"]
            .as_array()
            .expect("prepareCallHierarchy response should be an array"),
        preview_items
    );

    let incoming = response_value(fixture.server.handle_json(&request(
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
        line(fixture.text, 1)
            .find("preview")
            .expect("first preview call"),
    );
    assert_call_range(
        incoming_ranges,
        2,
        line(fixture.text, 2)
            .find("preview")
            .expect("second preview call"),
    );

    let main_items = prepare_main_items(&mut fixture);
    let outgoing = response_value(fixture.server.handle_json(&request(
        5,
        "callHierarchy/outgoingCalls",
        serde_json::json!({ "item": main_items[0].clone() }),
    )));
    let outgoing_calls = outgoing["result"]
        .as_array()
        .expect("outgoingCalls response should be an array");
    assert_eq!(outgoing_calls.len(), 2, "{outgoing_calls:?}");
    assert_outgoing_call(
        outgoing_calls,
        "rewardable",
        &fixture.schema_uri,
        1,
        line(fixture.text, 1)
            .find("rewardable")
            .expect("first rewardable call"),
    );
    assert_outgoing_call(
        outgoing_calls,
        "preview",
        &fixture.schema_uri,
        1,
        line(fixture.text, 1)
            .find("preview")
            .expect("first preview call"),
    );
    assert_outgoing_call(
        outgoing_calls,
        "rewardable",
        &fixture.schema_uri,
        2,
        line(fixture.text, 2)
            .find("rewardable")
            .expect("second rewardable call"),
    );
    assert_outgoing_call(
        outgoing_calls,
        "preview",
        &fixture.schema_uri,
        2,
        line(fixture.text, 2)
            .find("preview")
            .expect("second preview call"),
    );

    fs::remove_dir_all(&fixture.root).expect("temporary workspace should be removable");
}

struct SchemaReturnFixture {
    server: LspServer,
    root: PathBuf,
    uri: String,
    schema_uri: String,
    text: &'static str,
    schema_text: &'static str,
}

fn open_schema_method_return_fixture() -> SchemaReturnFixture {
    let schema_text = "\
pub fn inventory() { return 1 }
pub fn grant() { return 1 }";
    let inventory_start = schema_text.find("inventory").expect("inventory marker");
    let grant_start = schema_text.find("grant").expect("grant marker");
    open_fixture(
        schema_text,
        serde_json::json!({
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
                        },
                        "sourceSpan": {
                            "source": 1,
                            "start": inventory_start,
                            "end": inventory_start + "inventory".len()
                        }
                    },
                    {
                        "owner": "Inventory",
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
        "\
pub fn main(player: Player) -> i64 {
    let first = player.inventory().grant(1)
    return player.inventory().grant(first)
}",
    )
}

fn open_schema_trait_method_return_fixture() -> SchemaReturnFixture {
    let schema_text = "\
pub fn rewardable() { return 1 }
pub fn preview() { return 1 }";
    let rewardable_start = schema_text.find("rewardable").expect("rewardable marker");
    let preview_start = schema_text.find("preview").expect("preview marker");
    open_fixture(
        schema_text,
        serde_json::json!({
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
                        },
                        "sourceSpan": {
                            "source": 1,
                            "start": rewardable_start,
                            "end": rewardable_start + "rewardable".len()
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
        "\
pub fn main(player: Player) -> i64 {
    let first = player.rewardable().preview(1)
    return player.rewardable().preview(first)
}",
    )
}

fn open_fixture(
    schema_text: &'static str,
    schema_artifact: String,
    text: &'static str,
) -> SchemaReturnFixture {
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
    fs::write(&schema_path, schema_artifact).expect("schema should be writable");

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

    SchemaReturnFixture {
        server,
        root,
        uri,
        schema_uri,
        text,
        schema_text,
    }
}

fn prepare_main_items(fixture: &mut SchemaReturnFixture) -> Vec<serde_json::Value> {
    let prepare_main = response_value(fixture.server.handle_json(&request(
        6,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": fixture.uri },
            "position": {
                "line": 0,
                "character": line(fixture.text, 0).find("main").expect("main declaration")
            }
        }),
    )));
    let main_items = prepare_main["result"]
        .as_array()
        .expect("prepareCallHierarchy response should be an array");
    assert_eq!(main_items.len(), 1);
    main_items.clone()
}
