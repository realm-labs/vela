use crate::tests::{
    LspServer, handle_notification, handle_request, notification_value, response_value,
};

#[test]
fn lsp_hover_reports_imported_function_const_and_global_facts() {
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
use game::rewards::reward_bonus
pub fn main(amount: i64) -> i64 {
    let first = BASE_REWARD
    let scaled = reward_bonus(first, reward_scale)
    return scaled + amount
}";
    let rewards_text = r#"#[doc("Base reward amount")]
pub const BASE_REWARD: i64 = 4
#[doc("Current reward scale")]
pub global reward_scale: i64
#[doc("Compute reward bonus")]
pub fn reward_bonus(amount: i64, scale: i64 = reward_scale) -> i64 {
    return amount * scale
}"#;
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

    let const_hover = hover_at(
        &mut server,
        main_uri,
        2,
        4,
        line(main_text, 4)
            .find("BASE_REWARD")
            .expect("const use should exist"),
    );
    assert!(
        const_hover.contains("game::rewards::BASE_REWARD"),
        "{const_hover}"
    );
    assert!(const_hover.contains("_const_: i64"), "{const_hover}");
    assert!(const_hover.contains("Base reward amount"), "{const_hover}");

    let function_hover = hover_at(
        &mut server,
        main_uri,
        3,
        5,
        line(main_text, 5)
            .find("reward_bonus")
            .expect("function call should exist"),
    );
    assert!(
        function_hover.contains("game::rewards::reward_bonus"),
        "{function_hover}"
    );
    assert!(
        function_hover.contains("_function_: (amount: i64, scale: i64) -> i64"),
        "{function_hover}"
    );
    assert!(
        function_hover.contains("Compute reward bonus"),
        "{function_hover}"
    );

    let global_hover = hover_at(
        &mut server,
        main_uri,
        4,
        5,
        line(main_text, 5)
            .find("reward_scale")
            .expect("global use should exist"),
    );
    assert!(
        global_hover.contains("game::rewards::reward_scale"),
        "{global_hover}"
    );
    assert!(global_hover.contains("_global_: i64"), "{global_hover}");
    assert!(
        global_hover.contains("Current reward scale"),
        "{global_hover}"
    );
}

fn hover_at(server: &mut LspServer, uri: &str, id: i32, line: usize, character: usize) -> String {
    let response = response_value(handle_request(
        server,
        id,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": line,
                "character": character
            }
        }),
    ));

    response["result"]["contents"]["value"]
        .as_str()
        .expect("hover contents should be markdown")
        .to_owned()
}

fn line(text: &str, line: usize) -> &str {
    text.lines().nth(line).expect("line should exist")
}
