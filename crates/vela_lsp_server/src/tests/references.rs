use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::{LspServer, notification, notification_value, request, response_value};

mod modules;
mod schema;
mod schema_fields;
mod schema_variant_fields;

#[test]
fn lsp_references_find_local_binding_uses() {
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
    let text = "\
pub fn main(amount: i64) -> i64 {
    let next = amount + 1
    return next + amount
}";
    let uri = "file:///workspace/scripts/game/main.vela";
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
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 2,
                "character": line(text, 2).find("amount").expect("amount use")
            },
            "context": { "includeDeclaration": true }
        }),
    )));
    let references = response["result"]
        .as_array()
        .expect("references response should be an array");

    assert_eq!(references.len(), 3);
    assert_reference(
        references,
        uri,
        0,
        line(text, 0).find("amount").expect("parameter declaration"),
    );
    assert_reference(
        references,
        uri,
        1,
        line(text, 1).find("amount").expect("first read"),
    );
    assert_reference(
        references,
        uri,
        2,
        line(text, 2).find("amount").expect("second read"),
    );
}

#[test]
fn lsp_references_find_imported_function_uses() {
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
    let main_text = "\
use game::reward::grant
pub fn main(amount: i64) -> i64 {
    let first = grant(amount)
    return grant(first)
}";
    let helper_text = "pub fn grant(amount: i64) -> i64 { return amount }";
    let main_uri = "file:///workspace/scripts/game/main.vela";
    let helper_uri = "file:///workspace/scripts/game/reward.vela";
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": helper_uri,
                "languageId": "vela",
                "version": 1,
                "text": helper_text
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

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 2,
                "character": line(main_text, 2).find("grant").expect("grant call")
            },
            "context": { "includeDeclaration": true }
        }),
    )));
    let references = response["result"]
        .as_array()
        .expect("references response should be an array");

    assert_eq!(references.len(), 4);
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
        line(main_text, 0).find("grant").expect("import"),
    );
    assert_reference(
        references,
        main_uri,
        2,
        line(main_text, 2).find("grant").expect("first call"),
    );
    assert_reference(
        references,
        main_uri,
        3,
        line(main_text, 3).find("grant").expect("second call"),
    );
}

#[test]
fn lsp_references_find_field_reads_and_writes() {
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
    let text = "\
pub struct Reward {
    amount: i64
}

pub fn main(reward: Reward) -> i64 {
    let first = reward.amount
    reward.amount += 1
    return reward.amount + first
}";
    let uri = "file:///workspace/scripts/game/main.vela";
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
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 5,
                "character": line(text, 5).find("amount").expect("first field read")
            },
            "context": { "includeDeclaration": true }
        }),
    )));
    let references = response["result"]
        .as_array()
        .expect("references response should be an array");

    assert_eq!(references.len(), 4);
    assert_reference(
        references,
        uri,
        1,
        line(text, 1).find("amount").expect("field declaration"),
    );
    assert_reference(
        references,
        uri,
        5,
        line(text, 5).find("amount").expect("first field read"),
    );
    assert_reference(
        references,
        uri,
        6,
        line(text, 6).find("amount").expect("field write"),
    );
    assert_reference(
        references,
        uri,
        7,
        line(text, 7).find("amount").expect("second field read"),
    );
}

#[test]
fn lsp_references_find_record_constructor_field_labels() {
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
    let text = "\
pub struct Reward {
    amount: i64
}

pub fn make(amount: i64) -> Reward {
    return Reward { amount: amount }
}

pub fn main(reward: Reward) -> i64 {
    return reward.amount
}";
    let uri = "file:///workspace/scripts/game/main.vela";
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
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 5,
                "character": line(text, 5).find("amount").expect("constructor field label")
            },
            "context": { "includeDeclaration": true }
        }),
    )));
    let references = response["result"]
        .as_array()
        .expect("references response should be an array");

    assert_eq!(references.len(), 3, "{references:?}");
    assert_reference(
        references,
        uri,
        1,
        line(text, 1).find("amount").expect("field declaration"),
    );
    assert_reference(
        references,
        uri,
        5,
        line(text, 5)
            .find("amount")
            .expect("constructor field label"),
    );
    assert_reference(
        references,
        uri,
        9,
        line(text, 9).find("amount").expect("member field read"),
    );
}

