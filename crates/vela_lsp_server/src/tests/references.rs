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

#[test]
fn lsp_references_find_imported_function_uses() {
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

    let response = response_value(server.handle_json(&request(
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
    let references = response["result"]
        .as_array()
        .expect("references response should be an array");

    assert_eq!(references.len(), 4);
    assert_reference(
        references,
        helper_uri,
        0,
        helper_text.find("grant").expect("function declaration"),
    );
    assert_reference(
        references,
        main_uri,
        0,
        line(main_text, 0).find("grant").expect("import"),
    );
    assert_reference(
        references,
        main_uri,
        2,
        line(main_text, 2).find("grant").expect("first call"),
    );
    assert_reference(
        references,
        main_uri,
        3,
        line(main_text, 3).find("grant").expect("second call"),
    );
}

#[test]
fn lsp_document_highlight_marks_local_declaration_and_reads() {
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
        "textDocument/documentHighlight",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 2,
                "character": line(text, 2).find("amount").expect("amount use")
            }
        }),
    )));
    let highlights = response["result"]
        .as_array()
        .expect("documentHighlight response should be an array");

    assert_eq!(highlights.len(), 3);
    assert_highlight(
        highlights,
        0,
        line(text, 0).find("amount").expect("parameter declaration"),
        1,
    );
    assert_highlight(
        highlights,
        1,
        line(text, 1).find("amount").expect("first read"),
        2,
    );
    assert_highlight(
        highlights,
        2,
        line(text, 2).find("amount").expect("second read"),
        2,
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

fn assert_highlight(highlights: &[serde_json::Value], line: usize, character: usize, kind: u8) {
    assert!(
        highlights.iter().any(|highlight| {
            highlight["range"]["start"]["line"] == line
                && highlight["range"]["start"]["character"] == character
                && highlight["kind"] == kind
        }),
        "{highlights:?}"
    );
}

fn line(text: &str, line: usize) -> &str {
    text.lines().nth(line).expect("line should exist")
}
