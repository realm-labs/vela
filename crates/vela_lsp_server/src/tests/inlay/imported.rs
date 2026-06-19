use super::super::{LspServer, notification, notification_value, request, response_value};

#[test]
fn lsp_inlay_hints_show_imported_function_parameter_names() {
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
    open_document(
        &mut server,
        "file:///workspace/scripts/game/reward.vela",
        "pub fn grant(amount: i64, reason: String) -> i64 { return amount }",
    );
    let uri = "file:///workspace/scripts/game/main.vela";
    let text = "use game::reward::grant\npub fn main() { return grant(10, \"quest\") }";
    open_document(&mut server, uri, text);

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
    let main_line = text.lines().nth(1).expect("main line should exist");

    assert_eq!(
        response["result"],
        serde_json::json!([
            {
                "position": { "line": 1, "character": main_line.find("10").expect("first arg") },
                "label": "amount:",
                "kind": 2,
                "paddingRight": true
            },
            {
                "position": { "line": 1, "character": main_line.find("\"quest\"").expect("second arg") },
                "label": "reason:",
                "kind": 2,
                "paddingRight": true
            }
        ])
    );
}

fn open_document(server: &mut LspServer, uri: &str, text: &str) {
    let diagnostics = notification_value(server.handle_json(&notification(
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
    assert_eq!(diagnostics["method"], "textDocument/publishDiagnostics");
    assert_eq!(diagnostics["params"]["uri"], uri);
    assert_eq!(diagnostics["params"]["diagnostics"], serde_json::json!([]));
}
