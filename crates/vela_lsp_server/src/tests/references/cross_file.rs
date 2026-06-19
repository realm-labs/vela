use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::tests::{
    LspServer, handle_notification, handle_request, notification_value, response_value,
};

use super::{assert_reference, line};

#[test]
fn lsp_references_find_cross_file_imported_source_field_and_method_uses() {
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
    let types_uri = "file:///workspace/scripts/game/types.vela";
    let main_text = "\
use game::types::Reward

pub fn main(reward: Reward) -> i64 {
    let first = reward.amount
    let second = reward.total()
    return first + second + reward.amount + reward.total()
}";
    let types_text = "\
pub struct Reward {
    amount: i64
}

impl Reward {
    pub fn total(self) -> i64 { return 1 }
}";
    for (uri, text) in [(types_uri, types_text), (main_uri, main_text)] {
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

    let field_response = response_value(handle_request(
        &mut server,
        2,
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 3,
                "character": line(main_text, 3)
                    .find("amount")
                    .expect("first field read should exist")
            },
            "context": { "includeDeclaration": true }
        }),
    ));
    let field_references = field_response["result"]
        .as_array()
        .expect("references response should be an array");

    assert_eq!(field_references.len(), 3, "{field_references:?}");
    assert_reference(
        field_references,
        types_uri,
        1,
        line(types_text, 1)
            .find("amount")
            .expect("field declaration should exist"),
    );
    assert_reference(
        field_references,
        main_uri,
        3,
        line(main_text, 3)
            .find("amount")
            .expect("first field read should exist"),
    );
    assert_reference(
        field_references,
        main_uri,
        5,
        line(main_text, 5)
            .find("amount")
            .expect("second field read should exist"),
    );

    let method_response = response_value(handle_request(
        &mut server,
        3,
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 4,
                "character": line(main_text, 4)
                    .find("total")
                    .expect("first method call should exist")
            },
            "context": { "includeDeclaration": true }
        }),
    ));
    let method_references = method_response["result"]
        .as_array()
        .expect("references response should be an array");

    assert_eq!(method_references.len(), 3, "{method_references:?}");
    assert_reference(
        method_references,
        types_uri,
        5,
        line(types_text, 5)
            .find("total")
            .expect("method declaration should exist"),
    );
    assert_reference(
        method_references,
        main_uri,
        4,
        line(main_text, 4)
            .find("total")
            .expect("first method call should exist"),
    );
    assert_reference(
        method_references,
        main_uri,
        5,
        line(main_text, 5)
            .find("total")
            .expect("second method call should exist"),
    );
}

