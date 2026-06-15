use super::{LspServer, notification, notification_value, request, response_value};

#[test]
fn lsp_code_action_fixes_unknown_field_typo() {
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
    let text = "pub fn main(scores: Array<i64>) { return scores.frist() }";
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

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/codeAction",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 47 },
                "end": { "line": 0, "character": 52 }
            },
            "context": { "diagnostics": [] }
        }),
    )));
    let actions = response["result"]
        .as_array()
        .expect("codeAction should return an array");
    let action = actions
        .iter()
        .find(|action| action["title"] == "Replace with `first`")
        .expect("candidate quick fix should be returned");

    assert_eq!(action["kind"], "quickfix");
    let edit = &action["edit"]["changes"][uri][0];
    assert_eq!(edit["range"]["start"]["line"], 0);
    assert_eq!(
        edit["range"]["start"]["character"],
        text.find("frist").expect("member token")
    );
    assert_eq!(edit["range"]["end"]["line"], 0);
    assert_eq!(
        edit["range"]["end"]["character"],
        text.find("frist").expect("member token") + "frist".len()
    );
    assert_eq!(edit["newText"], "first");
}

#[test]
fn lsp_code_action_inserts_missing_import() {
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
    let reward_uri = "file:///workspace/scripts/game/reward.vela";
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": reward_uri,
                "languageId": "vela",
                "version": 1,
                "text": "pub fn grant() { return 1 }"
            }
        }),
    )));
    let text = "pub fn main() { return grant }";
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

    let grant_start = text.find("grant").expect("unresolved symbol");
    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/codeAction",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": grant_start },
                "end": { "line": 0, "character": grant_start + "grant".len() }
            },
            "context": { "diagnostics": [] }
        }),
    )));
    let actions = response["result"]
        .as_array()
        .expect("codeAction should return an array");
    let action = actions
        .iter()
        .find(|action| action["title"] == "Import `game::reward::grant`")
        .expect("import quick fix should be returned");

    assert_eq!(action["kind"], "quickfix");
    let edit = &action["edit"]["changes"][uri][0];
    assert_eq!(edit["range"]["start"]["line"], 0);
    assert_eq!(edit["range"]["start"]["character"], 0);
    assert_eq!(edit["range"]["end"]["line"], 0);
    assert_eq!(edit["range"]["end"]["character"], 0);
    assert_eq!(edit["newText"], "use game::reward::grant\n");
}
