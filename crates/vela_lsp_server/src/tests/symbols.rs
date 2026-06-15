use super::{LspServer, notification, notification_value, request, response_value};

#[test]
fn lsp_document_symbols_include_nested_script_members() {
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
pub struct Player {
    level: i64
}
pub enum Reward {
    Coins(amount: i64)
}
pub fn main(amount: i64) -> i64 { return amount }";
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
        "textDocument/documentSymbol",
        serde_json::json!({
            "textDocument": { "uri": "file:///workspace/scripts/game/main.vela" }
        }),
    )));

    let symbols = response["result"]
        .as_array()
        .expect("documentSymbol should return an array");
    assert_eq!(symbols.len(), 3, "{symbols:?}");
    assert_eq!(symbols[0]["name"], "Player");
    assert_eq!(symbols[0]["kind"], 23);
    assert_eq!(symbols[0]["children"][0]["name"], "level");
    assert_eq!(symbols[0]["children"][0]["kind"], 8);
    assert_eq!(
        symbols[0]["children"][0]["selectionRange"]["start"]["line"],
        1
    );
    assert_eq!(symbols[1]["name"], "Reward");
    assert_eq!(symbols[1]["children"][0]["name"], "Coins");
    assert_eq!(symbols[1]["children"][0]["kind"], 22);
    assert_eq!(symbols[1]["children"][0]["children"][0]["name"], "amount");
    assert_eq!(symbols[2]["name"], "main");
    assert_eq!(symbols[2]["kind"], 12);
    assert_eq!(symbols[2]["detail"], "(amount: i64) -> i64");
}
