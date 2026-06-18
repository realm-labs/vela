use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use super::{LspServer, notification, notification_value, request, response_value};

#[test]
fn lsp_call_hierarchy_uses_resolved_call_graph() {
    let mut server = LspServer::new();
    let initialize = response_value(server.handle_json(&request(
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": "file:///workspace/scripts",
            "capabilities": {}
        }),
    )));
    assert_eq!(
        initialize["result"]["capabilities"]["callHierarchyProvider"],
        true
    );
    let main_text = "\
use game::reward::grant
pub fn main(amount: i64) -> i64 {
    let first = grant(amount)
    return grant(first)
}";
    let helper_text = "pub fn grant(amount: i64) -> i64 { return amount }";
    let main_uri = "file:///workspace/scripts/game/main.vela";
    let helper_uri = "file:///workspace/scripts/game/reward.vela";
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": helper_uri,
                "languageId": "vela",
                "version": 1,
                "text": helper_text
            }
        }),
    )));
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": main_uri,
                "languageId": "vela",
                "version": 1,
                "text": main_text
            }
        }),
    )));

    let prepare_grant = response_value(server.handle_json(&request(
        2,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": helper_uri },
            "position": {
                "line": 0,
                "character": helper_text.find("grant").expect("grant declaration")
            }
        }),
    )));
    let grant_items = prepare_grant["result"]
        .as_array()
        .expect("prepareCallHierarchy response should be an array");
    assert_eq!(grant_items.len(), 1);
    assert_eq!(grant_items[0]["name"], "grant");
    assert_eq!(grant_items[0]["kind"], 12);
    assert_eq!(grant_items[0]["uri"], helper_uri);

    let incoming = response_value(server.handle_json(&request(
        3,
        "callHierarchy/incomingCalls",
        serde_json::json!({ "item": grant_items[0].clone() }),
    )));
    let incoming_calls = incoming["result"]
        .as_array()
        .expect("incomingCalls response should be an array");
    assert_eq!(incoming_calls.len(), 1);
    assert_eq!(incoming_calls[0]["from"]["name"], "main");
    assert_eq!(incoming_calls[0]["from"]["uri"], main_uri);
    assert_call_range(
        incoming_calls[0]["fromRanges"]
            .as_array()
            .expect("incoming call should include ranges"),
        2,
        line(main_text, 2).find("grant").expect("first call"),
    );
    assert_call_range(
        incoming_calls[0]["fromRanges"]
            .as_array()
            .expect("incoming call should include ranges"),
        3,
        line(main_text, 3).find("grant").expect("second call"),
    );

    let prepare_main = response_value(server.handle_json(&request(
        4,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 1,
                "character": line(main_text, 1).find("main").expect("main declaration")
            }
        }),
    )));
    let main_items = prepare_main["result"]
        .as_array()
        .expect("prepareCallHierarchy response should be an array");
    assert_eq!(main_items.len(), 1);

    let outgoing = response_value(server.handle_json(&request(
        5,
        "callHierarchy/outgoingCalls",
        serde_json::json!({ "item": main_items[0].clone() }),
    )));
    let outgoing_calls = outgoing["result"]
        .as_array()
        .expect("outgoingCalls response should be an array");
    assert_eq!(outgoing_calls.len(), 1);
    assert_eq!(outgoing_calls[0]["to"]["name"], "grant");
    assert_eq!(outgoing_calls[0]["to"]["uri"], helper_uri);
    assert_eq!(
        outgoing_calls[0]["fromRanges"]
            .as_array()
            .expect("outgoing call should include ranges")
            .len(),
        2
    );
}