#[test]
fn lsp_references_find_record_constructor_shorthand_field_labels() {
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
    let text = "\
pub struct Reward {
    amount: i64
}

pub fn make(amount: i64) -> Reward {
    return Reward { amount }
}

pub fn main(reward: Reward) -> i64 {
    return reward.amount
}";
    let uri = "file:///workspace/scripts/game/main.vela";
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
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 1,
                "character": line(text, 1).find("amount").expect("field declaration")
            },
            "context": { "includeDeclaration": true }
        }),
    )));
    let references = response["result"]
        .as_array()
        .expect("references response should be an array");

    assert_eq!(references.len(), 3, "{references:?}");
    assert_reference(
        references,
        uri,
        1,
        line(text, 1).find("amount").expect("field declaration"),
    );
    assert_reference(
        references,
        uri,
        5,
        line(text, 5)
            .find("amount")
            .expect("constructor shorthand field label"),
    );
    assert_reference(
        references,
        uri,
        9,
        line(text, 9).find("amount").expect("member field read"),
    );
}

#[test]
fn lsp_references_find_enum_variant_constructors_and_patterns() {
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
    let text = "\
pub enum QuestState {
    Active { count: i64 },
    Done
}

pub fn active(count: i64) -> QuestState {
    return QuestState::Active { count: count }
}

pub fn main(state: QuestState) -> i64 {
    match state {
        QuestState::Active { count } => { return count }
        QuestState::Done => { return 0 }
    }
}";
    let uri = "file:///workspace/scripts/game/main.vela";
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
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 6,
                "character": line(text, 6).find("Active").expect("Active constructor use")
            },
            "context": { "includeDeclaration": true }
        }),
    )));
    let references = response["result"]
        .as_array()
        .expect("references response should be an array");

    assert_eq!(references.len(), 3);
    assert_reference(
        references,
        uri,
        1,
        line(text, 1).find("Active").expect("Active declaration"),
    );
    assert_reference(
        references,
        uri,
        6,
        line(text, 6)
            .find("Active")
            .expect("Active constructor use"),
    );
    assert_reference(
        references,
        uri,
        11,
        line(text, 11).find("Active").expect("Active pattern use"),
    );
}

#[test]
fn lsp_references_find_enum_record_variant_field_labels_and_patterns() {
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
    let text = "\
pub enum QuestState {
    Active { count: i64 },
    Done
}

pub fn active(count: i64) -> QuestState {
    return QuestState::Active { count: count }
}

pub fn main(state: QuestState) -> i64 {
    match state {
        QuestState::Active { count: current } => { return current }
        QuestState::Done => { return 0 }
    }
}";
    let uri = "file:///workspace/scripts/game/main.vela";
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
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 1,
                "character": line(text, 1).find("count").expect("field declaration")
            },
            "context": { "includeDeclaration": true }
        }),
    )));
    let references = response["result"]
        .as_array()
        .expect("references response should be an array");

    assert_eq!(references.len(), 3, "{references:?}");
    assert_reference(
        references,
        uri,
        1,
        line(text, 1).find("count").expect("field declaration"),
    );
    assert_reference(
        references,
        uri,
        6,
        line(text, 6)
            .find("count")
            .expect("constructor field label"),
    );
    assert_reference(
        references,
        uri,
        11,
        line(text, 11).find("count").expect("pattern field label"),
    );
}

#[test]
fn lsp_references_find_script_method_calls() {
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
    let text = "\
pub struct Reward {
    amount: i64
}

impl Reward {
    pub fn grant(self, amount: i64) -> i64 { return amount }
}

pub fn main(reward: Reward) -> i64 {
    let first = reward.grant(1)
    return reward.grant(first)
}";
    let uri = "file:///workspace/scripts/game/main.vela";
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
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 9,
                "character": line(text, 9).find("grant").expect("first method call")
            },
            "context": { "includeDeclaration": true }
        }),
    )));
    let references = response["result"]
        .as_array()
        .expect("references response should be an array");

    assert_eq!(references.len(), 3);
    assert_reference(
        references,
        uri,
        5,
        line(text, 5).find("grant").expect("method declaration"),
    );
    assert_reference(
        references,
        uri,
        9,
        line(text, 9).find("grant").expect("first method call"),
    );
    assert_reference(
        references,
        uri,
        10,
        line(text, 10).find("grant").expect("second method call"),
    );
}

