use super::{LspServer, handle_notification, handle_request, notification_value, response_value};

#[test]
fn lsp_folding_ranges_cover_items_and_blocks() {
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
use game::reward::grant
use game::reward::Reward

pub struct Player {
    level: i64
}

pub fn main(player: Player) -> i64 {
    if player.level > 1 {
        return match player.level {
            1 => {
                return 1
            }
            _ => {
                return 2
            }
        }
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
        "textDocument/foldingRange",
        serde_json::json!({
            "textDocument": { "uri": uri }
        }),
    ));

    let ranges = response["result"]
        .as_array()
        .expect("foldingRange should return an array");
    assert!(
        ranges.iter().any(|range| {
            range["kind"] == "imports" && range["startLine"] == 0 && range["endLine"] == 1
        }),
        "{ranges:?}"
    );
    assert!(
        ranges.iter().any(|range| {
            range["kind"] == "region" && range["startLine"] == 3 && range["endLine"] == 5
        }),
        "{ranges:?}"
    );
    assert!(
        ranges.iter().any(|range| {
            range["kind"] == "region" && range["startLine"] == 7 && range["endLine"] == 19
        }),
        "{ranges:?}"
    );
    assert!(
        ranges.iter().any(|range| {
            range["kind"] == "region" && range["startLine"] == 10 && range["endLine"] == 12
        }),
        "{ranges:?}"
    );
}

#[test]
fn lsp_folding_ranges_cover_multiline_literals_and_recovery() {
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
pub fn main() -> i64 {
    let scores = [
        1,
        2
    ]
    let rewards = {
        \"gold\": 1,
        \"xp\": 2
    }
    let label = \"\"\"
daily
quest
\"\"\"
    return scores[0]
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
        "textDocument/foldingRange",
        serde_json::json!({
            "textDocument": { "uri": uri }
        }),
    ));

    let ranges = response["result"]
        .as_array()
        .expect("foldingRange should return an array");
    assert!(
        ranges.iter().any(|range| {
            range["kind"] == "region" && range["startLine"] == 1 && range["endLine"] == 4
        }),
        "{ranges:?}"
    );
    assert!(
        ranges.iter().any(|range| {
            range["kind"] == "region" && range["startLine"] == 5 && range["endLine"] == 8
        }),
        "{ranges:?}"
    );
    assert!(
        ranges.iter().any(|range| {
            range["kind"] == "region" && range["startLine"] == 9 && range["endLine"] == 12
        }),
        "{ranges:?}"
    );
}
