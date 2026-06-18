use super::{LspServer, notification, notification_value, request, response_value};

#[test]
fn lsp_struct_body_completion_enters_field_declaration_context() {
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
    let text = "pub fn spawn_player() { return 1 }\npub struct Player {  }";
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
    let struct_line = text.lines().nth(1).expect("struct line should exist");

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/completion",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 1,
                "character": struct_line.find("{  }").expect("struct body") + "{ ".len()
            },
            "context": {
                "triggerKind": 1
            }
        }),
    )));

    assert_completion_snippet(&response, "field", "struct field", "${1:name}: ${2:Type}");
    assert_completion_snippet(
        &response,
        "field default",
        "struct field with default",
        "${1:name}: ${2:Type} = ${3:value}",
    );
    assert_no_completion(&response, "spawn_player");
    assert_no_completion(&response, "fn");

    let type_text = "pub struct Player { level: i }";
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didChange",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "version": 2
            },
            "contentChanges": [{ "text": type_text }]
        }),
    )));
    let type_response = response_value(server.handle_json(&request(
        3,
        "textDocument/completion",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 0,
                "character": type_text.find("i }").expect("type prefix") + "i".len()
            },
            "context": {
                "triggerKind": 2,
                "triggerCharacter": ":"
            }
        }),
    )));

    assert_completion(&type_response, "i64", 22, "i64");
    assert_no_completion(&type_response, "field");
}

fn assert_completion(response: &serde_json::Value, label: &str, kind: u8, detail: &str) {
    assert_eq!(response["result"]["isIncomplete"], false);
    let Some(items) = response["result"]["items"].as_array() else {
        panic!("completion response should contain items");
    };
    assert!(
        items
            .iter()
            .any(|item| item["label"] == label && item["kind"] == kind && item["detail"] == detail),
        "{items:?}"
    );
}

fn assert_completion_snippet(
    response: &serde_json::Value,
    label: &str,
    detail: &str,
    insert_text: &str,
) {
    assert_eq!(response["result"]["isIncomplete"], false);
    let Some(items) = response["result"]["items"].as_array() else {
        panic!("completion response should contain items");
    };
    let item = items
        .iter()
        .find(|item| {
            item["label"] == label
                && item["kind"] == 15
                && item["detail"] == detail
                && item["insertText"] == insert_text
                && item["insertTextFormat"] == 2
        })
        .unwrap_or_else(|| panic!("missing snippet completion item in {items:?}"));
    assert_eq!(item["textEdit"]["newText"], insert_text);
}

fn assert_no_completion(response: &serde_json::Value, label: &str) {
    assert_eq!(response["result"]["isIncomplete"], false);
    let Some(items) = response["result"]["items"].as_array() else {
        panic!("completion response should contain items");
    };
    assert!(items.iter().all(|item| item["label"] != label), "{items:?}");
}
