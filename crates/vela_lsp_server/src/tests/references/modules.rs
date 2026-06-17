use crate::tests::{LspServer, notification, notification_value, request, response_value};

use super::{assert_highlight, assert_reference, line};

#[test]
fn lsp_references_find_imported_module_segments() {
    let mut server = LspServer::new();
    let _ = response_value(server.handle_json(&request(
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": "file:///workspace/scripts",
            "capabilities": {}
        }),
    )));
    let main_text = "\
use game::reward::grant
pub fn main() -> i64 { return grant() }";
    let other_text = "\
use game::reward::bonus
pub fn other() -> i64 { return bonus() }";
    let helper_text = "pub fn grant() -> i64 { return 1 }\npub fn bonus() -> i64 { return 2 }";
    let main_uri = "file:///workspace/scripts/game/main.vela";
    let other_uri = "file:///workspace/scripts/game/other.vela";
    let helper_uri = "file:///workspace/scripts/game/reward.vela";
    for (uri, text) in [
        (helper_uri, helper_text),
        (other_uri, other_text),
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

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 0,
                "character": line(main_text, 0).find("reward").expect("module segment")
            },
            "context": { "includeDeclaration": true }
        }),
    )));
    let references = response["result"]
        .as_array()
        .expect("references response should be an array");

    assert_eq!(references.len(), 2, "{references:?}");
    assert_reference(
        references,
        main_uri,
        0,
        line(main_text, 0)
            .find("reward")
            .expect("first module segment"),
    );
    assert_reference(
        references,
        other_uri,
        0,
        line(other_text, 0)
            .find("reward")
            .expect("second module segment"),
    );
}

#[test]
fn lsp_document_highlight_marks_imported_module_segments() {
    let mut server = LspServer::new();
    let _ = response_value(server.handle_json(&request(
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": "file:///workspace/scripts",
            "capabilities": {}
        }),
    )));
    let text = "\
use game::reward::grant
use game::reward::bonus
pub fn main() -> i64 {
    return grant() + bonus()
}";
    let helper_text = "pub fn grant() -> i64 { return 1 }\npub fn bonus() -> i64 { return 2 }";
    let uri = "file:///workspace/scripts/game/main.vela";
    let helper_uri = "file:///workspace/scripts/game/reward.vela";
    for (uri, text) in [(helper_uri, helper_text), (uri, text)] {
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

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/documentHighlight",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 0,
                "character": line(text, 0).find("reward").expect("module segment")
            }
        }),
    )));
    let highlights = response["result"]
        .as_array()
        .expect("documentHighlight response should be an array");

    assert_eq!(highlights.len(), 2, "{highlights:?}");
    assert_highlight(
        highlights,
        0,
        line(text, 0).find("reward").expect("first module segment"),
        1,
    );
    assert_highlight(
        highlights,
        1,
        line(text, 1).find("reward").expect("second module segment"),
        1,
    );
}
