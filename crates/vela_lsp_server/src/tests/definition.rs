use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::{LspServer, notification, notification_value, request, response_value};

#[test]
fn lsp_definition_follows_open_overlay_local_binding() {
    assert_local_binding_navigation("textDocument/definition");
}

#[test]
fn lsp_declaration_follows_open_overlay_local_binding() {
    assert_local_binding_navigation("textDocument/declaration");
}

#[test]
fn lsp_definition_follows_function_call_after_qualified_stdlib_call() {
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
    let text = r#"
fn add_mixed(value) {
    math::abs(value);
    return value + 1i8;
}

fn main() {
    return add_mixed(1);
}
"#;
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
    let call_line = text.lines().nth(7).expect("call line should exist");

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/definition",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 7,
                "character": call_line
                    .find("add_mixed")
                    .expect("call should contain function name")
            }
        }),
    )));

    assert_eq!(response["result"]["uri"], uri);
    assert_eq!(response["result"]["range"]["start"]["line"], 1);
    assert_eq!(response["result"]["range"]["start"]["character"], 3);
    assert_eq!(response["result"]["range"]["end"]["character"], 12);
}

#[test]
fn lsp_definition_follows_imported_const_and_global_declarations() {
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
    let rewards_uri = "file:///workspace/scripts/game/rewards.vela";
    let main_text = r#"use game::rewards::BASE_REWARD
use game::rewards::reward_scale

pub fn main() {
    return BASE_REWARD + reward_scale
}"#;
    let rewards_text = r#"pub const BASE_REWARD = 4
pub global reward_scale: i64"#;
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": rewards_uri,
                "languageId": "vela",
                "version": 1,
                "text": rewards_text
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
    let return_line = main_text.lines().nth(4).expect("return line should exist");

    let const_response = response_value(server.handle_json(&request(
        2,
        "textDocument/definition",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 4,
                "character": return_line
                    .find("BASE_REWARD")
                    .expect("const use should exist")
            }
        }),
    )));

    assert_eq!(const_response["result"]["uri"], rewards_uri);
    assert_eq!(const_response["result"]["range"]["start"]["line"], 0);
    assert_eq!(const_response["result"]["range"]["start"]["character"], 10);
    assert_eq!(const_response["result"]["range"]["end"]["character"], 21);

    let global_response = response_value(server.handle_json(&request(
        3,
        "textDocument/definition",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 4,
                "character": return_line
                    .find("reward_scale")
                    .expect("global use should exist")
            }
        }),
    )));

    assert_eq!(global_response["result"]["uri"], rewards_uri);
    assert_eq!(global_response["result"]["range"]["start"]["line"], 1);
    assert_eq!(global_response["result"]["range"]["start"]["character"], 11);
    assert_eq!(global_response["result"]["range"]["end"]["character"], 23);
}

#[test]
fn lsp_definition_follows_source_struct_field_member_access() {
    assert_source_struct_field_navigation("textDocument/definition");
}

#[test]
fn lsp_declaration_follows_source_struct_field_member_access() {
    assert_source_struct_field_navigation("textDocument/declaration");
}

#[test]
fn lsp_type_definition_follows_source_struct_field_type() {
    assert_source_struct_field_type_definition();
}

#[test]
fn lsp_type_definition_follows_imported_source_struct_field_type_alias() {
    assert_imported_source_struct_field_type_definition();
}

#[test]
fn lsp_type_definition_follows_imported_function_return_source_type() {
    assert_imported_function_return_source_type_definition();
}

#[test]
fn lsp_type_definition_follows_imported_source_member_type() {
    assert_imported_source_member_type_definition();
}

#[test]
fn lsp_type_definition_follows_imported_enum_variant_constructor_type() {
    assert_imported_enum_variant_constructor_type_definition();
}

#[test]
fn lsp_type_definition_follows_imported_const_and_global_source_types() {
    assert_imported_const_and_global_source_type_definitions();
}

#[test]
fn lsp_type_definition_returns_null_for_source_primitive_field() {
    assert_source_primitive_field_type_definition_null();
}

#[test]
fn lsp_type_definition_returns_null_for_dynamic_local_value() {
    assert_dynamic_local_value_type_definition_null();
}

