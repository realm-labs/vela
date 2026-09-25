use crate::tests::{TestServer, request, response_value};

use super::{assert_highlight, assert_reference, line};

#[test]
fn lsp_references_find_imported_module_segments() {
    let mut server = TestServer::new();
    let _ = response_value(request::<lsp_types::request::Initialize>(
        &mut server,
        1,
        serde_json::json!({
            "processId": null,
            "rootUri": "file:///workspace/scripts",
            "capabilities": {}
        }),
    ));
    let main_text = "\
use game::reward::grant
pub fn main() -> i64 { return grant() }";
    let other_text = "\
use game::reward::bonus
pub fn other() -> i64 { return bonus() }";
    let helper_text = "pub fn grant() -> i64 { return 1 }\npub fn bonus() -> i64 { return 2 }";
    let main_uri = "file:///workspace/scripts/game/main.vela";
    let other_uri = "file:///workspace/scripts/game/other.vela";
    let helper_uri = "file:///workspace/scripts/game/reward.vela";
    for (uri, text) in [
        (helper_uri, helper_text),
        (other_uri, other_text),
        (main_uri, main_text),
    ] {
        let _ = crate::tests::sync_diagnostics::<lsp_types::notification::DidOpenTextDocument>(
            &mut server,
            serde_json::json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": "vela",
                    "version": 1,
                    "text": text
                }
            }),
        );
    }

    let response = response_value(request::<lsp_types::request::References>(
        &mut server,
        2,
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 0,
                "character": line(main_text, 0).find("reward").expect("module segment")
            },
            "context": { "includeDeclaration": true }
        }),
    ));
    let references = response["result"]
        .as_array()
        .expect("references response should be an array");

    assert_eq!(references.len(), 2, "{references:?}");
    assert_reference(
        references,
        main_uri,
        0,
        line(main_text, 0)
            .find("reward")
            .expect("first module segment"),
    );
    assert_reference(
        references,
        other_uri,
        0,
        line(other_text, 0)
            .find("reward")
            .expect("second module segment"),
    );
}

#[test]
fn lsp_document_highlight_marks_imported_module_segments() {
    let mut server = TestServer::new();
    let _ = response_value(request::<lsp_types::request::Initialize>(
        &mut server,
        1,
        serde_json::json!({
            "processId": null,
            "rootUri": "file:///workspace/scripts",
            "capabilities": {}
        }),
    ));
    let text = "\
use game::reward::grant
use game::reward::bonus
pub fn main() -> i64 {
    return grant() + bonus()
}";
    let helper_text = "pub fn grant() -> i64 { return 1 }\npub fn bonus() -> i64 { return 2 }";
    let uri = "file:///workspace/scripts/game/main.vela";
    let helper_uri = "file:///workspace/scripts/game/reward.vela";
    for (uri, text) in [(helper_uri, helper_text), (uri, text)] {
        let _ = crate::tests::sync_diagnostics::<lsp_types::notification::DidOpenTextDocument>(
            &mut server,
            serde_json::json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": "vela",
                    "version": 1,
                    "text": text
                }
            }),
        );
    }

    let response = response_value(request::<lsp_types::request::DocumentHighlightRequest>(
        &mut server,
        2,
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 0,
                "character": line(text, 0).find("reward").expect("module segment")
            }
        }),
    ));
    let highlights = response["result"]
        .as_array()
        .expect("documentHighlight response should be an array");

    assert_eq!(highlights.len(), 2, "{highlights:?}");
    assert_highlight(
        highlights,
        0,
        line(text, 0).find("reward").expect("first module segment"),
        1,
    );
    assert_highlight(
        highlights,
        1,
        line(text, 1).find("reward").expect("second module segment"),
        1,
    );
}

#[test]
fn lsp_module_segments_stay_distinct_from_builtin_and_stdlib_query_targets() {
    use lsp_types::{notification as n, request as r};
    use serde_json::json;

    let mut server = TestServer::new();
    let _ = response_value(request::<r::Initialize>(
        &mut server,
        1,
        json!({"processId":null,"rootUri":"file:///workspace/scripts","capabilities":{}}),
    ));
    let text = "\
use game::reward::grant
use game::reward::bonus
fn main(amount: i64) -> i64 {
    let next = max(amount, 1)
    return grant() + bonus() + next
}";
    let main = "file:///workspace/scripts/game/main.vela";
    let helper = "file:///workspace/scripts/game/reward.vela";
    for (uri, source) in [
        (
            helper,
            "pub fn grant() -> i64 { return 1 }\npub fn bonus() -> i64 { return 2 }",
        ),
        (main, text),
    ] {
        let _ = crate::tests::sync_diagnostics::<n::DidOpenTextDocument>(
            &mut server,
            json!({"textDocument":{"uri":uri,"languageId":"vela","version":1,"text":source}}),
        );
    }
    let mut id = 1;
    for (line_index, needle, owned) in [(0, "reward", true), (2, "i64", false), (3, "max", false)] {
        let position = json!({
            "line":line_index,
            "character":line(text, line_index).find(needle).expect("query token")
        });
        let base = json!({"textDocument":{"uri":main},"position":position});
        for include in [true, false] {
            let mut params = base.clone();
            params["context"] = json!({"includeDeclaration":include});
            let refs = query_value::<r::References>(&mut server, &mut id, params);
            assert_eq!(
                refs.as_array().expect("references").len(),
                if owned { 2 } else { 0 }
            );
        }
        let highlights =
            query_value::<r::DocumentHighlightRequest>(&mut server, &mut id, base.clone());
        assert_eq!(
            highlights.as_array().expect("highlights").len(),
            if owned { 2 } else { 0 }
        );
        assert!(
            query_value::<r::PrepareRenameRequest>(&mut server, &mut id, base.clone()).is_null()
        );
        let mut params = base;
        params["newName"] = json!("awards");
        assert!(query_value::<r::Rename>(&mut server, &mut id, params).is_null());
    }
}

fn query_value<R: lsp_types::request::Request>(
    server: &mut TestServer,
    id: &mut i32,
    params: serde_json::Value,
) -> serde_json::Value {
    *id += 1;
    let response = response_value(request::<R>(server, *id, params));
    assert!(response.get("error").is_none(), "{response}");
    response["result"].clone()
}
