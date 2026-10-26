use std::fs;

use crate::tests::{TestServer, notification_value, notify, request, response_value};

use super::{file_uri, temp_workspace};

#[test]
fn lsp_hover_reports_source_method_on_source_function_return_receiver() {
    let text = include_str!(
        "../../../../../tests/lsp_matrix/fixtures/sources/local/returned-receivers/function-method.vela"
    );
    let method_line = text.lines().nth(10).expect("method use line should exist");
    let value = hover_value(text, 10, method_line.find("grant").expect("method use"));

    assert!(value.contains("game::main::Player.grant"), "{value}");
    assert!(
        value.contains("_method_: (self, amount: i64) -> bool"),
        "{value}"
    );
}

#[test]
fn lsp_hover_reports_source_method_on_source_method_return_receiver() {
    let text = include_str!(
        "../../../../../tests/lsp_matrix/fixtures/sources/local/returned-receivers/method-method.vela"
    );
    let method_line = text.lines().nth(15).expect("method use line should exist");
    let value = hover_value(text, 15, method_line.find("grant").expect("method use"));

    assert!(value.contains("game::main::Inventory.grant"), "{value}");
    assert!(
        value.contains("_method_: (self, amount: i64) -> bool"),
        "{value}"
    );
}

#[test]
fn lsp_hover_reports_source_trait_default_method_on_source_method_return_receiver() {
    let text = include_str!(
        "../../../../../tests/lsp_matrix/fixtures/sources/local/returned-receivers/method-default-trait.vela"
    );
    let method_line = text
        .lines()
        .nth(15)
        .expect("trait default method use line should exist");
    let value = hover_value(
        text,
        15,
        method_line.find("preview").expect("trait method use"),
    );

    assert!(value.contains("game::main::Rewardable.preview"), "{value}");
    assert!(
        value.contains("_method_: (self, amount: i64) -> bool"),
        "{value}"
    );
    assert!(value.contains("Preview reward"), "{value}");
}

fn hover_value(text: &str, line: usize, character: usize) -> String {
    let root = temp_workspace();
    let root_uri = file_uri(&root.join("scripts"));
    let uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    let mut server = TestServer::new();
    let _ = response_value(request::<lsp_types::request::Initialize>(
        &mut server,
        1,
        serde_json::json!({
            "processId": null,
            "rootUri": root_uri,
            "capabilities": {}
        }),
    ));
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

    let response = response_value(request::<lsp_types::request::HoverRequest>(
        &mut server,
        2,
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": line,
                "character": character
            }
        }),
    ));

    let value = response["result"]["contents"]["value"]
        .as_str()
        .expect("hover contents should be markdown")
        .to_owned();
    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
    value
}