#[test]
fn lsp_definition_returns_null_for_unknown_source_member() {
    assert_unknown_source_member_navigation_null("textDocument/definition");
}

#[test]
fn lsp_declaration_returns_null_for_unknown_source_member() {
    assert_unknown_source_member_navigation_null("textDocument/declaration");
}

#[test]
fn lsp_declaration_returns_null_for_dynamic_member() {
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
    let text = "pub fn main(value: Any) { return value.level }";
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

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/declaration",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 0,
                "character": text
                    .find("level")
                    .expect("dynamic member should exist")
            }
        }),
    )));

    assert!(response["result"].is_null(), "{response:?}");
}

mod schema;

fn assert_unknown_source_member_navigation_null(method: &str) {
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
    let text = r#"struct Cell {
    value: i64,
}

fn assign_cell(cell: Cell) {
    return cell.missing;
}"#;
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
    let use_line = text.lines().nth(5).expect("member use line should exist");

    let response = response_value(server.handle_json(&request(
        2,
        method,
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 5,
                "character": use_line
                    .find("missing")
                    .expect("unknown member should exist")
            }
        }),
    )));

    assert!(response["result"].is_null());
}

fn assert_local_binding_navigation(method: &str) {
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
    let text = "pub fn main(amount: i64) -> i64 { return amount }";
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": "file:///workspace/scripts/game/main.vela",
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        method,
        serde_json::json!({
            "textDocument": { "uri": "file:///workspace/scripts/game/main.vela" },
            "position": {
                "line": 0,
                "character": text.rfind("amount").unwrap_or_else(|| {
                    panic!("definition fixture should contain amount use")
                })
            }
        }),
    )));

    assert_eq!(
        response["result"]["uri"],
        "file:///workspace/scripts/game/main.vela"
    );
    assert_eq!(response["result"]["range"]["start"]["line"], 0);
    assert_eq!(
        response["result"]["range"]["start"]["character"],
        text.find("amount").expect("parameter declaration")
    );
}

fn assert_source_struct_field_navigation(method: &str) {
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
    let text = r#"struct Cell {
    value: i64,
}

fn assign_cell(cell: Cell, value) {
    cell.value = value;
    return cell.value;
}

fn main() {
    let cell: Cell = Cell { value: 1 };
    return assign_cell(cell, "bad");
}"#;
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
    let field_use_line = text.lines().nth(5).expect("field use line should exist");
    let field_declaration_line = text
        .lines()
        .nth(1)
        .expect("field declaration line should exist");

    let response = response_value(server.handle_json(&request(
        2,
        method,
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 5,
                "character": field_use_line
                    .find("value")
                    .expect("field use should contain name")
            }
        }),
    )));

    assert_eq!(response["result"]["uri"], uri);
    assert_eq!(response["result"]["range"]["start"]["line"], 1);
    assert_eq!(
        response["result"]["range"]["start"]["character"],
        field_declaration_line
            .find("value")
            .expect("field declaration should contain name")
    );
    assert_eq!(
        response["result"]["range"]["end"]["character"],
        field_declaration_line
            .find("value")
            .expect("field declaration should contain name")
            + "value".len()
    );
}

fn assert_source_struct_field_type_definition() {
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
    let text = r#"struct Inventory {
    slots: i64,
}

struct Player {
    inventory: Inventory,
}

fn main(player: Player) {
    return player.inventory;
}"#;
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
    let field_use_line = text.lines().nth(9).expect("field use line should exist");

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/typeDefinition",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 9,
                "character": field_use_line
                    .find("inventory")
                    .expect("field use should contain name")
            }
        }),
    )));

    assert_eq!(response["result"]["uri"], uri);
    assert_eq!(response["result"]["range"]["start"]["line"], 0);
    assert_eq!(response["result"]["range"]["start"]["character"], 7);
    assert_eq!(response["result"]["range"]["end"]["character"], 16);
}

