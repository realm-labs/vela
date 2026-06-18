use super::{LspServer, notification, notification_value, request, response_value};

#[test]
fn lsp_document_formatting_returns_full_document_edit() {
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
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": "pub fn main() {   \n    return 1\t\n}"
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/formatting",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "options": { "tabSize": 4, "insertSpaces": true }
        }),
    )));
    let edits = response["result"]
        .as_array()
        .expect("formatting should return edits");

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0]["range"]["start"]["line"], 0);
    assert_eq!(edits[0]["range"]["start"]["character"], 0);
    assert_eq!(edits[0]["range"]["end"]["line"], 2);
    assert_eq!(edits[0]["range"]["end"]["character"], 1);
    assert_eq!(edits[0]["newText"], "pub fn main() {\n    return 1\n}\n");
}

#[test]
fn lsp_document_formatting_returns_empty_edits_when_idempotent() {
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
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": "pub fn main() {\n    return 1\n}\n"
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/formatting",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "options": { "tabSize": 4, "insertSpaces": true }
        }),
    )));

    assert_eq!(response["result"], serde_json::json!([]));
}

#[test]
fn lsp_document_formatting_formats_declarations() {
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
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": "pub struct Player{level:i64 name:String}impl Player{fn heal(amount:i64)->i64{return amount}}"
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/formatting",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "options": { "tabSize": 4, "insertSpaces": true }
        }),
    )));
    let edits = response["result"]
        .as_array()
        .expect("formatting should return edits");

    assert_eq!(edits.len(), 1);
    assert_eq!(
        edits[0]["newText"],
        "\
pub struct Player {
    level: i64
    name: String
}
impl Player {
    fn heal(amount: i64) -> i64 {
        return amount
    }
}
"
    );
}

#[test]
fn lsp_document_formatting_formats_container_type_hint_example() {
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
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": "\
fn load_rewards(rewards:Map < String,i64 >)->Result < Map<String , i64>,String >{return result::ok(rewards)}

fn main(){let scores:Array < i64 > = [1,2,3];let rewards:Map < String,i64 >={\"xp\":5};let tags:Set < String > = set::from_array([\"daily\",\"vip\"]);return score(scores,rewards,tags).unwrap_or(0)}
"
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/formatting",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "options": { "tabSize": 4, "insertSpaces": true }
        }),
    )));
    let edits = response["result"]
        .as_array()
        .expect("formatting should return edits");

    assert_eq!(edits.len(), 1);
    assert_eq!(
        edits[0]["newText"],
        "\
fn load_rewards(rewards: Map<String, i64>) -> Result<Map<String, i64>, String> {
    return result::ok(rewards)
}

fn main() {
    let scores: Array<i64> = [1, 2, 3];
    let rewards: Map<String, i64> = {
        \"xp\": 5
    };
    let tags: Set<String> = set::from_array([\"daily\", \"vip\"]);
    return score(scores, rewards, tags).unwrap_or(0)
}
"
    );
}

#[test]
fn lsp_range_formatting_limits_edits_to_range() {
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
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": "pub fn main() {   \n    return 1   \n}\n"
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/rangeFormatting",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 1, "character": 0 },
                "end": { "line": 2, "character": 0 }
            },
            "options": { "tabSize": 4, "insertSpaces": true }
        }),
    )));
    let edits = response["result"]
        .as_array()
        .expect("rangeFormatting should return edits");

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0]["range"]["start"]["line"], 1);
    assert_eq!(edits[0]["range"]["start"]["character"], 12);
    assert_eq!(edits[0]["range"]["end"]["line"], 1);
    assert_eq!(edits[0]["range"]["end"]["character"], 15);
    assert_eq!(edits[0]["newText"], "");
}

#[test]
fn lsp_range_formatting_formats_selected_item() {
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
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": "pub fn main(){return 1}\n\npub fn other(){return 2}\n"
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/rangeFormatting",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 1, "character": 0 }
            },
            "options": { "tabSize": 4, "insertSpaces": true }
        }),
    )));
    let edits = response["result"]
        .as_array()
        .expect("rangeFormatting should return edits");

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0]["range"]["start"]["line"], 0);
    assert_eq!(edits[0]["range"]["start"]["character"], 0);
    assert_eq!(edits[0]["range"]["end"]["line"], 1);
    assert_eq!(edits[0]["range"]["end"]["character"], 0);
    assert_eq!(edits[0]["newText"], "pub fn main() {\n    return 1\n}\n");
}

#[test]
fn lsp_range_formatting_formats_item_with_leading_blank_selection() {
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
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": "\n\npub fn main(){return 1}\n\npub fn other(){return 2}\n"
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/rangeFormatting",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 3, "character": 0 }
            },
            "options": { "tabSize": 4, "insertSpaces": true }
        }),
    )));
    let edits = response["result"]
        .as_array()
        .expect("rangeFormatting should return edits");

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0]["range"]["start"]["line"], 2);
    assert_eq!(edits[0]["range"]["start"]["character"], 0);
    assert_eq!(edits[0]["range"]["end"]["line"], 3);
    assert_eq!(edits[0]["range"]["end"]["character"], 0);
    assert_eq!(edits[0]["newText"], "pub fn main() {\n    return 1\n}\n");
}

