use crate::tests::{
    LspServer, handle_notification, handle_request, notification_value, response_value,
};

use super::{assert_reference, line};

#[test]
fn lsp_references_find_imported_const_and_global_uses() {
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
    let main_text = "\
use game::rewards::BASE_REWARD
use game::rewards::reward_scale
pub fn main() -> i64 {
    let first = BASE_REWARD
    return first + reward_scale
}";
    let rewards_text = "\
pub const BASE_REWARD = 4
pub global reward_scale: i64";
    let main_uri = "file:///workspace/scripts/game/main.vela";
    let rewards_uri = "file:///workspace/scripts/game/rewards.vela";
    for (uri, text) in [(rewards_uri, rewards_text), (main_uri, main_text)] {
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
    }

    let const_response = response_value(handle_request(
        &mut server,
        2,
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 3,
                "character": line(main_text, 3)
                    .find("BASE_REWARD")
                    .expect("const use should exist")
            },
            "context": { "includeDeclaration": true }
        }),
    ));
    let const_references = const_response["result"]
        .as_array()
        .expect("references response should be an array");

    assert_eq!(const_references.len(), 3, "{const_references:?}");
    assert_reference(
        const_references,
        rewards_uri,
        0,
        line(rewards_text, 0)
            .find("BASE_REWARD")
            .expect("const declaration should exist"),
    );
    assert_reference(
        const_references,
        main_uri,
        0,
        line(main_text, 0)
            .find("BASE_REWARD")
            .expect("const import should exist"),
    );
    assert_reference(
        const_references,
        main_uri,
        3,
        line(main_text, 3)
            .find("BASE_REWARD")
            .expect("const use should exist"),
    );

    let global_response = response_value(handle_request(
        &mut server,
        3,
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 4,
                "character": line(main_text, 4)
                    .find("reward_scale")
                    .expect("global use should exist")
            },
            "context": { "includeDeclaration": true }
        }),
    ));
    let global_references = global_response["result"]
        .as_array()
        .expect("references response should be an array");

    assert_eq!(global_references.len(), 3, "{global_references:?}");
    assert_reference(
        global_references,
        rewards_uri,
        1,
        line(rewards_text, 1)
            .find("reward_scale")
            .expect("global declaration should exist"),
    );
    assert_reference(
        global_references,
        main_uri,
        1,
        line(main_text, 1)
            .find("reward_scale")
            .expect("global import should exist"),
    );
    assert_reference(
        global_references,
        main_uri,
        4,
        line(main_text, 4)
            .find("reward_scale")
            .expect("global use should exist"),
    );
}

#[test]
fn lsp_references_find_imported_function_alias_uses() {
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
    let main_text = "\
use game::reward::grant as award
pub fn main(amount: i64) -> i64 {
    let first = award(amount)
    return award(first)
}";
    let helper_text = "pub fn grant(amount: i64) -> i64 { return amount }";
    let main_uri = "file:///workspace/scripts/game/main.vela";
    let helper_uri = "file:///workspace/scripts/game/reward.vela";
    for (uri, text) in [(helper_uri, helper_text), (main_uri, main_text)] {
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
    }

    let response = response_value(handle_request(
        &mut server,
        2,
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 2,
                "character": line(main_text, 2)
                    .find("award")
                    .expect("first alias call should exist")
            },
            "context": { "includeDeclaration": true }
        }),
    ));
    let references = response["result"]
        .as_array()
        .expect("references response should be an array");

    assert_eq!(references.len(), 4, "{references:?}");
    assert_reference(
        references,
        helper_uri,
        0,
        helper_text.find("grant").expect("function declaration"),
    );
    assert_reference(
        references,
        main_uri,
        0,
        line(main_text, 0)
            .find("award")
            .expect("import alias should exist"),
    );
    assert_reference(
        references,
        main_uri,
        2,
        line(main_text, 2)
            .find("award")
            .expect("first alias call should exist"),
    );
    assert_reference(
        references,
        main_uri,
        3,
        line(main_text, 3)
            .find("award")
            .expect("second alias call should exist"),
    );
}

#[test]
fn lsp_references_find_imported_source_type_uses() {
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
    let main_uri = "file:///workspace/scripts/game/main.vela";
    let inventory_uri = "file:///workspace/scripts/game/inventory.vela";
    let main_text = "\
use game::inventory::Inventory as Bag

pub const DEFAULT_BAG: Bag = Bag { slots: 2 }

pub fn main(bag: Bag) -> Bag {
    let next: Bag = bag
    return next
}";
    let inventory_text = "\
pub struct Inventory {
    slots: i64
}";
    for (uri, text) in [(inventory_uri, inventory_text), (main_uri, main_text)] {
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
    }

    let response = response_value(handle_request(
        &mut server,
        2,
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 4,
                "character": line(main_text, 4)
                    .find("Bag")
                    .expect("parameter type hint should exist")
            },
            "context": { "includeDeclaration": true }
        }),
    ));
    let references = response["result"]
        .as_array()
        .expect("references response should be an array");

    assert_eq!(references.len(), 6, "{references:?}");
    assert_reference(
        references,
        inventory_uri,
        0,
        line(inventory_text, 0)
            .find("Inventory")
            .expect("type declaration should exist"),
    );
    assert_reference(
        references,
        main_uri,
        0,
        line(main_text, 0)
            .find("Bag")
            .expect("import alias should exist"),
    );
    assert_reference(
        references,
        main_uri,
        2,
        line(main_text, 2)
            .find("Bag")
            .expect("const type hint should exist"),
    );
    assert_reference(
        references,
        main_uri,
        4,
        line(main_text, 4)
            .find("Bag")
            .expect("parameter type hint should exist"),
    );
    assert_reference(
        references,
        main_uri,
        4,
        line(main_text, 4)
            .rfind("Bag")
            .expect("return type hint should exist"),
    );
    assert_reference(
        references,
        main_uri,
        5,
        line(main_text, 5)
            .find("Bag")
            .expect("local type hint should exist"),
    );
}
