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

#[test]
fn lsp_private_function_rename_updates_imports() {
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
pub fn main(amount: i64) -> i64 {
    return grant(amount)
}";
    let helper_text = "pub fn grant(amount: i64) -> i64 { return amount }";
    let main_uri = "file:///workspace/scripts/game/main.vela";
    let helper_uri = "file:///workspace/scripts/game/reward.vela";
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": helper_uri,
                "languageId": "vela",
                "version": 1,
                "text": helper_text
            }
        }),
    )));
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": main_uri,
                "languageId": "vela",
                "version": 1,
                "text": main_text
            }
        }),
    )));

    let prepare = response_value(server.handle_json(&request(
        2,
        "textDocument/prepareRename",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 2,
                "character": line(main_text, 2).find("grant").expect("grant call")
            }
        }),
    )));
    assert_eq!(prepare["result"]["placeholder"], "grant");
    assert_eq!(prepare["result"]["range"]["start"]["line"], 2);
    assert_eq!(prepare["result"]["range"]["start"]["character"], 11);

    let rename = response_value(server.handle_json(&request(
        3,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 2,
                "character": line(main_text, 2).find("grant").expect("grant call")
            },
            "newName": "award"
        }),
    )));
    let main_edits = rename["result"]["changes"][main_uri]
        .as_array()
        .expect("rename should return main edits");
    let helper_edits = rename["result"]["changes"][helper_uri]
        .as_array()
        .expect("rename should return helper edits");

    assert_eq!(main_edits.len(), 2);
    assert_text_edit(main_edits, 0, 18, "award");
    assert_text_edit(main_edits, 2, 11, "award");
    assert_eq!(helper_edits.len(), 1);
    assert_text_edit(helper_edits, 0, 7, "award");
}

#[test]
fn lsp_private_value_declaration_rename_updates_uses() {
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
const BONUS: i64 = 5
pub fn main() -> i64 {
    return BONUS + BONUS
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
                "character": line(text, 2).find("BONUS").expect("BONUS read")
            }
        }),
    )));
    assert_eq!(prepare["result"]["placeholder"], "BONUS");
    assert_eq!(prepare["result"]["range"]["start"]["line"], 2);
    assert_eq!(prepare["result"]["range"]["start"]["character"], 11);

    let rename = response_value(server.handle_json(&request(
        3,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 2,
                "character": line(text, 2).find("BONUS").expect("BONUS read")
            },
            "newName": "BASE"
        }),
    )));
    let edits = rename["result"]["changes"][uri]
        .as_array()
        .expect("rename should return text edits for the document");

    assert_eq!(edits.len(), 3);
    assert_text_edit(edits, 0, 6, "BASE");
    assert_text_edit(edits, 2, 11, "BASE");
    assert_text_edit(edits, 2, 19, "BASE");
}

#[test]
fn lsp_private_type_declaration_rename_updates_type_hints() {
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
struct Reward {
    amount: i64
}

fn grant(reward: Reward) -> Reward {
    let next: Reward = reward
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
                "line": 4,
                "character": line(text, 4).rfind("Reward").expect("return type")
            }
        }),
    )));
    assert_eq!(prepare["result"]["placeholder"], "Reward");
    assert_eq!(prepare["result"]["range"]["start"]["line"], 4);
    assert_eq!(prepare["result"]["range"]["start"]["character"], 28);

    let rename = response_value(server.handle_json(&request(
        3,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 4,
                "character": line(text, 4).rfind("Reward").expect("return type")
            },
            "newName": "Prize"
        }),
    )));
    let edits = rename["result"]["changes"][uri]
        .as_array()
        .expect("rename should return text edits for the document");

    assert_eq!(edits.len(), 4);
    assert_text_edit(edits, 0, 7, "Prize");
    assert_text_edit(edits, 4, 17, "Prize");
    assert_text_edit(edits, 4, 28, "Prize");
    assert_text_edit(edits, 5, 14, "Prize");
}

#[test]
fn lsp_public_export_rename_reports_hot_reload_risk() {
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
    let text = "pub fn grant(amount: i64) -> i64 { return amount }";
    let uri = "file:///workspace/scripts/game/reward.vela";
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

    let rename = response_value(server.handle_json(&request(
        2,
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
    let edits = rename["result"]["changes"][uri]
        .as_array()
        .expect("rename should return text edits");
    assert_eq!(edits.len(), 1);
    assert_text_edit(edits, 0, 7, "award");

    let annotations = rename["result"]["changeAnnotations"]
        .as_object()
        .expect("public export rename should include a change annotation");
    let risk = &annotations["renameRisk0"];
    assert_eq!(risk["needsConfirmation"], true);
    assert_eq!(risk["description"], "hotReloadAbi");
    assert!(
        risk["label"]
            .as_str()
            .expect("risk label should be a string")
            .contains("public function `grant`")
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
