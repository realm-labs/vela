use super::{LspServer, notification, notification_value, request, response_value};

#[test]
fn lsp_inlay_hints_show_parameter_names() {
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
    let text = "pub fn grant(amount: i64, reason: String) -> i64 { return amount }\npub fn main() { return grant(10, \"quest\") }";
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
        "textDocument/inlayHint",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 1, "character": 0 },
                "end": { "line": 1, "character": 80 }
            }
        }),
    )));
    let hints = response["result"]
        .as_array()
        .expect("inlayHint should return an array");

    assert_eq!(hints.len(), 2);
    assert_eq!(hints[0]["position"]["line"], 1);
    assert_eq!(hints[0]["position"]["character"], 29);
    assert_eq!(hints[0]["label"], "amount:");
    assert_eq!(hints[0]["kind"], 2);
    assert_eq!(hints[0]["paddingRight"], true);
    assert_eq!(hints[1]["position"]["character"], 33);
    assert_eq!(hints[1]["label"], "reason:");
}

#[test]
fn lsp_inlay_hints_respect_requested_range() {
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
    let text = "pub fn grant(amount: i64, reason: String) -> i64 { return amount }\npub fn main() { return grant(10, \"quest\") }";
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
        "textDocument/inlayHint",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 1, "character": 31 },
                "end": { "line": 1, "character": 80 }
            }
        }),
    )));

    assert_eq!(
        response["result"],
        serde_json::json!([{
            "position": { "line": 1, "character": 33 },
            "label": "reason:",
            "kind": 2,
            "paddingRight": true
        }])
    );
}
