use crate::tests::{
    LspServer, handle_notification, handle_request, notification_value, response_value,
};

use super::{assert_highlight, assert_reference, line};

#[test]
fn lsp_references_find_source_method_calls_on_source_method_return_receivers() {
    let (mut server, uri, text) = open_source_method_return_method_fixture();

    let response = response_value(handle_request(
        &mut server,
        2,
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 17,
                "character": line(text, 17).find("grant").expect("method call")
            },
            "context": { "includeDeclaration": true }
        }),
    ));
    let references = response["result"]
        .as_array()
        .expect("references response should be an array");

    assert_eq!(references.len(), 3, "{references:?}");
    assert_reference(
        references,
        uri,
        13,
        line(text, 13).find("grant").expect("method declaration"),
    );
    assert_reference(
        references,
        uri,
        17,
        line(text, 17).find("grant").expect("first method call"),
    );
    assert_reference(
        references,
        uri,
        18,
        line(text, 18).find("grant").expect("second method call"),
    );
}

#[test]
fn lsp_references_find_source_trait_default_method_calls_on_source_method_return_receivers() {
    let (mut server, uri, text) = open_source_method_return_trait_fixture();

    let response = response_value(handle_request(
        &mut server,
        2,
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 19,
                "character": line(text, 19).find("preview").expect("trait method call")
            },
            "context": { "includeDeclaration": true }
        }),
    ));
    let references = response["result"]
        .as_array()
        .expect("references response should be an array");

    assert_eq!(references.len(), 3, "{references:?}");
    assert_reference(
        references,
        uri,
        1,
        line(text, 1)
            .find("preview")
            .expect("trait method declaration"),
    );
    assert_reference(
        references,
        uri,
        19,
        line(text, 19)
            .find("preview")
            .expect("first trait method call"),
    );
    assert_reference(
        references,
        uri,
        20,
        line(text, 20)
            .find("preview")
            .expect("second trait method call"),
    );
}

#[test]
fn lsp_document_highlight_marks_source_method_calls_on_source_method_return_receivers() {
    let (mut server, uri, text) = open_source_method_return_method_fixture();

    let response = response_value(handle_request(
        &mut server,
        2,
        "textDocument/documentHighlight",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 17,
                "character": line(text, 17).find("grant").expect("method call")
            }
        }),
    ));
    let highlights = response["result"]
        .as_array()
        .expect("documentHighlight response should be an array");

    assert_eq!(highlights.len(), 3, "{highlights:?}");
    assert_highlight(
        highlights,
        13,
        line(text, 13).find("grant").expect("method declaration"),
        1,
    );
    assert_highlight(
        highlights,
        17,
        line(text, 17).find("grant").expect("first method call"),
        1,
    );
    assert_highlight(
        highlights,
        18,
        line(text, 18).find("grant").expect("second method call"),
        1,
    );
}

#[test]
fn lsp_document_highlight_marks_source_trait_default_method_calls_on_source_method_return_receivers()
 {
    let (mut server, uri, text) = open_source_method_return_trait_fixture();

    let response = response_value(handle_request(
        &mut server,
        2,
        "textDocument/documentHighlight",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 19,
                "character": line(text, 19).find("preview").expect("trait method call")
            }
        }),
    ));
    let highlights = response["result"]
        .as_array()
        .expect("documentHighlight response should be an array");

    assert_eq!(highlights.len(), 3, "{highlights:?}");
    assert_highlight(
        highlights,
        1,
        line(text, 1)
            .find("preview")
            .expect("trait method declaration"),
        1,
    );
    assert_highlight(
        highlights,
        19,
        line(text, 19)
            .find("preview")
            .expect("first trait method call"),
        1,
    );
    assert_highlight(
        highlights,
        20,
        line(text, 20)
            .find("preview")
            .expect("second trait method call"),
        1,
    );
}

fn open_source_method_return_method_fixture() -> (LspServer, &'static str, &'static str) {
    let text = "\
pub struct Player {
    level: i64
}

pub struct Inventory {
    count: i64
}

impl Player {
    pub fn inventory(self) -> Inventory { return Inventory { count: 1 } }
}

impl Inventory {
    pub fn grant(self, amount: i64) -> i64 { return amount }
}

pub fn main(player: Player) -> i64 {
    let first = player.inventory().grant(1)
    return player.inventory().grant(first)
}";
    open_fixture(text)
}

fn open_source_method_return_trait_fixture() -> (LspServer, &'static str, &'static str) {
    let text = "\
pub trait Rewardable {
    fn preview(self, amount: i64) -> i64 { return amount }
}

pub struct Player {
    level: i64
}

pub struct Inventory {
    count: i64
}

impl Player {
    pub fn inventory(self) -> Inventory { return Inventory { count: 1 } }
}

impl Rewardable for Inventory {}

pub fn main(player: Player) -> i64 {
    let first = player.inventory().preview(1)
    return player.inventory().preview(first)
}";
    open_fixture(text)
}

fn open_fixture(text: &'static str) -> (LspServer, &'static str, &'static str) {
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
    (server, uri, text)
}
