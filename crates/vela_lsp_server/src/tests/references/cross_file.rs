use crate::tests::{LspServer, notification, notification_value, request, response_value};

use super::{assert_reference, line};

#[test]
fn lsp_references_find_cross_file_imported_source_field_and_method_uses() {
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
    let main_uri = "file:///workspace/scripts/game/main.vela";
    let types_uri = "file:///workspace/scripts/game/types.vela";
    let main_text = "\
use game::types::Reward

pub fn main(reward: Reward) -> i64 {
    let first = reward.amount
    let second = reward.total()
    return first + second + reward.amount + reward.total()
}";
    let types_text = "\
pub struct Reward {
    amount: i64
}

impl Reward {
    pub fn total(self) -> i64 { return 1 }
}";
    for (uri, text) in [(types_uri, types_text), (main_uri, main_text)] {
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
    }

    let field_response = response_value(server.handle_json(&request(
        2,
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 3,
                "character": line(main_text, 3)
                    .find("amount")
                    .expect("first field read should exist")
            },
            "context": { "includeDeclaration": true }
        }),
    )));
    let field_references = field_response["result"]
        .as_array()
        .expect("references response should be an array");

    assert_eq!(field_references.len(), 3, "{field_references:?}");
    assert_reference(
        field_references,
        types_uri,
        1,
        line(types_text, 1)
            .find("amount")
            .expect("field declaration should exist"),
    );
    assert_reference(
        field_references,
        main_uri,
        3,
        line(main_text, 3)
            .find("amount")
            .expect("first field read should exist"),
    );
    assert_reference(
        field_references,
        main_uri,
        5,
        line(main_text, 5)
            .find("amount")
            .expect("second field read should exist"),
    );

    let method_response = response_value(server.handle_json(&request(
        3,
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 4,
                "character": line(main_text, 4)
                    .find("total")
                    .expect("first method call should exist")
            },
            "context": { "includeDeclaration": true }
        }),
    )));
    let method_references = method_response["result"]
        .as_array()
        .expect("references response should be an array");

    assert_eq!(method_references.len(), 3, "{method_references:?}");
    assert_reference(
        method_references,
        types_uri,
        5,
        line(types_text, 5)
            .find("total")
            .expect("method declaration should exist"),
    );
    assert_reference(
        method_references,
        main_uri,
        4,
        line(main_text, 4)
            .find("total")
            .expect("first method call should exist"),
    );
    assert_reference(
        method_references,
        main_uri,
        5,
        line(main_text, 5)
            .find("total")
            .expect("second method call should exist"),
    );
}
