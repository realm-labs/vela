use std::fs;

use super::{file_uri, temp_workspace};
use crate::tests::{
    LspServer, handle_notification, handle_request, notification_value, response_value,
};

#[test]
fn lsp_definition_follows_source_method_on_source_function_return_receiver() {
    assert_source_method_navigation_on_source_function_return_receiver("textDocument/definition");
}

#[test]
fn lsp_declaration_follows_source_method_on_source_function_return_receiver() {
    assert_source_method_navigation_on_source_function_return_receiver("textDocument/declaration");
}

#[test]
fn lsp_definition_follows_source_method_on_source_method_return_receiver() {
    assert_source_method_navigation_on_source_method_return_receiver("textDocument/definition");
}

#[test]
fn lsp_declaration_follows_source_method_on_source_method_return_receiver() {
    assert_source_method_navigation_on_source_method_return_receiver("textDocument/declaration");
}

#[test]
fn lsp_definition_follows_source_trait_default_method_on_source_method_return_receiver() {
    assert_source_trait_default_method_navigation_on_source_method_return_receiver(
        "textDocument/definition",
    );
}

#[test]
fn lsp_declaration_follows_source_trait_default_method_on_source_method_return_receiver() {
    assert_source_trait_default_method_navigation_on_source_method_return_receiver(
        "textDocument/declaration",
    );
}

fn assert_source_method_navigation_on_source_function_return_receiver(method: &str) {
    let text = r#"struct Player {
    level: i64,
}
impl Player {
    fn grant(self, amount: i64) -> bool {
        return amount > 0
    }
}
fn current_player() -> Player { return Player { level: 1 } }
pub fn main() {
    return current_player().grant(3)
}"#;
    assert_navigation(
        method,
        text,
        NavigationExpectation {
            call_line: 10,
            call_name: "grant",
            declaration_line: 4,
            declaration_name: "grant",
        },
    );
}

fn assert_source_method_navigation_on_source_method_return_receiver(method: &str) {
    let text = r#"struct Player {
    level: i64,
}
struct Inventory {
    slots: i64,
}
impl Player {
    fn inventory(self) -> Inventory { return Inventory { slots: 1 } }
}
impl Inventory {
    fn grant(self, amount: i64) -> bool {
        return amount > 0
    }
}
pub fn main(player: Player) {
    return player.inventory().grant(3)
}"#;
    assert_navigation(
        method,
        text,
        NavigationExpectation {
            call_line: 15,
            call_name: "grant",
            declaration_line: 10,
            declaration_name: "grant",
        },
    );
}

fn assert_source_trait_default_method_navigation_on_source_method_return_receiver(method: &str) {
    let text = r#"trait Rewardable {
    #[doc("Preview reward")]
    fn preview(self, amount: i64) -> bool { return amount > 0 }
}
struct Player {
    level: i64,
}
struct Inventory {
    slots: i64,
}
impl Player {
    fn inventory(self) -> Inventory { return Inventory { slots: 1 } }
}
impl Rewardable for Inventory {}
pub fn main(player: Player) {
    return player.inventory().preview(1)
}"#;
    assert_navigation(
        method,
        text,
        NavigationExpectation {
            call_line: 15,
            call_name: "preview",
            declaration_line: 2,
            declaration_name: "preview",
        },
    );
}

struct NavigationExpectation {
    call_line: usize,
    call_name: &'static str,
    declaration_line: usize,
    declaration_name: &'static str,
}

fn assert_navigation(method: &str, text: &str, expectation: NavigationExpectation) {
    let root = temp_workspace();
    let root_uri = file_uri(&root.join("scripts"));
    let uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    let mut server = LspServer::new();
    let _ = response_value(handle_request(
        &mut server,
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": root_uri,
            "capabilities": {}
        }),
    ));
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
    let call_line = text
        .lines()
        .nth(expectation.call_line)
        .expect("call line should exist");
    let declaration_line = text
        .lines()
        .nth(expectation.declaration_line)
        .expect("declaration line should exist");

    let response = response_value(handle_request(
        &mut server,
        2,
        method,
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": expectation.call_line,
                "character": call_line
                    .find(expectation.call_name)
                    .unwrap_or_else(|| panic!("{} call should exist", expectation.call_name))
            }
        }),
    ));

    assert_eq!(response["result"]["uri"], uri);
    assert_eq!(
        response["result"]["range"]["start"]["line"],
        expectation.declaration_line
    );
    assert_eq!(
        response["result"]["range"]["start"]["character"],
        declaration_line
            .find(expectation.declaration_name)
            .unwrap_or_else(|| panic!("{} declaration should exist", expectation.declaration_name))
    );
    assert_eq!(
        response["result"]["range"]["end"]["character"],
        declaration_line
            .find(expectation.declaration_name)
            .unwrap_or_else(|| panic!("{} declaration should exist", expectation.declaration_name))
            + expectation.declaration_name.len()
    );
    fs::remove_dir_all(root).expect("temporary workspace should be removable");
}
