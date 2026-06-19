use crate::tests::{
    LspServer, handle_notification, handle_request, notification_value, response_value,
};

use super::{assert_call_range, line};

#[test]
fn lsp_call_hierarchy_uses_imported_function_alias_calls() {
    let mut server = LspServer::new();
    let initialize = response_value(handle_request(
        &mut server,
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": "file:///workspace/scripts",
            "capabilities": {}
        }),
    ));
    assert_eq!(
        initialize["result"]["capabilities"]["callHierarchyProvider"],
        true
    );
    let main_text = "\
use game::reward::grant as award
pub fn main(amount: i64) -> i64 {
    let first = award(amount)
    return award(first)
}";
    let helper_text = "pub fn grant(amount: i64) -> i64 { return amount }";
    let main_uri = "file:///workspace/scripts/game/main.vela";
    let helper_uri = "file:///workspace/scripts/game/reward.vela";
    for (uri, text) in [(helper_uri, helper_text), (main_uri, main_text)] {
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
    }

    let prepare_grant = response_value(handle_request(
        &mut server,
        2,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": helper_uri },
            "position": {
                "line": 0,
                "character": helper_text.find("grant").expect("grant declaration")
            }
        }),
    ));
    let grant_items = prepare_grant["result"]
        .as_array()
        .expect("prepareCallHierarchy response should be an array");
    assert_eq!(grant_items.len(), 1);
    assert_eq!(grant_items[0]["name"], "grant");
    assert_eq!(grant_items[0]["kind"], 12);
    assert_eq!(grant_items[0]["uri"], helper_uri);

    let prepare_from_import_alias = response_value(handle_request(
        &mut server,
        3,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 0,
                "character": line(main_text, 0)
                    .find("award")
                    .expect("import alias should exist")
            }
        }),
    ));
    let import_alias_items = prepare_from_import_alias["result"]
        .as_array()
        .expect("prepareCallHierarchy response should be an array");
    assert_eq!(import_alias_items, grant_items);

    let prepare_from_import_path = response_value(handle_request(
        &mut server,
        4,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 0,
                "character": line(main_text, 0)
                    .find("grant")
                    .expect("import path function should exist")
            }
        }),
    ));
    let import_path_items = prepare_from_import_path["result"]
        .as_array()
        .expect("prepareCallHierarchy response should be an array");
    assert_eq!(import_path_items, grant_items);

    let prepare_from_alias_call = response_value(handle_request(
        &mut server,
        5,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 2,
                "character": line(main_text, 2)
                    .find("award")
                    .expect("first alias call")
            }
        }),
    ));
    let alias_items = prepare_from_alias_call["result"]
        .as_array()
        .expect("prepareCallHierarchy response should be an array");
    assert_eq!(alias_items, grant_items);

    let incoming = response_value(handle_request(
        &mut server,
        6,
        "callHierarchy/incomingCalls",
        serde_json::json!({ "item": grant_items[0].clone() }),
    ));
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
        line(main_text, 2).find("award").expect("first alias call"),
    );
    assert_call_range(
        incoming_calls[0]["fromRanges"]
            .as_array()
            .expect("incoming call should include ranges"),
        3,
        line(main_text, 3).find("award").expect("second alias call"),
    );

    let prepare_main = response_value(handle_request(
        &mut server,
        7,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 1,
                "character": line(main_text, 1).find("main").expect("main declaration")
            }
        }),
    ));
    let main_items = prepare_main["result"]
        .as_array()
        .expect("prepareCallHierarchy response should be an array");
    assert_eq!(main_items.len(), 1);

    let outgoing = response_value(handle_request(
        &mut server,
        8,
        "callHierarchy/outgoingCalls",
        serde_json::json!({ "item": main_items[0].clone() }),
    ));
    let outgoing_calls = outgoing["result"]
        .as_array()
        .expect("outgoingCalls response should be an array");
    assert_eq!(outgoing_calls.len(), 1);
    assert_eq!(outgoing_calls[0]["to"]["name"], "grant");
    assert_eq!(outgoing_calls[0]["to"]["uri"], helper_uri);
    let outgoing_ranges = outgoing_calls[0]["fromRanges"]
        .as_array()
        .expect("outgoing call should include ranges");
    assert_eq!(outgoing_ranges.len(), 2);
    assert_call_range(
        outgoing_ranges,
        2,
        line(main_text, 2).find("award").expect("first alias call"),
    );
    assert_call_range(
        outgoing_ranges,
        3,
        line(main_text, 3).find("award").expect("second alias call"),
    );
}
