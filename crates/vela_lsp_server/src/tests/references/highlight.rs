use crate::tests::{LspServer, notification, notification_value, request, response_value};

use super::{assert_highlight, assert_reference, line};

#[test]
fn lsp_document_highlight_returns_empty_for_dynamic_and_unresolved_targets() {
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
        initialize["result"]["capabilities"]["documentHighlightProvider"],
        true
    );

    let text = "\
pub fn unresolved() { return missing }
pub fn dynamic(value: Any) { return value.level }";
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

    assert_empty_highlights(
        &mut server,
        2,
        uri,
        0,
        line(text, 0)
            .find("missing")
            .expect("unresolved name should exist"),
    );
    assert_empty_highlights(
        &mut server,
        3,
        uri,
        1,
        line(text, 1)
            .find("level")
            .expect("dynamic member should exist"),
    );
}

#[test]
fn lsp_document_highlight_imported_symbol_stays_in_active_document() {
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
        initialize["result"]["capabilities"]["documentHighlightProvider"],
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
    for (uri, text) in [(helper_uri, helper_text), (main_uri, main_text)] {
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

    let references = response_value(server.handle_json(&request(
        2,
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 2,
                "character": line(main_text, 2).find("grant").expect("grant call")
            },
            "context": { "includeDeclaration": true }
        }),
    )));
    let reference_items = references["result"]
        .as_array()
        .expect("references response should be an array");
    assert_eq!(reference_items.len(), 4, "{reference_items:?}");
    assert_reference(
        reference_items,
        helper_uri,
        0,
        helper_text.find("grant").expect("helper declaration"),
    );
    assert_reference(
        reference_items,
        main_uri,
        0,
        line(main_text, 0).find("grant").expect("import"),
    );
    assert_reference(
        reference_items,
        main_uri,
        2,
        line(main_text, 2).find("grant").expect("first call"),
    );
    assert_reference(
        reference_items,
        main_uri,
        3,
        line(main_text, 3).find("grant").expect("second call"),
    );

    let highlights = response_value(server.handle_json(&request(
        3,
        "textDocument/documentHighlight",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 2,
                "character": line(main_text, 2).find("grant").expect("grant call")
            }
        }),
    )));
    let highlight_items = highlights["result"]
        .as_array()
        .expect("documentHighlight response should be an array");
    assert_eq!(highlight_items.len(), 3, "{highlight_items:?}");
    assert_highlight(
        highlight_items,
        0,
        line(main_text, 0).find("grant").expect("import"),
        1,
    );
    assert_highlight(
        highlight_items,
        2,
        line(main_text, 2).find("grant").expect("first call"),
        1,
    );
    assert_highlight(
        highlight_items,
        3,
        line(main_text, 3).find("grant").expect("second call"),
        1,
    );
    assert!(
        highlight_items.iter().all(|highlight| {
            highlight["range"]["start"]["line"] != 0
                || highlight["range"]["start"]["character"]
                    != helper_text.find("grant").expect("helper declaration")
        }),
        "{highlight_items:?}"
    );
}

#[test]
fn lsp_document_highlight_imported_const_and_global_stays_in_active_document() {
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
        initialize["result"]["capabilities"]["documentHighlightProvider"],
        true
    );
    let main_text = "\
use game::rewards::BASE_REWARD
use game::rewards::reward_scale
pub fn main() -> i64 {
    let first = BASE_REWARD
    return first + reward_scale
}";
    let rewards_text = "\
pub const BASE_REWARD = 4
pub global reward_scale: i64";
    let main_uri = "file:///workspace/scripts/game/main.vela";
    let rewards_uri = "file:///workspace/scripts/game/rewards.vela";
    for (uri, text) in [(rewards_uri, rewards_text), (main_uri, main_text)] {
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

    let const_references = response_value(server.handle_json(&request(
        2,
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 3,
                "character": line(main_text, 3)
                    .find("BASE_REWARD")
                    .expect("const use should exist")
            },
            "context": { "includeDeclaration": true }
        }),
    )));
    let const_reference_items = const_references["result"]
        .as_array()
        .expect("references response should be an array");
    assert_eq!(const_reference_items.len(), 3, "{const_reference_items:?}");
    assert_reference(
        const_reference_items,
        rewards_uri,
        0,
        line(rewards_text, 0)
            .find("BASE_REWARD")
            .expect("const declaration should exist"),
    );

    let const_highlights = response_value(server.handle_json(&request(
        3,
        "textDocument/documentHighlight",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 3,
                "character": line(main_text, 3)
                    .find("BASE_REWARD")
                    .expect("const use should exist")
            }
        }),
    )));
    let const_highlight_items = const_highlights["result"]
        .as_array()
        .expect("documentHighlight response should be an array");
    assert_eq!(const_highlight_items.len(), 2, "{const_highlight_items:?}");
    assert_highlight(
        const_highlight_items,
        0,
        line(main_text, 0)
            .find("BASE_REWARD")
            .expect("const import should exist"),
        1,
    );
    assert_highlight(
        const_highlight_items,
        3,
        line(main_text, 3)
            .find("BASE_REWARD")
            .expect("const use should exist"),
        2,
    );
    assert_no_highlight(
        const_highlight_items,
        0,
        line(rewards_text, 0)
            .find("BASE_REWARD")
            .expect("const declaration should exist"),
    );

    let global_references = response_value(server.handle_json(&request(
        4,
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 4,
                "character": line(main_text, 4)
                    .find("reward_scale")
                    .expect("global use should exist")
            },
            "context": { "includeDeclaration": true }
        }),
    )));
    let global_reference_items = global_references["result"]
        .as_array()
        .expect("references response should be an array");
    assert_eq!(
        global_reference_items.len(),
        3,
        "{global_reference_items:?}"
    );
    assert_reference(
        global_reference_items,
        rewards_uri,
        1,
        line(rewards_text, 1)
            .find("reward_scale")
            .expect("global declaration should exist"),
    );

    let global_highlights = response_value(server.handle_json(&request(
        5,
        "textDocument/documentHighlight",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 4,
                "character": line(main_text, 4)
                    .find("reward_scale")
                    .expect("global use should exist")
            }
        }),
    )));
    let global_highlight_items = global_highlights["result"]
        .as_array()
        .expect("documentHighlight response should be an array");
    assert_eq!(
        global_highlight_items.len(),
        2,
        "{global_highlight_items:?}"
    );
    assert_highlight(
        global_highlight_items,
        1,
        line(main_text, 1)
            .find("reward_scale")
            .expect("global import should exist"),
        1,
    );
    assert_highlight(
        global_highlight_items,
        4,
        line(main_text, 4)
            .find("reward_scale")
            .expect("global use should exist"),
        2,
    );
    assert_no_highlight(
        global_highlight_items,
        1,
        line(rewards_text, 1)
            .find("reward_scale")
            .expect("global declaration should exist"),
    );
}

fn assert_empty_highlights(
    server: &mut LspServer,
    id: i64,
    uri: &str,
    line: usize,
    character: usize,
) {
    let response = response_value(server.handle_json(&request(
        id,
        "textDocument/documentHighlight",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": line,
                "character": character
            }
        }),
    )));
    let highlights = response["result"]
        .as_array()
        .expect("documentHighlight response should be an array");
    assert!(highlights.is_empty(), "{highlights:?}");
}

fn assert_no_highlight(highlights: &[serde_json::Value], line: usize, character: usize) {
    assert!(
        highlights.iter().all(|highlight| {
            highlight["range"]["start"]["line"] != line
                || highlight["range"]["start"]["character"] != character
        }),
        "{highlights:?}"
    );
}