#[test]
fn lsp_prepare_call_hierarchy_returns_empty_for_unresolved_dynamic_and_non_callable_targets() {
    let mut server = LspServer::new();
    let initialize = response_value(server.handle_json(&request(
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": "file:///workspace/scripts",
            "capabilities": {}
        }),
    )));
    assert_eq!(
        initialize["result"]["capabilities"]["callHierarchyProvider"],
        true
    );
    let text = "\
pub fn main(player) {
    missing(1)
    player.grant(1)
    let amount = 1
    return amount
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

    assert_empty_prepare_call_hierarchy(
        &mut server,
        2,
        uri,
        1,
        line(text, 1)
            .find("missing")
            .expect("unresolved call should exist"),
    );
    assert_empty_prepare_call_hierarchy(
        &mut server,
        3,
        uri,
        2,
        line(text, 2)
            .find("grant")
            .expect("dynamic receiver call should exist"),
    );
    assert_empty_prepare_call_hierarchy(
        &mut server,
        4,
        uri,
        4,
        line(text, 4)
            .find("amount")
            .expect("non-callable local use should exist"),
    );
}

#[test]
fn lsp_call_hierarchy_uses_resolved_script_method_calls() {
    let mut server = LspServer::new();
    let initialize = response_value(server.handle_json(&request(
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": "file:///workspace/scripts",
            "capabilities": {}
        }),
    )));
    assert_eq!(
        initialize["result"]["capabilities"]["callHierarchyProvider"],
        true
    );
    let text = "\
pub struct Reward {
    amount: i64
}

impl Reward {
    pub fn grant(self, amount: i64) -> i64 { return amount }
}

