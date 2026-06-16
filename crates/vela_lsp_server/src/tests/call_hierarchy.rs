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

fn assert_call_range(ranges: &[serde_json::Value], line: usize, character: usize) {
    assert!(
        ranges.iter().any(|range| {
            range["start"]["line"] == line && range["start"]["character"] == character
        }),
        "{ranges:?}"
    );
}

fn line(text: &str, line: usize) -> &str {
    text.lines().nth(line).expect("line should exist")
}