#[test]
fn lsp_range_formatting_formats_selected_impl_method() {
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
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": "impl Player{fn heal(amount:i64)->i64{return amount}fn hurt(amount:i64)->i64{return amount}}\n"
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/rangeFormatting",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 12 },
                "end": { "line": 0, "character": 51 }
            },
            "options": { "tabSize": 4, "insertSpaces": true }
        }),
    )));
    let edits = response["result"]
        .as_array()
        .expect("rangeFormatting should return edits");

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0]["range"]["start"]["line"], 0);
    assert_eq!(edits[0]["range"]["start"]["character"], 12);
    assert_eq!(edits[0]["range"]["end"]["line"], 0);
    assert_eq!(edits[0]["range"]["end"]["character"], 51);
    assert_eq!(
        edits[0]["newText"],
        "fn heal(amount: i64) -> i64 {\n    return amount\n}\n"
    );
}

#[test]
fn lsp_range_formatting_formats_selected_trait_method() {
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
    let text = "pub trait Rewardable{fn preview(amount:i64)->i64 fn other(amount:i64)->i64}\n";
    let start = text.find("fn preview").expect("selected method");
    let end = start + "fn preview(amount:i64)->i64".len();
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
        "textDocument/rangeFormatting",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": start },
                "end": { "line": 0, "character": end }
            },
            "options": { "tabSize": 4, "insertSpaces": true }
        }),
    )));
    let edits = response["result"]
        .as_array()
        .expect("rangeFormatting should return edits");

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0]["range"]["start"]["line"], 0);
    assert_eq!(edits[0]["range"]["start"]["character"], start);
    assert_eq!(edits[0]["range"]["end"]["line"], 0);
    assert_eq!(edits[0]["range"]["end"]["character"], end);
    assert_eq!(edits[0]["newText"], "fn preview(amount: i64) -> i64");
}

#[test]
fn lsp_range_formatting_preserves_nested_method_indent() {
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
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": "\
impl Player {
    fn heal(amount:i64)->i64{return amount}
    fn hurt(amount:i64)->i64{return amount}
}
"
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/rangeFormatting",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 1, "character": 4 },
                "end": { "line": 1, "character": 43 }
            },
            "options": { "tabSize": 4, "insertSpaces": true }
        }),
    )));
    let edits = response["result"]
        .as_array()
        .expect("rangeFormatting should return edits");

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0]["range"]["start"]["line"], 1);
    assert_eq!(edits[0]["range"]["start"]["character"], 4);
    assert_eq!(edits[0]["range"]["end"]["line"], 1);
    assert_eq!(edits[0]["range"]["end"]["character"], 43);
    assert_eq!(
        edits[0]["newText"],
        "fn heal(amount: i64) -> i64 {\n        return amount\n    }"
    );
}

#[test]
fn lsp_range_formatting_preserves_struct_field_indent() {
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
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": "\
pub struct Player {
    level:i64
    name:String
}
"
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/rangeFormatting",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 1, "character": 4 },
                "end": { "line": 1, "character": 13 }
            },
            "options": { "tabSize": 4, "insertSpaces": true }
        }),
    )));
    let edits = response["result"]
        .as_array()
        .expect("rangeFormatting should return edits");

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0]["range"]["start"]["line"], 1);
    assert_eq!(edits[0]["range"]["start"]["character"], 4);
    assert_eq!(edits[0]["range"]["end"]["line"], 1);
    assert_eq!(edits[0]["range"]["end"]["character"], 13);
    assert_eq!(edits[0]["newText"], "level: i64");
}

#[test]
fn lsp_range_formatting_formats_selected_struct_field_group() {
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
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": "\
pub struct Player {
    level:i64
    name:String
    xp:i64
}
"
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/rangeFormatting",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 1, "character": 4 },
                "end": { "line": 2, "character": 15 }
            },
            "options": { "tabSize": 4, "insertSpaces": true }
        }),
    )));
    let edits = response["result"]
        .as_array()
        .expect("rangeFormatting should return edits");

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0]["range"]["start"]["line"], 1);
    assert_eq!(edits[0]["range"]["start"]["character"], 4);
    assert_eq!(edits[0]["range"]["end"]["line"], 2);
    assert_eq!(edits[0]["range"]["end"]["character"], 15);
    assert_eq!(edits[0]["newText"], "level: i64\n    name: String");
}