pub fn main(reward: Reward) -> i64 {
    let first = reward.grant(1)
    return reward.grant(first)
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

    let prepare_grant = response_value(server.handle_json(&request(
        2,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 5,
                "character": line(text, 5).find("grant").expect("method declaration")
            }
        }),
    )));
    let grant_items = prepare_grant["result"]
        .as_array()
        .expect("prepareCallHierarchy response should be an array");
    assert_eq!(grant_items.len(), 1);
    assert_eq!(grant_items[0]["name"], "grant");
    assert_eq!(grant_items[0]["kind"], 12);
    assert_eq!(grant_items[0]["uri"], uri);

    let prepare_from_call = response_value(server.handle_json(&request(
        3,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 9,
                "character": line(text, 9).find("grant").expect("method call")
            }
        }),
    )));
    let call_items = prepare_from_call["result"]
        .as_array()
        .expect("prepareCallHierarchy response should be an array");
    assert_eq!(call_items, grant_items);

    let incoming = response_value(server.handle_json(&request(
        4,
        "callHierarchy/incomingCalls",
        serde_json::json!({ "item": grant_items[0].clone() }),
    )));
    let incoming_calls = incoming["result"]
        .as_array()
        .expect("incomingCalls response should be an array");
    assert_eq!(incoming_calls.len(), 1);
    assert_eq!(incoming_calls[0]["from"]["name"], "main");
    assert_eq!(incoming_calls[0]["from"]["uri"], uri);
    assert_call_range(
        incoming_calls[0]["fromRanges"]
            .as_array()
            .expect("incoming call should include ranges"),
        9,
        line(text, 9).find("grant").expect("first method call"),
    );
    assert_call_range(
        incoming_calls[0]["fromRanges"]
            .as_array()
            .expect("incoming call should include ranges"),
        10,
        line(text, 10).find("grant").expect("second method call"),
    );

    let prepare_main = response_value(server.handle_json(&request(
        5,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 8,
                "character": line(text, 8).find("main").expect("main declaration")
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
    assert_eq!(outgoing_calls.len(), 1);
    assert_eq!(outgoing_calls[0]["to"]["name"], "grant");
    assert_eq!(outgoing_calls[0]["to"]["uri"], uri);
    assert_eq!(
        outgoing_calls[0]["fromRanges"]
            .as_array()
            .expect("outgoing call should include ranges")
            .len(),
        2
    );
}

#[test]
fn lsp_call_hierarchy_uses_resolved_trait_impl_method_calls() {
    let mut server = LspServer::new();
    let initialize = response_value(server.handle_json(&request(
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": "file:///workspace/scripts",
            "capabilities": {}
        }),
    )));
    assert_eq!(
        initialize["result"]["capabilities"]["callHierarchyProvider"],
        true
    );
    let text = "\
pub fn clamp(value: i64) -> i64 { return value }

pub trait Rewardable {
    fn grant(self, amount: i64) -> i64;
}

pub struct Player { level: i64 }

impl Rewardable for Player {
    fn grant(self, amount: i64) -> i64 { return clamp(amount) }
}

pub fn main(player: Player) -> i64 {
    let first = player.grant(1)
    return player.grant(first)
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

    let prepare_grant = response_value(server.handle_json(&request(
        2,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 9,
                "character": line(text, 9).find("grant").expect("method declaration")
            }
        }),
    )));
    let grant_items = prepare_grant["result"]
        .as_array()
        .expect("prepareCallHierarchy response should be an array");
    assert_eq!(grant_items.len(), 1);
    assert_eq!(grant_items[0]["name"], "grant");
    assert_eq!(grant_items[0]["kind"], 12);
    assert_eq!(grant_items[0]["uri"], uri);

    let prepare_from_call = response_value(server.handle_json(&request(
        3,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 13,
                "character": line(text, 13).find("grant").expect("method call")
            }
        }),
    )));
    let call_items = prepare_from_call["result"]
        .as_array()
        .expect("prepareCallHierarchy response should be an array");
    assert_eq!(call_items, grant_items);

    let incoming = response_value(server.handle_json(&request(
        4,
        "callHierarchy/incomingCalls",
        serde_json::json!({ "item": grant_items[0].clone() }),
    )));
    let incoming_calls = incoming["result"]
        .as_array()
        .expect("incomingCalls response should be an array");
    assert_eq!(incoming_calls.len(), 1);
    assert_eq!(incoming_calls[0]["from"]["name"], "main");
    assert_eq!(incoming_calls[0]["from"]["uri"], uri);
    assert_call_range(
        incoming_calls[0]["fromRanges"]
            .as_array()
            .expect("incoming call should include ranges"),
        13,
        line(text, 13).find("grant").expect("first method call"),
    );
    assert_call_range(
        incoming_calls[0]["fromRanges"]
            .as_array()
            .expect("incoming call should include ranges"),
        14,
        line(text, 14).find("grant").expect("second method call"),
    );

    let outgoing = response_value(server.handle_json(&request(
        5,
        "callHierarchy/outgoingCalls",
        serde_json::json!({ "item": grant_items[0].clone() }),
    )));
    let outgoing_calls = outgoing["result"]
        .as_array()
        .expect("outgoingCalls response should be an array");
    assert_eq!(outgoing_calls.len(), 1);
    assert_eq!(outgoing_calls[0]["to"]["name"], "clamp");
    assert_eq!(outgoing_calls[0]["to"]["uri"], uri);
    assert_call_range(
        outgoing_calls[0]["fromRanges"]
            .as_array()
            .expect("outgoing call should include ranges"),
        9,
        line(text, 9).find("clamp").expect("helper call"),
    );
}

#[test]
fn lsp_call_hierarchy_uses_trait_default_and_interface_methods() {
    let mut server = LspServer::new();
    let initialize = response_value(server.handle_json(&request(
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": "file:///workspace/scripts",
            "capabilities": {}
        }),
    )));
    assert_eq!(
        initialize["result"]["capabilities"]["callHierarchyProvider"],
        true
    );
    let text = "\
pub fn clamp(value: i64) -> i64 { return value }

pub trait Rewardable {
    fn grant(self, amount: i64) -> i64 { return clamp(amount) }
    fn preview(self, amount: i64) -> i64;
}

