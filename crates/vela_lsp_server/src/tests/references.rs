use super::{LspServer, notification, notification_value, request, response_value};

#[test]
fn lsp_references_find_local_binding_uses() {
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
pub fn main(amount: i64) -> i64 {
    let next = amount + 1
    return next + amount
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

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 2,
                "character": line(text, 2).find("amount").expect("amount use")
            },
            "context": { "includeDeclaration": true }
        }),
    )));
    let references = response["result"]
        .as_array()
        .expect("references response should be an array");

    assert_eq!(references.len(), 3);
    assert_reference(
        references,
        uri,
        0,
        line(text, 0).find("amount").expect("parameter declaration"),
    );
    assert_reference(
        references,
        uri,
        1,
        line(text, 1).find("amount").expect("first read"),
    );
    assert_reference(
        references,
        uri,
        2,
        line(text, 2).find("amount").expect("second read"),
    );
}

fn assert_reference(references: &[serde_json::Value], uri: &str, line: usize, character: usize) {
    assert!(
        references.iter().any(|reference| {
            reference["uri"] == uri
                && reference["range"]["start"]["line"] == line
                && reference["range"]["start"]["character"] == character
        }),
        "{references:?}"
    );
}

fn line(text: &str, line: usize) -> &str {
    text.lines().nth(line).expect("line should exist")
}
