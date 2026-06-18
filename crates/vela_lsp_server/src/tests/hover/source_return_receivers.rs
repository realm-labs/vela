use super::*;

#[test]
fn lsp_hover_reports_source_method_on_source_function_return_receiver() {
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
    let mut server = LspServer::new();
    let _ = response_value(server.handle_json(&request(
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": root_uri,
            "capabilities": {}
        }),
    )));
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
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": line,
                "character": character
            }
        }),
    )));

    let value = response["result"]["contents"]["value"]
        .as_str()
        .expect("hover contents should be markdown")
        .to_owned();
    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
    value
}