#[test]
fn lsp_range_formatting_formats_selected_enum_record_field_group() {
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
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": "\
pub enum Reward {
    Coins {
        amount:i64
        label:String
        rare:bool
    }
    None
}
"
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/rangeFormatting",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 2, "character": 8 },
                "end": { "line": 3, "character": 20 }
            },
            "options": { "tabSize": 4, "insertSpaces": true }
        }),
    )));
    let edits = response["result"]
        .as_array()
        .expect("rangeFormatting should return edits");

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0]["range"]["start"]["line"], 2);
    assert_eq!(edits[0]["range"]["start"]["character"], 8);
    assert_eq!(edits[0]["range"]["end"]["line"], 3);
    assert_eq!(edits[0]["range"]["end"]["character"], 20);
    assert_eq!(edits[0]["newText"], "amount: i64\n        label: String");
}

#[test]
fn lsp_on_type_formatting_only_edits_current_construct() {
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
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": "\
pub fn main() {   
    return 1   
}

pub fn other() {   
    return 2   
}
"
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/onTypeFormatting",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 2, "character": 1 },
            "ch": "}",
            "options": { "tabSize": 4, "insertSpaces": true }
        }),
    )));
    let edits = response["result"]
        .as_array()
        .expect("onTypeFormatting should return edits");

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0]["range"]["start"]["line"], 0);
    assert_eq!(edits[0]["range"]["start"]["character"], 0);
    assert_eq!(edits[0]["range"]["end"]["line"], 3);
    assert_eq!(edits[0]["range"]["end"]["character"], 0);
    assert_eq!(edits[0]["newText"], "pub fn main() {\n    return 1\n}\n");
}

#[test]
fn lsp_on_type_formatting_reflows_completed_item() {
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
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": "pub fn main(){return 1}\n\npub fn other(){return 2}\n"
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/onTypeFormatting",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 0, "character": 23 },
            "ch": "}",
            "options": { "tabSize": 4, "insertSpaces": true }
        }),
    )));
    let edits = response["result"]
        .as_array()
        .expect("onTypeFormatting should return edits");

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0]["range"]["start"]["line"], 0);
    assert_eq!(edits[0]["range"]["start"]["character"], 0);
    assert_eq!(edits[0]["range"]["end"]["line"], 1);
    assert_eq!(edits[0]["range"]["end"]["character"], 0);
    assert_eq!(edits[0]["newText"], "pub fn main() {\n    return 1\n}\n");
}

#[test]
fn lsp_on_type_formatting_reflows_completed_multiline_item() {
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
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": "\
pub fn main(){
    return 1+2
}

pub fn other(){return 2}
"
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/onTypeFormatting",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 2, "character": 1 },
            "ch": "}",
            "options": { "tabSize": 4, "insertSpaces": true }
        }),
    )));
    let edits = response["result"]
        .as_array()
        .expect("onTypeFormatting should return edits");

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0]["range"]["start"]["line"], 0);
    assert_eq!(edits[0]["range"]["start"]["character"], 0);
    assert_eq!(edits[0]["range"]["end"]["line"], 3);
    assert_eq!(edits[0]["range"]["end"]["character"], 0);
    assert_eq!(
        edits[0]["newText"],
        "pub fn main() {\n    return 1 + 2\n}\n"
    );
}

#[test]
fn lsp_on_type_formatting_reflows_completed_nested_method() {
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
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": "\
impl Player {
    fn heal(amount:i64)->i64{return amount}
    fn hurt(amount:i64)->i64{return amount}
}
"
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/onTypeFormatting",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 1, "character": 43 },
            "ch": "}",
            "options": { "tabSize": 4, "insertSpaces": true }
        }),
    )));
    let edits = response["result"]
        .as_array()
        .expect("onTypeFormatting should return edits");

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0]["range"]["start"]["line"], 1);
    assert_eq!(edits[0]["range"]["start"]["character"], 4);
    assert_eq!(edits[0]["range"]["end"]["line"], 2);
    assert_eq!(edits[0]["range"]["end"]["character"], 0);
    assert_eq!(
        edits[0]["newText"],
        "fn heal(amount: i64) -> i64 {\n        return amount\n    }\n"
    );
}

#[test]
fn lsp_on_type_formatting_reflows_completed_enum_record_variant() {
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
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": "\
pub enum Reward {
    Coins {
        amount:i64
        label:String
    }
    None
}
"
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/onTypeFormatting",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 4, "character": 5 },
            "ch": "}",
            "options": { "tabSize": 4, "insertSpaces": true }
        }),
    )));
    let edits = response["result"]
        .as_array()
        .expect("onTypeFormatting should return edits");

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0]["range"]["start"]["line"], 1);
    assert_eq!(edits[0]["range"]["start"]["character"], 4);
    assert_eq!(edits[0]["range"]["end"]["line"], 5);
    assert_eq!(edits[0]["range"]["end"]["character"], 0);
    assert_eq!(
        edits[0]["newText"],
        "Coins {\n        amount: i64\n        label: String\n    }\n"
    );
}