fn assert_imported_source_struct_field_type_definition() {
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

struct Player {
    inventory: Bag,
}

fn main(player: Player) {
    return player.inventory;
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
    let field_use_line = main_text
        .lines()
        .nth(7)
        .expect("field use line should exist");

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/typeDefinition",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 7,
                "character": field_use_line
                    .find("inventory")
                    .expect("field use should contain name")
            }
        }),
    )));

    assert_eq!(response["result"]["uri"], inventory_uri);
    assert_eq!(response["result"]["range"]["start"]["line"], 0);
    assert_eq!(response["result"]["range"]["start"]["character"], 11);
    assert_eq!(response["result"]["range"]["end"]["character"], 20);
}

fn assert_imported_function_return_source_type_definition() {
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
    let main_text = r#"use game::inventory::make_inventory

fn main() {
    return make_inventory();
}"#;
    let inventory_text = r#"pub struct Inventory {
    slots: i64,
}

pub fn make_inventory() -> Inventory {
    return Inventory { slots: 2 };
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
    let call_line = main_text.lines().nth(3).expect("call line should exist");

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/typeDefinition",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 3,
                "character": call_line
                    .find("make_inventory")
                    .expect("call should contain imported function name")
            }
        }),
    )));

    assert_eq!(response["result"]["uri"], inventory_uri);
    assert_eq!(response["result"]["range"]["start"]["line"], 0);
    assert_eq!(response["result"]["range"]["start"]["character"], 11);
    assert_eq!(response["result"]["range"]["end"]["character"], 20);
}

fn assert_imported_source_member_type_definition() {
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
    let main_text = r#"use game::inventory::Player

fn main(player: Player) {
return player.inventory;
}"#;
    let inventory_text = r#"pub struct Inventory {
slots: i64,
}

pub struct Player {
inventory: Inventory,
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
    let field_use_line = main_text
        .lines()
        .nth(3)
        .expect("field use line should exist");

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/typeDefinition",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 3,
                "character": field_use_line
                    .find("inventory")
                    .expect("field use should contain name")
            }
        }),
    )));

    assert_eq!(response["result"]["uri"], inventory_uri);
    assert_eq!(response["result"]["range"]["start"]["line"], 0);
    assert_eq!(response["result"]["range"]["start"]["character"], 11);
    assert_eq!(response["result"]["range"]["end"]["character"], 20);
}

fn assert_imported_enum_variant_constructor_type_definition() {
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
    let rewards_uri = "file:///workspace/scripts/game/rewards.vela";
    let main_text = r#"use game::rewards::RewardOutcome

fn main() {
    return RewardOutcome::Granted { item: "gold", count: 1 };
}"#;
    let rewards_text = r#"pub enum RewardOutcome {
    Granted { item: String, count: i64 },
    Skipped,
}"#;
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": rewards_uri,
                "languageId": "vela",
                "version": 1,
                "text": rewards_text
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
    let constructor_line = main_text
        .lines()
        .nth(3)
        .expect("constructor line should exist");

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/typeDefinition",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 3,
                "character": constructor_line
                    .find("Granted")
                    .expect("variant constructor should exist")
            }
        }),
    )));

    assert_eq!(response["result"]["uri"], rewards_uri);
    assert_eq!(response["result"]["range"]["start"]["line"], 0);
    assert_eq!(response["result"]["range"]["start"]["character"], 9);
    assert_eq!(response["result"]["range"]["end"]["character"], 22);
}

fn assert_imported_const_and_global_source_type_definitions() {
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
    let rewards_uri = "file:///workspace/scripts/game/rewards.vela";
    let main_text = r#"use game::rewards::DEFAULT_CONFIG
use game::rewards::active_config

fn main() {
    return DEFAULT_CONFIG.count + active_config.count;
}"#;
    let rewards_text = r#"pub struct RewardConfig {
    count: i64,
}

