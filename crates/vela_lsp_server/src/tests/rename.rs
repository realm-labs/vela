use super::{LspServer, notification, notification_value, request, response_value};

#[test]
fn lsp_prepare_rename_rejects_keywords_and_literals() {
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
pub fn main(amount: i64) -> i64 {
    return amount + 1
}";
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

    let keyword_response = response_value(server.handle_json(&request(
        2,
        "textDocument/prepareRename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 1,
                "character": line(text, 1).find("return").expect("return keyword")
            }
        }),
    )));
    assert_eq!(keyword_response["result"], serde_json::Value::Null);

    let literal_response = response_value(server.handle_json(&request(
        3,
        "textDocument/prepareRename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 1,
                "character": line(text, 1).find('1').expect("literal")
            }
        }),
    )));
    assert_eq!(literal_response["result"], serde_json::Value::Null);
}

#[test]
fn lsp_local_rename_updates_all_function_uses() {
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
pub fn main(amount: i64) -> i64 {
    let next = amount + 1
    next += amount
    return next
}";
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
                "line": 2,
                "character": line(text, 2).find("next").expect("next write")
            }
        }),
    )));
    assert_eq!(prepare["result"]["placeholder"], "next");
    assert_eq!(prepare["result"]["range"]["start"]["line"], 2);
    assert_eq!(prepare["result"]["range"]["start"]["character"], 4);

    let rename = response_value(server.handle_json(&request(
        3,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 2,
                "character": line(text, 2).find("next").expect("next write")
            },
            "newName": "score"
        }),
    )));
    let edits = rename["result"]["changes"][uri]
        .as_array()
        .expect("rename should return text edits for the document");

    assert_eq!(edits.len(), 3);
    assert_text_edit(edits, 1, 8, "score");
    assert_text_edit(edits, 2, 4, "score");
    assert_text_edit(edits, 3, 11, "score");
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