pub fn main(rewardable: Rewardable) -> i64 {
    let first = rewardable.grant(1)
    return rewardable.preview(first)
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

    let prepare_grant = response_value(server.handle_json(&request(
        2,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 3,
                "character": line(text, 3).find("grant").expect("default method")
            }
        }),
    )));
    let grant_items = prepare_grant["result"]
        .as_array()
        .expect("prepareCallHierarchy response should be an array");
    assert_eq!(grant_items.len(), 1);
    assert_eq!(grant_items[0]["name"], "grant");
    assert_eq!(grant_items[0]["kind"], 12);
    assert_eq!(grant_items[0]["uri"], uri);

    let prepare_preview = response_value(server.handle_json(&request(
        3,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 4,
                "character": line(text, 4).find("preview").expect("interface method")
            }
        }),
    )));
    let preview_items = prepare_preview["result"]
        .as_array()
        .expect("prepareCallHierarchy response should be an array");
    assert_eq!(preview_items.len(), 1);
    assert_eq!(preview_items[0]["name"], "preview");
    assert_eq!(preview_items[0]["uri"], uri);

    let incoming_grant = response_value(server.handle_json(&request(
        4,
        "callHierarchy/incomingCalls",
        serde_json::json!({ "item": grant_items[0].clone() }),
    )));
    let grant_incoming = incoming_grant["result"]
        .as_array()
        .expect("incomingCalls response should be an array");
    assert_eq!(grant_incoming.len(), 1);
    assert_eq!(grant_incoming[0]["from"]["name"], "main");
    assert_call_range(
        grant_incoming[0]["fromRanges"]
            .as_array()
            .expect("incoming call should include ranges"),
        8,
        line(text, 8).find("grant").expect("default method call"),
    );

    let outgoing_grant = response_value(server.handle_json(&request(
        5,
        "callHierarchy/outgoingCalls",
        serde_json::json!({ "item": grant_items[0].clone() }),
    )));
    let grant_outgoing = outgoing_grant["result"]
        .as_array()
        .expect("outgoingCalls response should be an array");
    assert_eq!(grant_outgoing.len(), 1);
    assert_eq!(grant_outgoing[0]["to"]["name"], "clamp");
    assert_call_range(
        grant_outgoing[0]["fromRanges"]
            .as_array()
            .expect("outgoing call should include ranges"),
        3,
        line(text, 3).find("clamp").expect("default helper call"),
    );

    let incoming_preview = response_value(server.handle_json(&request(
        6,
        "callHierarchy/incomingCalls",
        serde_json::json!({ "item": preview_items[0].clone() }),
    )));
    let preview_incoming = incoming_preview["result"]
        .as_array()
        .expect("incomingCalls response should be an array");
    assert_eq!(preview_incoming.len(), 1);
    assert_eq!(preview_incoming[0]["from"]["name"], "main");
    assert_call_range(
        preview_incoming[0]["fromRanges"]
            .as_array()
            .expect("incoming call should include ranges"),
        9,
        line(text, 9)
            .find("preview")
            .expect("interface method call"),
    );

    let outgoing_preview = response_value(server.handle_json(&request(
        7,
        "callHierarchy/outgoingCalls",
        serde_json::json!({ "item": preview_items[0].clone() }),
    )));
    let preview_outgoing = outgoing_preview["result"]
        .as_array()
        .expect("outgoingCalls response should be an array");
    assert!(preview_outgoing.is_empty(), "{preview_outgoing:?}");
}

