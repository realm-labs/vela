use super::{LspServer, notification, notification_value, request, response_value};

#[test]
fn lsp_hover_reports_open_overlay_parameter_fact() {
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
    let text = "pub fn main(amount: i64) -> i64 { return amount }";
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": "file:///workspace/scripts/game/main.vela",
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": "file:///workspace/scripts/game/main.vela" },
            "position": {
                "line": 0,
                "character": text.rfind("amount").unwrap_or_else(|| {
                    panic!("hover fixture should contain amount use")
                })
            }
        }),
    )));

    assert_eq!(response["result"]["range"]["start"]["line"], 0);
    assert_eq!(
        response["result"]["contents"]["kind"],
        serde_json::json!("markdown")
    );
    let value = response["result"]["contents"]["value"]
        .as_str()
        .expect("hover contents should be markdown");
    assert!(value.contains("amount"), "{value}");
    assert!(value.contains("_parameter_: i64"), "{value}");
}
