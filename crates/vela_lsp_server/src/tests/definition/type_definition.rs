use super::super::{LspServer, notification, notification_value, request, response_value};

#[test]
fn lsp_type_definition_follows_source_struct_field_type() {
    assert_source_struct_field_type_definition();
}

#[test]
fn lsp_type_definition_follows_imported_source_struct_field_type_alias() {
    assert_imported_source_struct_field_type_definition();
}

#[test]
fn lsp_type_definition_follows_imported_parameter_source_type_alias() {
    assert_imported_parameter_source_type_definition();
}

#[test]
fn lsp_type_definition_follows_imported_local_source_type_alias() {
    assert_imported_local_source_type_definition();
}

#[test]
fn lsp_type_definition_follows_imported_local_source_type_hint() {
    assert_imported_local_source_type_hint_definition();
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
fn lsp_type_definition_follows_imported_source_method_return_type() {
    assert_imported_source_method_return_type_definition();
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

fn assert_imported_parameter_source_type_definition() {
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

fn main(bag: Bag) {
    return bag;
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
    let return_line = main_text.lines().nth(3).expect("return line should exist");

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/typeDefinition",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 3,
                "character": return_line
                    .find("bag")
                    .expect("parameter use should contain name")
            }
        }),
    )));

    assert_eq!(response["result"]["uri"], inventory_uri);
    assert_eq!(response["result"]["range"]["start"]["line"], 0);
    assert_eq!(response["result"]["range"]["start"]["character"], 11);
    assert_eq!(response["result"]["range"]["end"]["character"], 20);
}

fn assert_imported_local_source_type_definition() {
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
use game::inventory::make_inventory

fn main() {
    let bag: Bag = make_inventory();
    return bag;
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
    let return_line = main_text.lines().nth(5).expect("return line should exist");

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/typeDefinition",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 5,
                "character": return_line
                    .find("bag")
                    .expect("local use should contain name")
            }
        }),
    )));

    assert_eq!(response["result"]["uri"], inventory_uri);
    assert_eq!(response["result"]["range"]["start"]["line"], 0);
    assert_eq!(response["result"]["range"]["start"]["character"], 11);
    assert_eq!(response["result"]["range"]["end"]["character"], 20);
}

fn assert_imported_local_source_type_hint_definition() {
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
use game::inventory::make_inventory

fn main() {
    let bag: Bag = make_inventory();
    return bag;
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
    let annotation_line = main_text
        .lines()
        .nth(4)
        .expect("annotation line should exist");

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/typeDefinition",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 4,
                "character": annotation_line
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

fn assert_imported_source_method_return_type_definition() {
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
    let main_text = r#"use game::rewards::RewardConfig

fn main(config: RewardConfig) {
return config.outcome();
}"#;
    let rewards_text = r#"pub enum RewardOutcome {
Granted,
Skipped,
}

pub struct RewardConfig {
count: i64,
}

impl RewardConfig {
pub fn outcome(self) -> RewardOutcome {
return RewardOutcome::Granted;
}
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
    let call_line = main_text.lines().nth(3).expect("method call line");

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/typeDefinition",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 3,
                "character": call_line
                    .find("outcome")
                    .expect("method call should contain name")
            }
        }),
    )));

    assert_eq!(response["result"]["uri"], rewards_uri);
    assert_eq!(response["result"]["range"]["start"]["line"], 0);
    assert_eq!(response["result"]["range"]["start"]["character"], 9);
    assert_eq!(response["result"]["range"]["end"]["character"], 22);
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
