use super::{LspServer, notification, notification_value, request, response_value};

#[test]
fn lsp_rename_rejects_module_declaration_collision() {
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
pub fn grant(amount: i64) -> i64 { return amount }
pub fn award(amount: i64) -> i64 { return amount + 1 }";
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

    let prepare = response_value(server.handle_json(&request(
        2,
        "textDocument/prepareRename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 0,
                "character": line(text, 0).find("grant").expect("grant declaration")
            }
        }),
    )));
    assert_eq!(prepare["result"]["placeholder"], "grant");

    let rename = response_value(server.handle_json(&request(
        3,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 0,
                "character": line(text, 0).find("grant").expect("grant declaration")
            },
            "newName": "award"
        }),
    )));
    assert_eq!(rename["result"], serde_json::Value::Null);
}

#[test]
fn lsp_rename_rejects_import_alias_collision() {
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
use game::bonus::score as award
pub fn main() -> i64 {
    return grant() + award()
}";
    let reward_text = "pub fn grant() -> i64 { return 1 }";
    let bonus_text = "pub fn score() -> i64 { return 2 }";
    let main_uri = "file:///workspace/scripts/game/main.vela";
    let reward_uri = "file:///workspace/scripts/game/reward.vela";
    let bonus_uri = "file:///workspace/scripts/game/bonus.vela";
    for (uri, text) in [
        (reward_uri, reward_text),
        (bonus_uri, bonus_text),
        (main_uri, main_text),
    ] {
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
    }

    let rename = response_value(server.handle_json(&request(
        2,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 3,
                "character": line(main_text, 3).find("grant").expect("grant call")
            },
            "newName": "award"
        }),
    )));
    assert_eq!(rename["result"], serde_json::Value::Null);
}

fn line(text: &str, line: usize) -> &str {
    text.lines().nth(line).expect("line should exist")
}
