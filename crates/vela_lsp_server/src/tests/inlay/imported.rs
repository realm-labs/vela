use crate::tests::{
    LspServer, handle_notification, handle_request, notification_value, response_value,
};

#[test]
fn lsp_inlay_hints_show_imported_function_parameter_names() {
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
    open_document(
        &mut server,
        "file:///workspace/scripts/game/reward.vela",
        "pub fn grant(amount: i64, reason: String) -> i64 { return amount }",
    );
    let uri = "file:///workspace/scripts/game/main.vela";
    let text = "use game::reward::grant\npub fn main() { return grant(10, \"quest\") }";
    open_document(&mut server, uri, text);

    let response = response_value(handle_request(
        &mut server,
        2,
        "textDocument/inlayHint",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 1, "character": 0 },
                "end": { "line": 1, "character": 80 }
            }
        }),
    ));
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

#[test]
fn lsp_inlay_hints_show_imported_const_and_global_typefacts() {
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
    open_document(
        &mut server,
        "file:///workspace/scripts/game/config.vela",
        "pub const BONUS: i64 = 10\npub global reward_scale: i64",
    );
    let uri = "file:///workspace/scripts/game/main.vela";
    let text = r#"use game::config::BONUS
use game::config::reward_scale
pub fn main() {
    let scripted = BONUS;
    let scale = reward_scale;
}"#;
    open_document(&mut server, uri, text);

    let response = response_value(handle_request(
        &mut server,
        2,
        "textDocument/inlayHint",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 6, "character": 0 }
            }
        }),
    ));

    assert_eq!(
        response["result"],
        serde_json::json!([
            {
                "position": { "line": 3, "character": "    let scripted".len() },
                "label": ": i64",
                "kind": 1,
                "paddingRight": true
            },
            {
                "position": { "line": 4, "character": "    let scale".len() },
                "label": ": i64",
                "kind": 1,
                "paddingRight": true
            }
        ])
    );
}

#[test]
fn lsp_inlay_hints_show_imported_enum_variant_payload_names() {
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
    open_document(
        &mut server,
        "file:///workspace/scripts/game/quest.vela",
        r#"pub enum QuestProgress {
    Active(quest_id: String, count: i64),
    Done,
}"#,
    );
    let uri = "file:///workspace/scripts/game/main.vela";
    let text = r#"use game::quest::QuestProgress
pub fn main() {
    let active = QuestProgress::Active("quest-1", 3);
}"#;
    open_document(&mut server, uri, text);

    let response = response_value(handle_request(
        &mut server,
        2,
        "textDocument/inlayHint",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 4, "character": 0 }
            }
        }),
    ));
    let call_line = text.lines().nth(2).expect("call line should exist");

    assert_eq!(
        response["result"],
        serde_json::json!([
            {
                "position": { "line": 2, "character": call_line.find("\"quest-1\"").expect("first arg") },
                "label": "quest_id:",
                "kind": 2,
                "paddingRight": true
            },
            {
                "position": { "line": 2, "character": call_line.find(", 3").expect("second arg") + 2 },
                "label": "count:",
                "kind": 2,
                "paddingRight": true
            }
        ])
    );
}

fn open_document(server: &mut LspServer, uri: &str, text: &str) {
    let diagnostics = notification_value(handle_notification(
        server,
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
    assert_eq!(diagnostics["method"], "textDocument/publishDiagnostics");
    assert_eq!(diagnostics["params"]["uri"], uri);
    assert_eq!(diagnostics["params"]["diagnostics"], serde_json::json!([]));
}