#[test]
fn lsp_call_hierarchy_uses_schema_method_and_trait_method_calls() {
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
pub fn grant() { return 1 }
pub fn preview() { return 1 }";
    let grant_start = schema_text.find("grant").expect("grant marker");
    let preview_start = schema_text.find("preview").expect("preview marker");
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
                "traits": [
                    {
                        "name": "Rewardable",
                        "fact": { "kind": "trait", "name": "Rewardable" }
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
pub fn main(player: Player, rewardable: Rewardable) -> i64 {
    let first = player.grant(1)
    return rewardable.preview(first)
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

    let prepare_preview = response_value(server.handle_json(&request(
        4,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": schema_uri },
            "position": {
                "line": 1,
                "character": line(schema_text, 1).find("preview").expect("preview declaration")
            }
        }),
    )));
    let preview_items = prepare_preview["result"]
        .as_array()
        .expect("prepareCallHierarchy response should be an array");
    assert_eq!(preview_items.len(), 1);
    assert_eq!(preview_items[0]["name"], "preview");
    assert_eq!(preview_items[0]["uri"], schema_uri);

    let incoming_grant = response_value(server.handle_json(&request(
        5,
        "callHierarchy/incomingCalls",
        serde_json::json!({ "item": grant_items[0].clone() }),
    )));
    let grant_incoming = incoming_grant["result"]
        .as_array()
        .expect("incomingCalls response should be an array");
    assert_eq!(grant_incoming.len(), 1);
    assert_eq!(grant_incoming[0]["from"]["name"], "main");
    assert_call_range(
        grant_incoming[0]["fromRanges"]
            .as_array()
            .expect("incoming call should include ranges"),
        1,
        line(text, 1).find("grant").expect("grant call"),
    );

    let incoming_preview = response_value(server.handle_json(&request(
        6,
        "callHierarchy/incomingCalls",
        serde_json::json!({ "item": preview_items[0].clone() }),
    )));
    let preview_incoming = incoming_preview["result"]
        .as_array()
        .expect("incomingCalls response should be an array");
    assert_eq!(preview_incoming.len(), 1);
    assert_eq!(preview_incoming[0]["from"]["name"], "main");
    assert_call_range(
        preview_incoming[0]["fromRanges"]
            .as_array()
            .expect("incoming call should include ranges"),
        2,
        line(text, 2).find("preview").expect("preview call"),
    );

    let prepare_main = response_value(server.handle_json(&request(
        7,
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
        8,
        "callHierarchy/outgoingCalls",
        serde_json::json!({ "item": main_items[0].clone() }),
    )));
    let outgoing_calls = outgoing["result"]
        .as_array()
        .expect("outgoingCalls response should be an array");
    assert_eq!(outgoing_calls.len(), 2, "{outgoing_calls:?}");
    assert_outgoing_call(
        outgoing_calls,
        "grant",
        &schema_uri,
        1,
        line(text, 1).find("grant").expect("grant call"),
    );
    assert_outgoing_call(
        outgoing_calls,
        "preview",
        &schema_uri,
        2,
        line(text, 2).find("preview").expect("preview call"),
    );

    let outgoing_schema = response_value(server.handle_json(&request(
        9,
        "callHierarchy/outgoingCalls",
        serde_json::json!({ "item": grant_items[0].clone() }),
    )));
    let schema_outgoing = outgoing_schema["result"]
        .as_array()
        .expect("outgoingCalls response should be an array");
    assert!(schema_outgoing.is_empty(), "{schema_outgoing:?}");

    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

fn assert_empty_prepare_call_hierarchy(
    server: &mut LspServer,
    id: i64,
    uri: &str,
    line: usize,
    character: usize,
) {
    let response = response_value(server.handle_json(&request(
        id,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": line,
                "character": character
            }
        }),
    )));
    let items = response["result"]
        .as_array()
        .expect("prepareCallHierarchy response should be an array");
    assert!(items.is_empty(), "{items:?}");
}

fn assert_call_range(ranges: &[serde_json::Value], line: usize, character: usize) {
    assert!(
        ranges.iter().any(|range| {
            range["start"]["line"] == line && range["start"]["character"] == character
        }),
        "{ranges:?}"
    );
}

fn assert_outgoing_call(
    calls: &[serde_json::Value],
    name: &str,
    uri: &str,
    line: usize,
    character: usize,
) {
    assert!(
        calls.iter().any(|call| {
            call["to"]["name"] == name
                && call["to"]["uri"] == uri
                && call["fromRanges"].as_array().is_some_and(|ranges| {
                    ranges.iter().any(|range| {
                        range["start"]["line"] == line && range["start"]["character"] == character
                    })
                })
        }),
        "{calls:?}"
    );
}

fn line(text: &str, line: usize) -> &str {
    text.lines().nth(line).expect("line should exist")
}

fn temp_workspace() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "vela_lsp_call_hierarchy_schema_{}_{}",
        std::process::id(),
        nonce
    ));
    fs::create_dir_all(&path).expect("temporary workspace should be creatable");
    path
}

fn file_uri(path: &Path) -> String {
    format!(
        "file:///{}",
        path.to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches('/')
    )
}