#[test]
fn lsp_references_drop_deleted_imported_source_file() {
    let root = temp_workspace();
    let config_path = root.join("vela.toml");
    let reward_path = root.join("scripts").join("game").join("reward.vela");
    fs::write(
        &config_path,
        r#"
            [workspace]
            roots = ["scripts"]
        "#,
    )
    .expect("vela.toml should be writable");
    fs::write(
        &reward_path,
        "pub fn grant(amount: i64) -> i64 { return amount }",
    )
    .expect("source should be writable");

    let root_uri = file_uri(&root);
    let main_uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    let reward_uri = file_uri(&reward_path);
    let main_text = "\
use game::reward::grant

pub fn main(amount: i64) -> i64 {
    return grant(amount)
}";

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
    let _ = handle_notification(
        &mut server,
        "workspace/didChangeWatchedFiles",
        serde_json::json!({
            "changes": [
                { "uri": file_uri(&config_path), "type": 1 },
                { "uri": reward_uri.clone(), "type": 1 }
            ]
        }),
    );
    let _ = notification_value(handle_notification(
        &mut server,
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": main_uri,
                "languageId": "vela",
                "version": 1,
                "text": main_text
            }
        }),
    ));

    let before = response_value(handle_request(
        &mut server,
        2,
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 3,
                "character": line(main_text, 3)
                    .find("grant")
                    .expect("grant call should exist")
            },
            "context": { "includeDeclaration": true }
        }),
    ));
    let before_references = before["result"]
        .as_array()
        .expect("references response should be an array");
    assert_eq!(before_references.len(), 3, "{before_references:?}");
    assert_reference(before_references, &reward_uri, 0, "pub fn ".len());
    assert_reference(
        before_references,
        &main_uri,
        0,
        line(main_text, 0)
            .find("grant")
            .expect("import should exist"),
    );
    assert_reference(
        before_references,
        &main_uri,
        3,
        line(main_text, 3).find("grant").expect("call should exist"),
    );

    fs::remove_file(&reward_path).expect("source should be removable");
    let _ = handle_notification(
        &mut server,
        "workspace/didChangeWatchedFiles",
        serde_json::json!({
            "changes": [{ "uri": reward_uri, "type": 3 }]
        }),
    );

    let after = response_value(handle_request(
        &mut server,
        3,
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 3,
                "character": line(main_text, 3)
                    .find("grant")
                    .expect("grant call should exist")
            },
            "context": { "includeDeclaration": true }
        }),
    ));
    let after_references = after["result"]
        .as_array()
        .expect("references response should be an array");
    assert!(
        after_references.is_empty(),
        "deleted imported source must not leave stale references: {after_references:?}"
    );

    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_references_refresh_renamed_imported_source_file() {
    let root = temp_workspace();
    let config_path = root.join("vela.toml");
    let reward_path = root.join("scripts").join("game").join("reward.vela");
    let bonus_path = root.join("scripts").join("game").join("bonus.vela");
    fs::write(
        &config_path,
        r#"
            [workspace]
            roots = ["scripts"]
        "#,
    )
    .expect("vela.toml should be writable");
    fs::write(
        &reward_path,
        "pub fn grant(amount: i64) -> i64 { return amount }",
    )
    .expect("source should be writable");

    let root_uri = file_uri(&root);
    let main_uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    let reward_uri = file_uri(&reward_path);
    let bonus_uri = file_uri(&bonus_path);
    let old_main_text = "\
use game::reward::grant

pub fn main(amount: i64) -> i64 {
    return grant(amount)
}";
    let new_main_text = "\
use game::bonus::grant

pub fn main(amount: i64) -> i64 {
    return grant(amount)
}";

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
    let _ = handle_notification(
        &mut server,
        "workspace/didChangeWatchedFiles",
        serde_json::json!({
            "changes": [
                { "uri": file_uri(&config_path), "type": 1 },
                { "uri": reward_uri.clone(), "type": 1 }
            ]
        }),
    );
    let _ = notification_value(handle_notification(
        &mut server,
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": main_uri,
                "languageId": "vela",
                "version": 1,
                "text": old_main_text
            }
        }),
    ));

    let before = response_value(handle_request(
        &mut server,
        2,
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 3,
                "character": line(old_main_text, 3)
                    .find("grant")
                    .expect("grant call should exist")
            },
            "context": { "includeDeclaration": true }
        }),
    ));
    let before_references = before["result"]
        .as_array()
        .expect("references response should be an array");
    assert_reference(before_references, &reward_uri, 0, "pub fn ".len());

    fs::rename(&reward_path, &bonus_path).expect("source should be renameable");
    let _ = handle_notification(
        &mut server,
        "workspace/didChangeWatchedFiles",
        serde_json::json!({
            "changes": [
                { "uri": reward_uri.clone(), "type": 3 },
                { "uri": bonus_uri.clone(), "type": 1 }
            ]
        }),
    );
    let _ = notification_value(handle_notification(
        &mut server,
        "textDocument/didChange",
        serde_json::json!({
            "textDocument": {
                "uri": main_uri,
                "version": 2
            },
            "contentChanges": [
                { "text": new_main_text }
            ]
        }),
    ));

    let after = response_value(handle_request(
        &mut server,
        3,
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 3,
                "character": line(new_main_text, 3)
                    .find("grant")
                    .expect("grant call should exist")
            },
            "context": { "includeDeclaration": true }
        }),
    ));
    let after_references = after["result"]
        .as_array()
        .expect("references response should be an array");
    assert_eq!(after_references.len(), 3, "{after_references:?}");
    assert_reference(after_references, &bonus_uri, 0, "pub fn ".len());
    assert_reference(
        after_references,
        &main_uri,
        0,
        line(new_main_text, 0)
            .find("grant")
            .expect("import should exist"),
    );
    assert_reference(
        after_references,
        &main_uri,
        3,
        line(new_main_text, 3)
            .find("grant")
            .expect("call should exist"),
    );
    assert!(
        after_references
            .iter()
            .all(|reference| reference["uri"] != reward_uri),
        "renamed source must not leave stale references to old URI: {after_references:?}"
    );

    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_references_use_open_overlay_for_imported_defining_file() {
    let root = temp_workspace();
    let config_path = root.join("vela.toml");
    let reward_path = root.join("scripts").join("game").join("reward.vela");
    fs::write(
        &config_path,
        r#"
            [workspace]
            roots = ["scripts"]
        "#,
    )
    .expect("vela.toml should be writable");
    fs::write(
        &reward_path,
        "pub fn stale(amount: i64) -> i64 { return amount }",
    )
    .expect("disk source should be writable");

    let root_uri = file_uri(&root);
    let main_uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    let reward_uri = file_uri(&reward_path);
    let main_text = "\
use game::reward::grant

pub fn main(amount: i64) -> i64 {
    return grant(amount)
}";
    let overlay_text = "pub fn grant(amount: i64) -> i64 { return amount }";

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
    let _ = handle_notification(
        &mut server,
        "workspace/didChangeWatchedFiles",
        serde_json::json!({
            "changes": [
                { "uri": file_uri(&config_path), "type": 1 },
                { "uri": reward_uri.clone(), "type": 1 }
            ]
        }),
    );
    let _ = notification_value(handle_notification(
        &mut server,
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": reward_uri,
                "languageId": "vela",
                "version": 1,
                "text": overlay_text
            }
        }),
    ));
    let _ = notification_value(handle_notification(
        &mut server,
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": main_uri,
                "languageId": "vela",
                "version": 1,
                "text": main_text
            }
        }),
    ));

    let response = response_value(handle_request(
        &mut server,
        2,
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 3,
                "character": line(main_text, 3)
                    .find("grant")
                    .expect("grant call should exist")
            },
            "context": { "includeDeclaration": true }
        }),
    ));
    let references = response["result"]
        .as_array()
        .expect("references response should be an array");
    assert_eq!(references.len(), 3, "{references:?}");
    assert_reference(
        references,
        &reward_uri,
        0,
        overlay_text
            .find("grant")
            .expect("overlay declaration should exist"),
    );
    assert_reference(
        references,
        &main_uri,
        0,
        line(main_text, 0)
            .find("grant")
            .expect("import should exist"),
    );
    assert_reference(
        references,
        &main_uri,
        3,
        line(main_text, 3).find("grant").expect("call should exist"),
    );

    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_references_use_open_overlay_for_importing_file() {
    let root = temp_workspace();
    let config_path = root.join("vela.toml");
    let main_path = root.join("scripts").join("game").join("main.vela");
    let reward_path = root.join("scripts").join("game").join("reward.vela");
    fs::write(
        &config_path,
        r#"
            [workspace]
            roots = ["scripts"]
        "#,
    )
    .expect("vela.toml should be writable");
    fs::write(
        &main_path,
        "\
use game::reward::stale

pub fn main(amount: i64) -> i64 {
    return stale(amount)
}",
    )
    .expect("disk importing source should be writable");
    fs::write(
        &reward_path,
        "pub fn grant(amount: i64) -> i64 { return amount }",
    )
    .expect("defining source should be writable");

    let root_uri = file_uri(&root);
    let main_uri = file_uri(&main_path);
    let reward_uri = file_uri(&reward_path);
    let overlay_text = "\
use game::reward::grant

pub fn main(amount: i64) -> i64 {
    return grant(amount)
}";

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
    let _ = handle_notification(
        &mut server,
        "workspace/didChangeWatchedFiles",
        serde_json::json!({
            "changes": [
                { "uri": file_uri(&config_path), "type": 1 },
                { "uri": main_uri.clone(), "type": 1 },
                { "uri": reward_uri.clone(), "type": 1 }
            ]
        }),
    );
    let _ = notification_value(handle_notification(
        &mut server,
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": main_uri,
                "languageId": "vela",
                "version": 1,
                "text": overlay_text
            }
        }),
    ));

    let response = response_value(handle_request(
        &mut server,
        2,
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 3,
                "character": line(overlay_text, 3)
                    .find("grant")
                    .expect("grant call should exist")
            },
            "context": { "includeDeclaration": true }
        }),
    ));
    let references = response["result"]
        .as_array()
        .expect("references response should be an array");
    assert_eq!(references.len(), 3, "{references:?}");
    assert_reference(references, &reward_uri, 0, "pub fn ".len());
    assert_reference(
        references,
        &main_uri,
        0,
        line(overlay_text, 0)
            .find("grant")
            .expect("overlay import should exist"),
    );
    assert_reference(
        references,
        &main_uri,
        3,
        line(overlay_text, 3)
            .find("grant")
            .expect("overlay call should exist"),
    );

    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_references_find_cross_file_imported_source_enum_variant_uses() {
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
    let types_uri = "file:///workspace/scripts/game/types.vela";
    let main_text = "\
use game::types::QuestState

pub fn active(count: i64) -> QuestState {
    return QuestState::Active { count: count }
}

pub fn main(state: QuestState) -> i64 {
    match state {
        QuestState::Active { count } => { return count }
        QuestState::Done => { return 0 }
    }
}";
    let types_text = "\
pub enum QuestState {
    Active { count: i64 },
    Done
}";
    for (uri, text) in [(types_uri, types_text), (main_uri, main_text)] {
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
                "line": 3,
                "character": line(main_text, 3)
                    .find("Active")
                    .expect("constructor variant should exist")
            },
            "context": { "includeDeclaration": true }
        }),
    ));
    let references = response["result"]
        .as_array()
        .expect("references response should be an array");

    assert_eq!(references.len(), 3, "{references:?}");
    assert_reference(
        references,
        types_uri,
        1,
        line(types_text, 1)
            .find("Active")
            .expect("variant declaration should exist"),
    );
    assert_reference(
        references,
        main_uri,
        3,
        line(main_text, 3)
            .find("Active")
            .expect("constructor variant should exist"),
    );
    assert_reference(
        references,
        main_uri,
        8,
        line(main_text, 8)
            .find("Active")
            .expect("pattern variant should exist"),
    );
}