#[test]
fn lsp_references_find_trait_impl_uses() {
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
    let text = "\
pub trait Rewardable {
    fn grant(self, amount: i64) -> i64;
}

pub struct Player {
    level: i64
}

pub struct Chest {
    amount: i64
}

impl Rewardable for Player {
    fn grant(self, amount: i64) -> i64 { return amount }
}

impl Rewardable for Chest {
    fn grant(self, amount: i64) -> i64 { return amount }
}";
    let uri = "file:///workspace/scripts/game/main.vela";
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
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 12,
                "character": line(text, 12).find("Rewardable").expect("first impl use")
            },
            "context": { "includeDeclaration": true }
        }),
    )));
    let references = response["result"]
        .as_array()
        .expect("references response should be an array");

    assert_eq!(references.len(), 3);
    assert_reference(
        references,
        uri,
        0,
        line(text, 0).find("Rewardable").expect("trait declaration"),
    );
    assert_reference(
        references,
        uri,
        12,
        line(text, 12).find("Rewardable").expect("first impl use"),
    );
    assert_reference(
        references,
        uri,
        16,
        line(text, 16).find("Rewardable").expect("second impl use"),
    );
}

#[test]
fn lsp_document_highlight_marks_local_declaration_and_reads() {
    let mut server = LspServer::new();
    let initialize = response_value(server.handle_json(&request(
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": "file:///workspace/scripts",
            "capabilities": {}
        }),
    )));
    assert_eq!(
        initialize["result"]["capabilities"]["documentHighlightProvider"],
        true
    );
    let text = "\
pub fn main(amount: i64) -> i64 {
    let next = amount + 1
    return next + amount
}";
    let uri = "file:///workspace/scripts/game/main.vela";
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
        "textDocument/documentHighlight",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 2,
                "character": line(text, 2).find("amount").expect("amount use")
            }
        }),
    )));
    let highlights = response["result"]
        .as_array()
        .expect("documentHighlight response should be an array");

    assert_eq!(highlights.len(), 3);
    assert_highlight(
        highlights,
        0,
        line(text, 0).find("amount").expect("parameter declaration"),
        1,
    );
    assert_highlight(
        highlights,
        1,
        line(text, 1).find("amount").expect("first read"),
        2,
    );
    assert_highlight(
        highlights,
        2,
        line(text, 2).find("amount").expect("second read"),
        2,
    );
}

#[test]
fn lsp_document_highlight_marks_import_and_calls_in_active_document() {
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
    let main_text = "\
use game::reward::grant
pub fn main(amount: i64) -> i64 {
    let first = grant(amount)
    return grant(first)
}";
    let helper_text = "pub fn grant(amount: i64) -> i64 { return amount }";
    let main_uri = "file:///workspace/scripts/game/main.vela";
    let helper_uri = "file:///workspace/scripts/game/reward.vela";
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": helper_uri,
                "languageId": "vela",
                "version": 1,
                "text": helper_text
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

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/documentHighlight",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 2,
                "character": line(main_text, 2).find("grant").expect("grant call")
            }
        }),
    )));
    let highlights = response["result"]
        .as_array()
        .expect("documentHighlight response should be an array");

    assert_eq!(highlights.len(), 3);
    assert_highlight(
        highlights,
        0,
        line(main_text, 0).find("grant").expect("import"),
        1,
    );
    assert_highlight(
        highlights,
        2,
        line(main_text, 2).find("grant").expect("first call"),
        1,
    );
    assert_highlight(
        highlights,
        3,
        line(main_text, 3).find("grant").expect("second call"),
        1,
    );
}

