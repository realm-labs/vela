use crate::tests::{LspServer, notification, notification_value, request, response_value};

use super::{assert_call_range, line};

#[test]
fn lsp_call_hierarchy_cross_file_trait_impl_method_calls() {
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
    let main_uri = "file:///workspace/scripts/game/main.vela";
    let math_uri = "file:///workspace/scripts/game/math.vela";
    let types_uri = "file:///workspace/scripts/game/types.vela";
    let main_text = "\
use game::types::Player
pub fn first(player: Player) -> i64 {
    return player.grant(1)
}

pub fn second(player: Player) -> i64 {
    return player.grant(2)
}";
    let math_text = "pub fn clamp(value: i64) -> i64 { return value }";
    let types_text = "\
use game::math::clamp
pub trait Rewardable {
    fn grant(self, amount: i64) -> i64;
}

pub struct Player { level: i64 }

impl Rewardable for Player {
    fn grant(self, amount: i64) -> i64 { return clamp(amount) }
}";
    for (uri, text) in [
        (math_uri, math_text),
        (types_uri, types_text),
        (main_uri, main_text),
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

    let prepare_grant = response_value(server.handle_json(&request(
        2,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": types_uri },
            "position": {
                "line": 8,
                "character": line(types_text, 8)
                    .find("grant")
                    .expect("trait impl method declaration")
            }
        }),
    )));
    let grant_items = prepare_grant["result"]
        .as_array()
        .expect("prepareCallHierarchy response should be an array");
    assert_eq!(grant_items.len(), 1);
    assert_eq!(grant_items[0]["name"], "grant");
    assert_eq!(grant_items[0]["kind"], 12);
    assert_eq!(grant_items[0]["uri"], types_uri);

    let prepare_from_call = response_value(server.handle_json(&request(
        3,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 2,
                "character": line(main_text, 2).find("grant").expect("method call")
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
    assert_eq!(incoming_calls.len(), 2, "{incoming_calls:?}");
    assert_eq!(incoming_calls[0]["from"]["name"], "first");
    assert_eq!(incoming_calls[0]["from"]["uri"], main_uri);
    assert_call_range(
        incoming_calls[0]["fromRanges"]
            .as_array()
            .expect("incoming call should include ranges"),
        2,
        line(main_text, 2).find("grant").expect("first method call"),
    );
    assert_eq!(incoming_calls[1]["from"]["name"], "second");
    assert_eq!(incoming_calls[1]["from"]["uri"], main_uri);
    assert_call_range(
        incoming_calls[1]["fromRanges"]
            .as_array()
            .expect("incoming call should include ranges"),
        6,
        line(main_text, 6)
            .find("grant")
            .expect("second method call"),
    );

    let prepare_first = response_value(server.handle_json(&request(
        5,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 1,
                "character": line(main_text, 1).find("first").expect("first declaration")
            }
        }),
    )));
    let first_items = prepare_first["result"]
        .as_array()
        .expect("prepareCallHierarchy response should be an array");
    assert_eq!(first_items.len(), 1);

    let first_outgoing = response_value(server.handle_json(&request(
        6,
        "callHierarchy/outgoingCalls",
        serde_json::json!({ "item": first_items[0].clone() }),
    )));
    let first_calls = first_outgoing["result"]
        .as_array()
        .expect("outgoingCalls response should be an array");
    assert_eq!(first_calls.len(), 1);
    assert_eq!(first_calls[0]["to"]["name"], "grant");
    assert_eq!(first_calls[0]["to"]["uri"], types_uri);
    assert_call_range(
        first_calls[0]["fromRanges"]
            .as_array()
            .expect("outgoing call should include ranges"),
        2,
        line(main_text, 2).find("grant").expect("first method call"),
    );

    let method_outgoing = response_value(server.handle_json(&request(
        7,
        "callHierarchy/outgoingCalls",
        serde_json::json!({ "item": grant_items[0].clone() }),
    )));
    let method_calls = method_outgoing["result"]
        .as_array()
        .expect("outgoingCalls response should be an array");
    assert_eq!(method_calls.len(), 1);
    assert_eq!(method_calls[0]["to"]["name"], "clamp");
    assert_eq!(method_calls[0]["to"]["uri"], math_uri);
    assert_call_range(
        method_calls[0]["fromRanges"]
            .as_array()
            .expect("outgoing call should include ranges"),
        8,
        line(types_text, 8)
            .find("clamp")
            .expect("imported helper call"),
    );
}