#[test]
fn lsp_references_find_cross_file_imported_source_enum_record_variant_field_uses() {
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
    let types_uri = "file:///workspace/scripts/game/types.vela";
    let main_text = "\
use game::types::QuestState

pub fn active(count: i64) -> QuestState {
    return QuestState::Active { count: count }
}

pub fn main(state: QuestState) -> i64 {
    match state {
        QuestState::Active { count: current } => { return current }
        QuestState::Done => { return 0 }
    }
}";
    let types_text = "\
pub enum QuestState {
    Active { count: i64 },
    Done
}";
    for (uri, text) in [(types_uri, types_text), (main_uri, main_text)] {
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
                "line": 3,
                "character": line(main_text, 3)
                    .find("count")
                    .expect("constructor field should exist")
            },
            "context": { "includeDeclaration": true }
        }),
    ));
    let references = response["result"]
        .as_array()
        .expect("references response should be an array");

    assert_eq!(references.len(), 3, "{references:?}");
    assert_reference(
        references,
        types_uri,
        1,
        line(types_text, 1)
            .find("count")
            .expect("variant field declaration should exist"),
    );
    assert_reference(
        references,
        main_uri,
        3,
        line(main_text, 3)
            .find("count")
            .expect("constructor field should exist"),
    );
    assert_reference(
        references,
        main_uri,
        8,
        line(main_text, 8)
            .find("count")
            .expect("pattern field should exist"),
    );
}

fn temp_workspace() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let suffix = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(error) => panic!("system time should be after UNIX_EPOCH: {error}"),
    };
    let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "vela_lsp_references_cross_file_{}_{}_{}",
        std::process::id(),
        suffix,
        sequence
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
