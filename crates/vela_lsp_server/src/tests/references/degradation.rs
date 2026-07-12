use crate::tests::{TestServer, notification_value, notify, request, response_value};

use super::line;

#[test]
fn lsp_references_return_empty_for_dynamic_and_unresolved_targets() {
    let mut server = TestServer::new();
    let initialize = response_value(request::<lsp_types::request::Initialize>(
        &mut server,
        1,
        serde_json::json!({
            "processId": null,
            "rootUri": "file:///workspace/scripts",
            "capabilities": {}
        }),
    ));
    assert_eq!(
        initialize["result"]["capabilities"]["referencesProvider"],
        true
    );

    let text = "\
pub fn unresolved() { return missing }
pub fn dynamic(value: Any) { return value.level }";
    let uri = "file:///workspace/scripts/game/main.vela";
    let _ = notification_value(notify::<lsp_types::notification::DidOpenTextDocument>(
        &mut server,
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    ));

    assert_empty_references(
        &mut server,
        2,
        uri,
        0,
        line(text, 0)
            .find("missing")
            .expect("unresolved name should exist"),
    );
    assert_empty_references(
        &mut server,
        3,
        uri,
        1,
        line(text, 1)
            .find("level")
            .expect("dynamic member should exist"),
    );
}

#[test]
fn lsp_references_return_empty_for_source_any_return_receiver_member() {
    let mut server = TestServer::new();
    let initialize = response_value(request::<lsp_types::request::Initialize>(
        &mut server,
        1,
        serde_json::json!({
            "processId": null,
            "rootUri": "file:///workspace/scripts",
            "capabilities": {}
        }),
    ));
    assert_eq!(
        initialize["result"]["capabilities"]["referencesProvider"],
        true
    );

    let text = "\
struct Player { level: i64 }
fn source_any() -> Any { return Player { level: 1 } }
pub fn main() { return source_any().level }";
    let uri = "file:///workspace/scripts/game/main.vela";
    let _ = notification_value(notify::<lsp_types::notification::DidOpenTextDocument>(
        &mut server,
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    ));

    assert_empty_references(
        &mut server,
        2,
        uri,
        2,
        line(text, 2).find("level").expect("member use"),
    );
}

fn assert_empty_references(
    server: &mut TestServer,
    id: i32,
    uri: &str,
    line: usize,
    character: usize,
) {
    let response = response_value(request::<lsp_types::request::References>(
        server,
        id,
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": line,
                "character": character
            },
            "context": { "includeDeclaration": true }
        }),
    ));
    let references = response["result"]
        .as_array()
        .expect("references response should be an array");
    assert!(references.is_empty(), "{references:?}");
}