#[test]
fn lsp_call_hierarchy_cross_file_trait_default_and_interface_methods() {
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
    let main_uri = "file:///workspace/scripts/game/main.vela";
    let math_uri = "file:///workspace/scripts/game/math.vela";
    let traits_uri = "file:///workspace/scripts/game/traits.vela";
    let main_text = "\
use game::traits::Rewardable
pub fn main(rewardable: Rewardable) -> i64 {
    let first = rewardable.grant(1)
    return rewardable.preview(first)
}";
    let math_text = "pub fn clamp(value: i64) -> i64 { return value }";
    let traits_text = "\
use game::math::clamp
pub trait Rewardable {
    fn grant(self, amount: i64) -> i64 { return clamp(amount) }
    fn preview(self, amount: i64) -> i64;
}";
    for (uri, text) in [
        (math_uri, math_text),
        (traits_uri, traits_text),
        (main_uri, main_text),
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

    let prepare_grant = response_value(server.handle_json(&request(
        2,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": traits_uri },
            "position": {
                "line": 2,
                "character": line(traits_text, 2)
                    .find("grant")
                    .expect("trait default method declaration")
            }
        }),
    )));
    let grant_items = prepare_grant["result"]
        .as_array()
        .expect("prepareCallHierarchy response should be an array");
    assert_eq!(grant_items.len(), 1);
    assert_eq!(grant_items[0]["name"], "grant");
    assert_eq!(grant_items[0]["kind"], 12);
    assert_eq!(grant_items[0]["uri"], traits_uri);

    let prepare_grant_call = response_value(server.handle_json(&request(
        3,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 2,
                "character": line(main_text, 2).find("grant").expect("grant call")
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
            "textDocument": { "uri": traits_uri },
            "position": {
                "line": 3,
                "character": line(traits_text, 3)
                    .find("preview")
                    .expect("trait interface method declaration")
            }
        }),
    )));
    let preview_items = prepare_preview["result"]
        .as_array()
        .expect("prepareCallHierarchy response should be an array");
    assert_eq!(preview_items.len(), 1);
    assert_eq!(preview_items[0]["name"], "preview");
    assert_eq!(preview_items[0]["kind"], 12);
    assert_eq!(preview_items[0]["uri"], traits_uri);

    let prepare_preview_call = response_value(server.handle_json(&request(
        5,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 3,
                "character": line(main_text, 3).find("preview").expect("preview call")
            }
        }),
    )));
    assert_eq!(
        prepare_preview_call["result"]
            .as_array()
            .expect("prepareCallHierarchy response should be an array"),
        preview_items
    );

    let incoming_grant = response_value(server.handle_json(&request(
        6,
        "callHierarchy/incomingCalls",
        serde_json::json!({ "item": grant_items[0].clone() }),
    )));
    let grant_incoming = incoming_grant["result"]
        .as_array()
        .expect("incomingCalls response should be an array");
    assert_eq!(grant_incoming.len(), 1);
    assert_eq!(grant_incoming[0]["from"]["name"], "main");
    assert_eq!(grant_incoming[0]["from"]["uri"], main_uri);
    assert_call_range(
        grant_incoming[0]["fromRanges"]
            .as_array()
            .expect("incoming call should include ranges"),
        2,
        line(main_text, 2).find("grant").expect("grant call"),
    );

    let incoming_preview = response_value(server.handle_json(&request(
        7,
        "callHierarchy/incomingCalls",
        serde_json::json!({ "item": preview_items[0].clone() }),
    )));
    let preview_incoming = incoming_preview["result"]
        .as_array()
        .expect("incomingCalls response should be an array");
    assert_eq!(preview_incoming.len(), 1);
    assert_eq!(preview_incoming[0]["from"]["name"], "main");
    assert_eq!(preview_incoming[0]["from"]["uri"], main_uri);
    assert_call_range(
        preview_incoming[0]["fromRanges"]
            .as_array()
            .expect("incoming call should include ranges"),
        3,
        line(main_text, 3).find("preview").expect("preview call"),
    );

    let prepare_main = response_value(server.handle_json(&request(
        8,
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

    let main_outgoing = response_value(server.handle_json(&request(
        9,
        "callHierarchy/outgoingCalls",
        serde_json::json!({ "item": main_items[0].clone() }),
    )));
    let main_calls = main_outgoing["result"]
        .as_array()
        .expect("outgoingCalls response should be an array");
    assert_eq!(main_calls.len(), 2, "{main_calls:?}");
    assert_outgoing_call(
        main_calls,
        "grant",
        traits_uri,
        2,
        line(main_text, 2).find("grant").expect("grant call"),
    );
    assert_outgoing_call(
        main_calls,
        "preview",
        traits_uri,
        3,
        line(main_text, 3).find("preview").expect("preview call"),
    );

    let grant_outgoing = response_value(server.handle_json(&request(
        10,
        "callHierarchy/outgoingCalls",
        serde_json::json!({ "item": grant_items[0].clone() }),
    )));
    let grant_calls = grant_outgoing["result"]
        .as_array()
        .expect("outgoingCalls response should be an array");
    assert_eq!(grant_calls.len(), 1);
    assert_eq!(grant_calls[0]["to"]["name"], "clamp");
    assert_eq!(grant_calls[0]["to"]["uri"], math_uri);
    assert_call_range(
        grant_calls[0]["fromRanges"]
            .as_array()
            .expect("outgoing call should include ranges"),
        2,
        line(traits_text, 2)
            .find("clamp")
            .expect("imported helper call"),
    );

    let preview_outgoing = response_value(server.handle_json(&request(
        11,
        "callHierarchy/outgoingCalls",
        serde_json::json!({ "item": preview_items[0].clone() }),
    )));
    let preview_calls = preview_outgoing["result"]
        .as_array()
        .expect("outgoingCalls response should be an array");
    assert!(preview_calls.is_empty(), "{preview_calls:?}");
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