pub const DEFAULT_CONFIG: RewardConfig = RewardConfig { count: 1 }
pub global active_config: RewardConfig"#;
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": rewards_uri,
                "languageId": "vela",
                "version": 1,
                "text": rewards_text
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
    let return_line = main_text.lines().nth(4).expect("return line should exist");

    let const_response = response_value(server.handle_json(&request(
        2,
        "textDocument/typeDefinition",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 4,
                "character": return_line
                    .find("DEFAULT_CONFIG")
                    .expect("imported const use should exist")
            }
        }),
    )));
    assert_eq!(const_response["result"]["uri"], rewards_uri);
    assert_eq!(const_response["result"]["range"]["start"]["line"], 0);
    assert_eq!(const_response["result"]["range"]["start"]["character"], 11);
    assert_eq!(const_response["result"]["range"]["end"]["character"], 23);

    let global_response = response_value(server.handle_json(&request(
        3,
        "textDocument/typeDefinition",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 4,
                "character": return_line
                    .find("active_config")
                    .expect("imported global use should exist")
            }
        }),
    )));
    assert_eq!(global_response["result"]["uri"], rewards_uri);
    assert_eq!(global_response["result"]["range"]["start"]["line"], 0);
    assert_eq!(global_response["result"]["range"]["start"]["character"], 11);
    assert_eq!(global_response["result"]["range"]["end"]["character"], 23);
}

fn assert_source_primitive_field_type_definition_null() {
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
    let text = r#"struct Cell {
    value: i64,
}

fn main(cell: Cell) {
    return cell.value;
}"#;
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
    let field_use_line = text.lines().nth(5).expect("field use line should exist");

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/typeDefinition",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 5,
                "character": field_use_line
                    .find("value")
                    .expect("field use should contain name")
            }
        }),
    )));

    assert!(response["result"].is_null());
}

fn assert_dynamic_local_value_type_definition_null() {
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
    let text = r#"fn main(value) {
    return value;
}"#;
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
    let use_line = text.lines().nth(1).expect("value use line should exist");

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/typeDefinition",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 1,
                "character": use_line
                    .find("value")
                    .expect("value use should contain name")
            }
        }),
    )));

    assert!(response["result"].is_null());
}

fn assert_schema_source_navigation(method: &str) {
    let root = temp_workspace();
    let config_path = root.join("vela.toml");
    let schema_path = root.join("target").join("vela").join("schema.json");
    fs::create_dir_all(schema_path.parent().expect("schema should have parent"))
        .expect("schema directory should be creatable");
    let text = "pub fn main(player: Player) { return 1 }";
    let target_start = text
        .find("main")
        .expect("schema target marker should exist");
    let target_end = target_start + "main".len();
    fs::write(
        &config_path,
        r#"
            [workspace]
            roots = ["scripts"]

            [host]
            schema = "target/vela/schema.json"
        "#,
    )
    .expect("vela.toml should be writable");
    fs::write(
        &schema_path,
        serde_json::json!({
            "formatVersion": 1,
            "facts": {
                "types": [
                    {
                        "name": "Player",
                        "fact": { "kind": "host", "name": "Player" },
                        "sourceSpan": {
                            "source": 1,
                            "start": target_start,
                            "end": target_end
                        }
                    }
                ]
            }
        })
        .to_string(),
    )
    .expect("schema should be writable");

    let mut server = LspServer::new();
    let _ = response_value(server.handle_json(&request(
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": file_uri(&root),
            "capabilities": {}
        }),
    )));
    let _ = server.handle_json(&notification(
        "workspace/didChangeWatchedFiles",
        serde_json::json!({
            "changes": [{ "uri": file_uri(&config_path), "type": 1 }]
        }),
    ));
    let main_uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": main_uri,
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        method,
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 0,
                "character": text.find("Player").expect("type hint should exist")
            }
        }),
    )));

    assert_eq!(response["result"]["uri"], main_uri);
    assert_eq!(response["result"]["range"]["start"]["line"], 0);
    assert_eq!(
        response["result"]["range"]["start"]["character"],
        target_start
    );
    assert_eq!(response["result"]["range"]["end"]["character"], target_end);
    fs::remove_dir_all(root).expect("temporary workspace should be removable");
}

fn temp_workspace() -> PathBuf {
    let suffix = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(error) => panic!("system time should be after UNIX_EPOCH: {error}"),
    };
    let root = std::env::temp_dir().join(format!(
        "vela_lsp_server_definition_{}_{}",
        std::process::id(),
        suffix
    ));
    fs::create_dir_all(root.join("scripts").join("game"))
        .expect("temporary workspace should be creatable");
    root
}

fn file_uri(path: &Path) -> String {
    let path = path.display().to_string().replace('\\', "/");
    if path.starts_with('/') {
        format!("file://{path}")
    } else {
        format!("file:///{path}")
    }
}