#[test]
fn lsp_document_highlight_marks_read_write_call() {
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
    let text = "\
pub fn grant(amount: i64) -> i64 { return amount }
pub fn main(amount: i64) -> i64 {
    let score = amount
    score += grant(amount)
    return score + grant(score)
}";
    let uri = "file:///workspace/scripts/game/main.vela";
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

    let score_response = response_value(server.handle_json(&request(
        2,
        "textDocument/documentHighlight",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 3,
                "character": line(text, 3).find("score").expect("score write")
            }
        }),
    )));
    let score_highlights = score_response["result"]
        .as_array()
        .expect("documentHighlight response should be an array");

    assert_eq!(score_highlights.len(), 4);
    assert_highlight(
        score_highlights,
        2,
        line(text, 2).find("score").expect("score declaration"),
        1,
    );
    assert_highlight(
        score_highlights,
        3,
        line(text, 3).find("score").expect("score write"),
        3,
    );
    assert_highlight(
        score_highlights,
        4,
        line(text, 4).find("score").expect("score read"),
        2,
    );

    let grant_response = response_value(server.handle_json(&request(
        3,
        "textDocument/documentHighlight",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 3,
                "character": line(text, 3).find("grant").expect("grant call")
            }
        }),
    )));
    let grant_highlights = grant_response["result"]
        .as_array()
        .expect("documentHighlight response should be an array");

    assert_eq!(grant_highlights.len(), 3);
    assert_highlight(
        grant_highlights,
        0,
        line(text, 0).find("grant").expect("grant declaration"),
        1,
    );
    assert_highlight(
        grant_highlights,
        3,
        line(text, 3).find("grant").expect("first grant call"),
        1,
    );
    assert_highlight(
        grant_highlights,
        4,
        line(text, 4).find("grant").expect("second grant call"),
        1,
    );
}

#[test]
fn lsp_document_highlight_marks_script_method_calls() {
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
    let text = "\
pub struct Reward {
    amount: i64
}

impl Reward {
    pub fn grant(self, amount: i64) -> i64 { return amount }
}

pub fn main(reward: Reward) -> i64 {
    let first = reward.grant(1)
    return reward.grant(first)
}";
    let uri = "file:///workspace/scripts/game/main.vela";
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
        "textDocument/documentHighlight",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 9,
                "character": line(text, 9).find("grant").expect("first method call")
            }
        }),
    )));
    let highlights = response["result"]
        .as_array()
        .expect("documentHighlight response should be an array");

    assert_eq!(highlights.len(), 3);
    assert_highlight(
        highlights,
        5,
        line(text, 5).find("grant").expect("method declaration"),
        1,
    );
    assert_highlight(
        highlights,
        9,
        line(text, 9).find("grant").expect("first method call"),
        1,
    );
    assert_highlight(
        highlights,
        10,
        line(text, 10).find("grant").expect("second method call"),
        1,
    );
}

#[test]
fn lsp_document_highlight_marks_trait_impl_uses() {
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
    let text = "\
pub trait Rewardable {
    fn grant(self, amount: i64) -> i64;
}

pub struct Player { level: i64 }

impl Rewardable for Player {
    fn grant(self, amount: i64) -> i64 { return amount }
}";
    let uri = "file:///workspace/scripts/game/main.vela";
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
        "textDocument/documentHighlight",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 6,
                "character": line(text, 6).find("Rewardable").expect("impl use")
            }
        }),
    )));
    let highlights = response["result"]
        .as_array()
        .expect("documentHighlight response should be an array");

    assert_eq!(highlights.len(), 2, "{highlights:?}");
    assert_highlight(
        highlights,
        0,
        line(text, 0).find("Rewardable").expect("trait declaration"),
        1,
    );
    assert_highlight(
        highlights,
        6,
        line(text, 6).find("Rewardable").expect("impl use"),
        2,
    );
}

fn assert_reference(references: &[serde_json::Value], uri: &str, line: usize, character: usize) {
    assert!(
        references.iter().any(|reference| {
            reference["uri"] == uri
                && reference["range"]["start"]["line"] == line
                && reference["range"]["start"]["character"] == character
        }),
        "{references:?}"
    );
}

fn assert_highlight(highlights: &[serde_json::Value], line: usize, character: usize, kind: u8) {
    assert!(
        highlights.iter().any(|highlight| {
            highlight["range"]["start"]["line"] == line
                && highlight["range"]["start"]["character"] == character
                && highlight["kind"] == kind
        }),
        "{highlights:?}"
    );
}

fn line(text: &str, line: usize) -> &str {
    text.lines().nth(line).expect("line should exist")
}

fn temp_workspace() -> PathBuf {
    let suffix = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(error) => panic!("system time should be after UNIX_EPOCH: {error}"),
    };
    let root = std::env::temp_dir().join(format!(
        "vela_lsp_server_refs_{}_{}",
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
