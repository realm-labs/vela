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

#[test]
fn lsp_document_formatting_formats_declarations() {
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
                "text": "pub struct Player{level:i64 name:String}impl Player{fn heal(amount:i64)->i64{return amount}}"
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
    assert_eq!(
        edits[0]["newText"],
        "\
pub struct Player {
    level: i64
    name: String
}
impl Player {
    fn heal(amount: i64) -> i64 {
        return amount
    }
}
"
    );
}

#[test]
fn lsp_range_formatting_limits_edits_to_range() {
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
                "text": "pub fn main() {   \n    return 1   \n}\n"
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/rangeFormatting",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 1, "character": 0 },
                "end": { "line": 2, "character": 0 }
            },
            "options": { "tabSize": 4, "insertSpaces": true }
        }),
    )));
    let edits = response["result"]
        .as_array()
        .expect("rangeFormatting should return edits");

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0]["range"]["start"]["line"], 1);
    assert_eq!(edits[0]["range"]["start"]["character"], 12);
    assert_eq!(edits[0]["range"]["end"]["line"], 1);
    assert_eq!(edits[0]["range"]["end"]["character"], 15);
    assert_eq!(edits[0]["newText"], "");
}

#[test]
fn lsp_range_formatting_formats_selected_item() {
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
                "text": "pub fn main(){return 1}\n\npub fn other(){return 2}\n"
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/rangeFormatting",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 1, "character": 0 }
            },
            "options": { "tabSize": 4, "insertSpaces": true }
        }),
    )));
    let edits = response["result"]
        .as_array()
        .expect("rangeFormatting should return edits");

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0]["range"]["start"]["line"], 0);
    assert_eq!(edits[0]["range"]["start"]["character"], 0);
    assert_eq!(edits[0]["range"]["end"]["line"], 1);
    assert_eq!(edits[0]["range"]["end"]["character"], 0);
    assert_eq!(edits[0]["newText"], "pub fn main() {\n    return 1\n}\n");
}

#[test]
fn lsp_range_formatting_formats_item_with_leading_blank_selection() {
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
                "text": "\n\npub fn main(){return 1}\n\npub fn other(){return 2}\n"
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/rangeFormatting",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 3, "character": 0 }
            },
            "options": { "tabSize": 4, "insertSpaces": true }
        }),
    )));
    let edits = response["result"]
        .as_array()
        .expect("rangeFormatting should return edits");

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0]["range"]["start"]["line"], 2);
    assert_eq!(edits[0]["range"]["start"]["character"], 0);
    assert_eq!(edits[0]["range"]["end"]["line"], 3);
    assert_eq!(edits[0]["range"]["end"]["character"], 0);
    assert_eq!(edits[0]["newText"], "pub fn main() {\n    return 1\n}\n");
}

#[test]
fn lsp_range_formatting_formats_selected_impl_method() {
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
                "text": "impl Player{fn heal(amount:i64)->i64{return amount}fn hurt(amount:i64)->i64{return amount}}\n"
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/rangeFormatting",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 12 },
                "end": { "line": 0, "character": 51 }
            },
            "options": { "tabSize": 4, "insertSpaces": true }
        }),
    )));
    let edits = response["result"]
        .as_array()
        .expect("rangeFormatting should return edits");

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0]["range"]["start"]["line"], 0);
    assert_eq!(edits[0]["range"]["start"]["character"], 12);
    assert_eq!(edits[0]["range"]["end"]["line"], 0);
    assert_eq!(edits[0]["range"]["end"]["character"], 51);
    assert_eq!(
        edits[0]["newText"],
        "fn heal(amount: i64) -> i64 {\n    return amount\n}\n"
    );
}

#[test]
fn lsp_range_formatting_preserves_nested_method_indent() {
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
                "text": "\
impl Player {
    fn heal(amount:i64)->i64{return amount}
    fn hurt(amount:i64)->i64{return amount}
}
"
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/rangeFormatting",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 1, "character": 4 },
                "end": { "line": 1, "character": 43 }
            },
            "options": { "tabSize": 4, "insertSpaces": true }
        }),
    )));
    let edits = response["result"]
        .as_array()
        .expect("rangeFormatting should return edits");

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0]["range"]["start"]["line"], 1);
    assert_eq!(edits[0]["range"]["start"]["character"], 4);
    assert_eq!(edits[0]["range"]["end"]["line"], 1);
    assert_eq!(edits[0]["range"]["end"]["character"], 43);
    assert_eq!(
        edits[0]["newText"],
        "fn heal(amount: i64) -> i64 {\n        return amount\n    }"
    );
}

#[test]
fn lsp_on_type_formatting_only_edits_current_construct() {
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
                "text": "\
pub fn main() {   
    return 1   
}

pub fn other() {   
    return 2   
}
"
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/onTypeFormatting",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 2, "character": 1 },
            "ch": "}",
            "options": { "tabSize": 4, "insertSpaces": true }
        }),
    )));
    let edits = response["result"]
        .as_array()
        .expect("onTypeFormatting should return edits");

    assert_eq!(edits.len(), 2);
    assert_eq!(edits[0]["range"]["start"]["line"], 0);
    assert_eq!(edits[0]["range"]["start"]["character"], 15);
    assert_eq!(edits[0]["range"]["end"]["line"], 0);
    assert_eq!(edits[0]["range"]["end"]["character"], 18);
    assert_eq!(edits[0]["newText"], "");
    assert_eq!(edits[1]["range"]["start"]["line"], 1);
    assert_eq!(edits[1]["range"]["start"]["character"], 12);
    assert_eq!(edits[1]["range"]["end"]["line"], 1);
    assert_eq!(edits[1]["range"]["end"]["character"], 15);
    assert_eq!(edits[1]["newText"], "");
}

#[test]
fn lsp_on_type_formatting_reflows_completed_item() {
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
                "text": "pub fn main(){return 1}\n\npub fn other(){return 2}\n"
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/onTypeFormatting",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 0, "character": 23 },
            "ch": "}",
            "options": { "tabSize": 4, "insertSpaces": true }
        }),
    )));
    let edits = response["result"]
        .as_array()
        .expect("onTypeFormatting should return edits");

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0]["range"]["start"]["line"], 0);
    assert_eq!(edits[0]["range"]["start"]["character"], 0);
    assert_eq!(edits[0]["range"]["end"]["line"], 1);
    assert_eq!(edits[0]["range"]["end"]["character"], 0);
    assert_eq!(edits[0]["newText"], "pub fn main() {\n    return 1\n}\n");
}
