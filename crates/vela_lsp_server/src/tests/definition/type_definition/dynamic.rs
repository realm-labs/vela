use super::*;

#[test]
fn lsp_type_definition_returns_null_for_dynamic_receiver_member() {
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
    let text = r#"fn main(value: Any) {
    return value.level;
}"#;
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
    let member_line = text.lines().nth(1).expect("member use line should exist");

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/typeDefinition",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 1,
                "character": member_line
                    .find("level")
                    .expect("dynamic member should contain name")
            }
        }),
    )));

    assert!(response["result"].is_null(), "{response:?}");
}
