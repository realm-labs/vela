use super::{LspServer, handle_notification, handle_request, notification_value, response_value};

#[test]
fn lsp_selection_ranges_walk_syntax_ancestors() {
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
    let text = "\
pub fn main(player: Player) -> i64 {
    let next = player.level + 1
    if next > 1 {
        return next
    }
    return 0
}";
    let uri = "file:///workspace/scripts/game/main.vela";
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

    let response = response_value(handle_request(
        &mut server,
        2,
        "textDocument/selectionRange",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "positions": [{ "line": 1, "character": 22 }]
        }),
    ));

    let ranges = response["result"]
        .as_array()
        .expect("selectionRange should return an array");
    assert_eq!(ranges.len(), 1);
    let chain = flatten_selection_chain(&ranges[0]);
    assert!(
        chain.iter().any(|range| range["start"]["line"] == 1
            && range["start"]["character"] == 22
            && range["end"]["line"] == 1
            && range["end"]["character"] == 27),
        "{chain:?}"
    );
    assert!(
        chain.iter().any(|range| range["start"]["line"] == 1
            && range["start"]["character"] == 15
            && range["end"]["line"] == 1
            && range["end"]["character"] == 27),
        "{chain:?}"
    );
    assert!(
        chain.iter().any(|range| range["start"]["line"] == 1
            && range["start"]["character"] == 4
            && range["end"]["line"] == 1
            && range["end"]["character"] == 31),
        "{chain:?}"
    );
}

fn flatten_selection_chain(range: &serde_json::Value) -> Vec<&serde_json::Value> {
    let mut ranges = Vec::new();
    let mut current = Some(range);
    while let Some(selection) = current {
        ranges.push(&selection["range"]);
        current = selection.get("parent");
    }
    ranges
}

#[test]
fn lsp_selection_ranges_preserve_ancestors_under_parser_recovery() {
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
    let text = "\
pub fn main(player: Player) -> i64 {
    let next = player.level + 1
    if next > 1 {
        return next
";
    let uri = "file:///workspace/scripts/game/main.vela";
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

    let response = response_value(handle_request(
        &mut server,
        2,
        "textDocument/selectionRange",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "positions": [{ "line": 1, "character": 22 }]
        }),
    ));

    let ranges = response["result"]
        .as_array()
        .expect("selectionRange should return an array");
    assert_eq!(ranges.len(), 1);
    let chain = flatten_selection_chain(&ranges[0]);
    assert!(
        chain.iter().any(|range| range["start"]["line"] == 1
            && range["start"]["character"] == 22
            && range["end"]["line"] == 1
            && range["end"]["character"] == 27),
        "{chain:?}"
    );
    assert!(
        chain
            .iter()
            .any(|range| range["start"]["line"] == 0 && range["end"]["line"] == 4),
        "{chain:?}"
    );
}
