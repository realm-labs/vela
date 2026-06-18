use super::super::super::{LspServer, notification, notification_value, request, response_value};

#[test]
fn lsp_type_definition_follows_imported_source_struct_field_type_alias() {
    super::assert_imported_source_struct_field_type_definition();
}

#[test]
fn lsp_type_definition_follows_imported_parameter_source_type_alias() {
    super::assert_imported_parameter_source_type_definition();
}

#[test]
fn lsp_type_definition_follows_imported_local_source_type_alias() {
    super::assert_imported_local_source_type_definition();
}

#[test]
fn lsp_type_definition_follows_imported_local_source_type_hint() {
    super::assert_imported_local_source_type_hint_definition();
}

#[test]
fn lsp_type_definition_follows_imported_parameter_source_type_hint() {
    super::assert_imported_parameter_source_type_hint_definition();
}

#[test]
fn lsp_type_definition_follows_imported_trait_source_type_hint() {
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
    let traits_uri = "file:///workspace/scripts/game/traits.vela";
    let main_text = r#"use game::traits::Describable as Named

fn describe(value: Named) {
    return value;
}"#;
    let traits_text = r#"pub trait Describable {
    fn describe(self) -> String
}"#;
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": traits_uri,
                "languageId": "vela",
                "version": 1,
                "text": traits_text
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
    let parameter_line = main_text
        .lines()
        .nth(2)
        .expect("parameter line should exist");

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/typeDefinition",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 2,
                "character": parameter_line
                    .find("Named")
                    .expect("type hint should contain alias")
            }
        }),
    )));

    assert_eq!(response["result"]["uri"], traits_uri);
    assert_eq!(response["result"]["range"]["start"]["line"], 0);
    assert_eq!(response["result"]["range"]["start"]["character"], 10);
    assert_eq!(response["result"]["range"]["end"]["character"], 21);
}

#[test]
fn lsp_type_definition_follows_imported_field_source_type_hint() {
    super::assert_imported_field_source_type_hint_definition();
}

#[test]
fn lsp_type_definition_follows_imported_enum_field_source_type_hint() {
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
    let inventory_uri = "file:///workspace/scripts/game/inventory.vela";
    let main_text = r#"use game::inventory::Inventory as Bag

enum Reward {
    Granted { inventory: Bag },
    Skipped,
}"#;
    let inventory_text = r#"pub struct Inventory {
    slots: i64,
}"#;
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": inventory_uri,
                "languageId": "vela",
                "version": 1,
                "text": inventory_text
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
    let field_line = main_text.lines().nth(3).expect("enum field line");

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/typeDefinition",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 3,
                "character": field_line
                    .find("Bag")
                    .expect("type hint should contain alias")
            }
        }),
    )));

    assert_eq!(response["result"]["uri"], inventory_uri);
    assert_eq!(response["result"]["range"]["start"]["line"], 0);
    assert_eq!(response["result"]["range"]["start"]["character"], 11);
    assert_eq!(response["result"]["range"]["end"]["character"], 20);
}

#[test]
fn lsp_type_definition_follows_imported_return_source_type_hint() {
    super::assert_imported_return_source_type_hint_definition();
}

#[test]
fn lsp_type_definition_follows_imported_function_return_source_type() {
    super::assert_imported_function_return_source_type_definition();
}

#[test]
fn lsp_type_definition_follows_imported_source_member_type() {
    super::assert_imported_source_member_type_definition();
}

#[test]
fn lsp_type_definition_follows_imported_source_method_return_type() {
    super::assert_imported_source_method_return_type_definition();
}

#[test]
fn lsp_type_definition_follows_imported_enum_variant_constructor_type() {
    super::assert_imported_enum_variant_constructor_type_definition();
}

#[test]
fn lsp_type_definition_follows_imported_struct_constructor_type() {
    super::assert_imported_struct_constructor_type_definition();
}

#[test]
fn lsp_type_definition_follows_imported_const_and_global_source_types() {
    super::assert_imported_const_and_global_source_type_definitions();
}

#[test]
fn lsp_type_definition_follows_imported_const_and_global_source_type_hints() {
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
    let inventory_uri = "file:///workspace/scripts/game/inventory.vela";
    let main_text = r#"use game::inventory::Inventory as Bag

pub const DEFAULT_BAG: Bag = Bag { slots: 2 }
pub global active_bag: Bag"#;
    let inventory_text = r#"pub struct Inventory {
    slots: i64,
}"#;
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": inventory_uri,
                "languageId": "vela",
                "version": 1,
                "text": inventory_text
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
    let const_line = main_text.lines().nth(2).expect("const line should exist");
    let global_line = main_text.lines().nth(3).expect("global line should exist");

    let const_response = response_value(server.handle_json(&request(
        2,
        "textDocument/typeDefinition",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 2,
                "character": const_line
                    .find("Bag")
                    .expect("const type hint should exist")
            }
        }),
    )));
    assert_eq!(const_response["result"]["uri"], inventory_uri);
    assert_eq!(const_response["result"]["range"]["start"]["line"], 0);
    assert_eq!(const_response["result"]["range"]["start"]["character"], 11);
    assert_eq!(const_response["result"]["range"]["end"]["character"], 20);

    let global_response = response_value(server.handle_json(&request(
        3,
        "textDocument/typeDefinition",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 3,
                "character": global_line
                    .find("Bag")
                    .expect("global type hint should exist")
            }
        }),
    )));
    assert_eq!(global_response["result"]["uri"], inventory_uri);
    assert_eq!(global_response["result"]["range"]["start"]["line"], 0);
    assert_eq!(global_response["result"]["range"]["start"]["character"], 11);
    assert_eq!(global_response["result"]["range"]["end"]["character"], 20);
}
