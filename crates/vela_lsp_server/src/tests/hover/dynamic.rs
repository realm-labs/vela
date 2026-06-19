use super::super::{LspServer, notification, notification_value, request, response_value};

#[test]
fn lsp_hover_returns_null_for_source_any_return_receiver_member() {
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
    let text = "\
struct Player { level: i64 }
fn source_any() -> Any { return Player { level: 1 } }
pub fn main() { return source_any().level }";
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

    let use_line = text.lines().nth(2).expect("member use line should exist");
    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 2,
                "character": use_line.find("level").expect("member use")
            }
        }),
    )));

    assert!(response["result"].is_null(), "{response:?}");
}
