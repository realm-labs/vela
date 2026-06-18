use crate::tests::{LspServer, notification, notification_value, request, response_value};

use super::line;

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
