use crate::tests::{
    LspServer, handle_notification, handle_request, notification_value, response_value,
};

#[test]
fn lsp_signature_help_returns_null_for_source_any_return_receiver_call() {
    let mut server = LspServer::new();
    let _ = response_value(handle_request(
        &mut server,
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": "file:///workspace/scripts",
            "capabilities": {}
        }),
    ));
    let uri = "file:///workspace/scripts/game/main.vela";
    let text = "\
struct Player { level: i64 }
impl Player {
    fn grant(self, amount: i64, bonus: i64) -> i64 { return amount + bonus }
}
fn source_any() -> Any { return Player { level: 1 } }
pub fn main() { source_any().grant(1, 2) }";
    let _ = notification_value(handle_notification(
        &mut server,
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    ));

    let call_line = text.lines().nth(5).expect("call line should exist");
    let response = response_value(handle_request(
        &mut server,
        2,
        "textDocument/signatureHelp",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 5,
                "character": call_line.find("2)").expect("second argument")
            }
        }),
    ));

    assert!(response["result"].is_null(), "{response:?}");
}
