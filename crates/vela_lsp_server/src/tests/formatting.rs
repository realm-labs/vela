use super::{LspServer, notification, notification_value, request, response_value};

#[test]
fn lsp_document_formatting_returns_full_document_edit() {
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
    let uri = "file:///workspace/scripts/game/main.vela";
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": "pub fn main() {   \n    return 1\t\n}"
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/formatting",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "options": { "tabSize": 4, "insertSpaces": true }
        }),
    )));
    let edits = response["result"]
        .as_array()
        .expect("formatting should return edits");

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0]["range"]["start"]["line"], 0);
    assert_eq!(edits[0]["range"]["start"]["character"], 0);
    assert_eq!(edits[0]["range"]["end"]["line"], 2);
    assert_eq!(edits[0]["range"]["end"]["character"], 1);
    assert_eq!(edits[0]["newText"], "pub fn main() {\n    return 1\n}\n");
}

#[test]
fn lsp_document_formatting_returns_empty_edits_when_idempotent() {
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
    let uri = "file:///workspace/scripts/game/main.vela";
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": "pub fn main() {\n    return 1\n}\n"
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/formatting",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "options": { "tabSize": 4, "insertSpaces": true }
        }),
    )));

    assert_eq!(response["result"], serde_json::json!([]));
}
