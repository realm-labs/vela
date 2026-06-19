use crate::tests::{
    LspServer, handle_notification, handle_request, notification_value, response_value,
};

#[test]
fn lsp_rename_rejects_source_any_return_receiver_member() {
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
    let text = r#"fn source_any() -> Any { return 1 }
pub fn main() -> i64 {
    return source_any().level
}"#;
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
    let member = line(text, 2);
    let position = serde_json::json!({
        "line": 2,
        "character": member
            .find("level")
            .expect("source Any receiver member should exist")
    });

    let prepare = response_value(handle_request(
        &mut server,
        2,
        "textDocument/prepareRename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": position
        }),
    ));
    assert_eq!(prepare["result"], serde_json::Value::Null);

    let rename = response_value(handle_request(
        &mut server,
        3,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": position,
            "newName": "rank"
        }),
    ));
    assert_eq!(rename["result"], serde_json::Value::Null);
}

#[test]
fn lsp_source_trait_default_method_rename_updates_source_function_return_receiver_calls() {
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
    let text = r#"trait Rewardable {
    fn preview(self, amount: i64) -> i64 { return amount }
}
struct Player {
    level: i64,
}
impl Rewardable for Player {}
fn current_player() -> Player { return Player { level: 1 } }
pub fn main() -> i64 {
    let first = current_player().preview(1)
    return current_player().preview(first)
}"#;
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
    let first_call = line(text, 9);
    let position = serde_json::json!({
        "line": 9,
        "character": first_call
            .find("preview")
            .expect("trait default method call should exist")
    });

    let prepare = response_value(handle_request(
        &mut server,
        2,
        "textDocument/prepareRename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": position
        }),
    ));
    assert_eq!(prepare["result"]["placeholder"], "preview");
    assert_eq!(prepare["result"]["range"]["start"]["line"], 9);
    assert_eq!(
        prepare["result"]["range"]["start"]["character"],
        first_call
            .find("preview")
            .expect("trait default method call should exist")
    );

    let rename = response_value(handle_request(
        &mut server,
        3,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": position,
            "newName": "inspect"
        }),
    ));
    let edits = rename["result"]["changes"][uri]
        .as_array()
        .expect("rename should return text edits for the document");

    assert_eq!(edits.len(), 3, "{edits:?}");
    assert_text_edit(
        edits,
        1,
        line(text, 1)
            .find("preview")
            .expect("trait method declaration should exist"),
        "inspect",
    );
    assert_text_edit(
        edits,
        9,
        line(text, 9)
            .find("preview")
            .expect("first trait default method call should exist"),
        "inspect",
    );
    assert_text_edit(
        edits,
        10,
        line(text, 10)
            .find("preview")
            .expect("second trait default method call should exist"),
        "inspect",
    );
}

fn assert_text_edit(edits: &[serde_json::Value], line: usize, character: usize, new_text: &str) {
    assert!(
        edits.iter().any(|edit| {
            edit["range"]["start"]["line"] == line
                && edit["range"]["start"]["character"] == character
                && edit["newText"] == new_text
        }),
        "{edits:?}"
    );
}

fn line(text: &str, line: usize) -> &str {
    text.lines().nth(line).expect("line should exist")
}
