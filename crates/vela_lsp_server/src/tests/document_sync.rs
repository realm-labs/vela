use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::{TestServer, assert_no_messages, notification_value, notify, request, response_value};

fn temp_workspace() -> PathBuf {
    let suffix = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(error) => panic!("system time should be after UNIX_EPOCH: {error}"),
    };
    let root = std::env::temp_dir().join(format!(
        "vela_lsp_server_sync_{}_{}",
        std::process::id(),
        suffix
    ));
    if let Err(error) = fs::create_dir_all(root.join("scripts").join("game")) {
        panic!("temporary workspace should be creatable: {error}");
    }
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

fn initialize_server(server: &mut TestServer) {
    let _ = response_value(request::<lsp_types::request::Initialize>(
        server,
        0,
        serde_json::json!({
            "processId": null,
            "capabilities": {}
        }),
    ));
}

#[test]
fn lsp_did_open_publishes_diagnostics() {
    let mut server = TestServer::new();
    initialize_server(&mut server);
    let notification = notification_value(notify::<lsp_types::notification::DidOpenTextDocument>(
        &mut server,
        serde_json::json!({
            "textDocument": {
                "uri": "file:///workspace/main.vela",
                "languageId": "vela",
                "version": 1,
                "text": "pub fn main(scores: Array<i64>) { return scores.frist() }"
            }
        }),
    ));

    assert_eq!(notification["jsonrpc"], "2.0");
    assert_eq!(notification["method"], "textDocument/publishDiagnostics");
    assert_eq!(notification["params"]["uri"], "file:///workspace/main.vela");
    let Some(diagnostics) = notification["params"]["diagnostics"].as_array() else {
        panic!("publishDiagnostics should contain a diagnostic array");
    };
    assert_eq!(diagnostics.len(), 1);
    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic["severity"], 1);
    assert_eq!(diagnostic["source"], "vela");
    assert_eq!(diagnostic["code"], "analysis::unknown_method");
    assert!(
        diagnostic["message"]
            .as_str()
            .is_some_and(|message| message.contains("unknown method `frist`"))
    );

    let Some(candidates) = diagnostic["data"]["candidates"].as_array() else {
        panic!("diagnostic should preserve candidate metadata");
    };
    assert!(
        candidates
            .iter()
            .any(|candidate| candidate["replacement"] == "first")
    );
    let Some(repair_hints) = diagnostic["data"]["repairHints"].as_array() else {
        panic!("diagnostic should preserve repair hints");
    };
    assert!(repair_hints.is_empty());
}
#[test]
fn lsp_did_change_replaces_document_text() {
    let mut server = TestServer::new();
    initialize_server(&mut server);
    let open = notification_value(notify::<lsp_types::notification::DidOpenTextDocument>(
        &mut server,
        serde_json::json!({
            "textDocument": {
                "uri": "file:///workspace/main.vela",
                "languageId": "vela",
                "version": 1,
                "text": "pub fn main(scores: Array<i64>) { return scores.frist() }"
            }
        }),
    ));
    let Some(open_diagnostics) = open["params"]["diagnostics"].as_array() else {
        panic!("didOpen should publish diagnostics");
    };
    assert_eq!(open_diagnostics.len(), 1);
    let change = notification_value(notify::<lsp_types::notification::DidChangeTextDocument>(
        &mut server,
        serde_json::json!({
            "textDocument": {
                "uri": "file:///workspace/main.vela",
                "version": 2
            },
            "contentChanges": [
                {
                    "text": "pub fn main(scores: Array<i64>) { return scores.first() }"
                }
            ]
        }),
    ));

    assert_eq!(change["jsonrpc"], "2.0");
    assert_eq!(change["method"], "textDocument/publishDiagnostics");
    assert_eq!(change["params"]["uri"], "file:///workspace/main.vela");
    let Some(change_diagnostics) = change["params"]["diagnostics"].as_array() else {
        panic!("didChange should publish diagnostics");
    };
    assert!(change_diagnostics.is_empty());
}

#[test]
fn lsp_did_change_applies_incremental_text_edit() {
    let mut server = TestServer::new();
    initialize_server(&mut server);
    let source = "pub fn main(scores: Array<i64>) { return scores.frist() }";
    let start = source
        .find("frist")
        .expect("test source should contain typo");
    let end = start + "frist".len();
    let open = notification_value(notify::<lsp_types::notification::DidOpenTextDocument>(
        &mut server,
        serde_json::json!({
            "textDocument": {
                "uri": "file:///workspace/main.vela",
                "languageId": "vela",
                "version": 1,
                "text": source
            }
        }),
    ));
    let Some(open_diagnostics) = open["params"]["diagnostics"].as_array() else {
        panic!("didOpen should publish diagnostics");
    };
    assert_eq!(open_diagnostics.len(), 1);

    let change = notification_value(notify::<lsp_types::notification::DidChangeTextDocument>(
        &mut server,
        serde_json::json!({
            "textDocument": {
                "uri": "file:///workspace/main.vela",
                "version": 2
            },
            "contentChanges": [
                {
                    "range": {
                        "start": { "line": 0, "character": start },
                        "end": { "line": 0, "character": end }
                    },
                    "text": "first"
                }
            ]
        }),
    ));

    assert_eq!(change["method"], "textDocument/publishDiagnostics");
    assert_eq!(change["params"]["uri"], "file:///workspace/main.vela");
    let Some(change_diagnostics) = change["params"]["diagnostics"].as_array() else {
        panic!("incremental didChange should publish diagnostics");
    };
    assert!(change_diagnostics.is_empty(), "{change_diagnostics:?}");
}

#[test]
fn lsp_did_close_clears_scratch_diagnostics() {
    let mut server = TestServer::new();
    initialize_server(&mut server);
    let uri = "file:///workspace/main.vela";
    let open = notification_value(notify::<lsp_types::notification::DidOpenTextDocument>(
        &mut server,
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": "pub fn main(scores: Array<i64>) { return scores.frist() }"
            }
        }),
    ));
    let open_diagnostics = open["params"]["diagnostics"]
        .as_array()
        .expect("didOpen should publish diagnostics");
    assert_eq!(open_diagnostics.len(), 1);

    let close = notification_value(notify::<lsp_types::notification::DidCloseTextDocument>(
        &mut server,
        serde_json::json!({
            "textDocument": {
                "uri": uri
            }
        }),
    ));

    assert_eq!(close["method"], "textDocument/publishDiagnostics");
    assert_eq!(close["params"]["uri"], uri);
    let close_diagnostics = close["params"]["diagnostics"]
        .as_array()
        .expect("didClose should publish diagnostics");
    assert!(close_diagnostics.is_empty(), "{close_diagnostics:?}");
}

#[test]
fn lsp_did_close_restores_disk_snapshot_diagnostics() {
    let root = temp_workspace();
    let source_path = root.join("scripts").join("game").join("main.vela");
    fs::write(
        &source_path,
        "pub fn main(scores: Array<i64>) { return scores.first() }",
    )
    .expect("disk source should be writable");
    let source_uri = file_uri(&source_path);

    let mut server = TestServer::new();
    let _ = response_value(request::<lsp_types::request::Initialize>(
        &mut server,
        1,
        serde_json::json!({
            "processId": null,
            "rootUri": file_uri(&root.join("scripts")),
            "capabilities": {}
        }),
    ));
    assert_no_messages(notify::<lsp_types::notification::DidChangeWatchedFiles>(
        &mut server,
        serde_json::json!({
            "changes": [{ "uri": source_uri, "type": 1 }]
        }),
    ));
    let open = notification_value(notify::<lsp_types::notification::DidOpenTextDocument>(
        &mut server,
        serde_json::json!({
            "textDocument": {
                "uri": source_uri,
                "languageId": "vela",
                "version": 1,
                "text": "pub fn main(scores: Array<i64>) { return scores.frist() }"
            }
        }),
    ));
    let open_diagnostics = open["params"]["diagnostics"]
        .as_array()
        .expect("didOpen should publish diagnostics");
    assert_eq!(open_diagnostics.len(), 1);

    let close = notification_value(notify::<lsp_types::notification::DidCloseTextDocument>(
        &mut server,
        serde_json::json!({
            "textDocument": {
                "uri": source_uri
            }
        }),
    ));

    let close_diagnostics = close["params"]["diagnostics"]
        .as_array()
        .expect("didClose should publish restored disk diagnostics");
    assert!(close_diagnostics.is_empty(), "{close_diagnostics:?}");
    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_did_close_restores_disk_snapshot_definition_queries() {
    let root = temp_workspace();
    let source_path = root.join("scripts").join("game").join("main.vela");
    let disk_source = r#"struct DiskCell {
    disk_value: i64,
}

fn main(cell: DiskCell) {
    return cell.disk_value;
}"#;
    let overlay_source = r#"struct OverlayCell {
    overlay_value: i64,
}

fn main(cell: OverlayCell) {
    return cell.overlay_value;
}"#;
    fs::write(&source_path, disk_source).expect("disk source should be writable");
    let source_uri = file_uri(&source_path);

    let mut server = TestServer::new();
    let _ = response_value(request::<lsp_types::request::Initialize>(
        &mut server,
        1,
        serde_json::json!({
            "processId": null,
            "rootUri": file_uri(&root.join("scripts")),
            "capabilities": {}
        }),
    ));
    assert_no_messages(notify::<lsp_types::notification::DidChangeWatchedFiles>(
        &mut server,
        serde_json::json!({
            "changes": [{ "uri": source_uri, "type": 1 }]
        }),
    ));
    let _ = notification_value(notify::<lsp_types::notification::DidOpenTextDocument>(
        &mut server,
        serde_json::json!({
            "textDocument": {
                "uri": source_uri,
                "languageId": "vela",
                "version": 1,
                "text": overlay_source
            }
        }),
    ));

    let overlay_use_line = overlay_source
        .lines()
        .nth(5)
        .expect("overlay field use line should exist");
    let overlay_definition = response_value(request::<lsp_types::request::GotoDefinition>(
        &mut server,
        2,
        serde_json::json!({
            "textDocument": { "uri": source_uri },
            "position": {
                "line": 5,
                "character": overlay_use_line
                    .find("overlay_value")
                    .expect("overlay field use should exist")
            }
        }),
    ));

    assert_eq!(overlay_definition["result"]["uri"], source_uri);
    assert_eq!(overlay_definition["result"]["range"]["start"]["line"], 1);
    assert_eq!(
        overlay_definition["result"]["range"]["end"]["character"],
        overlay_source
            .lines()
            .nth(1)
            .expect("overlay field declaration line should exist")
            .find("overlay_value")
            .expect("overlay field declaration should exist")
            + "overlay_value".len()
    );

    let close = notification_value(notify::<lsp_types::notification::DidCloseTextDocument>(
        &mut server,
        serde_json::json!({
            "textDocument": {
                "uri": source_uri
            }
        }),
    ));
    assert_eq!(close["method"], "textDocument/publishDiagnostics");
    assert_eq!(close["params"]["uri"], source_uri);

    let disk_use_line = disk_source
        .lines()
        .nth(5)
        .expect("disk field use line should exist");
    let disk_definition = response_value(request::<lsp_types::request::GotoDefinition>(
        &mut server,
        3,
        serde_json::json!({
            "textDocument": { "uri": source_uri },
            "position": {
                "line": 5,
                "character": disk_use_line
                    .find("disk_value")
                    .expect("disk field use should exist")
            }
        }),
    ));

    assert_eq!(disk_definition["result"]["uri"], source_uri);
    assert_eq!(disk_definition["result"]["range"]["start"]["line"], 1);
    assert_eq!(
        disk_definition["result"]["range"]["end"]["character"],
        disk_source
            .lines()
            .nth(1)
            .expect("disk field declaration line should exist")
            .find("disk_value")
            .expect("disk field declaration should exist")
            + "disk_value".len()
    );

    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_initialize_uses_workspace_root_for_document_sync() {
    let mut server = TestServer::new();
    let response = response_value(request::<lsp_types::request::Initialize>(
        &mut server,
        1,
        serde_json::json!({
            "processId": null,
            "rootUri": "file:///workspace/scripts",
            "capabilities": {}
        }),
    ));
    assert_eq!(response["result"]["serverInfo"]["name"], "vela_lsp_server");
    let helper = notification_value(notify::<lsp_types::notification::DidOpenTextDocument>(
        &mut server,
        serde_json::json!({
            "textDocument": {
                "uri": "file:///workspace/scripts/game/helper.vela",
                "languageId": "vela",
                "version": 1,
                "text": "pub fn grant() { return 1 }"
            }
        }),
    ));
    let Some(helper_diagnostics) = helper["params"]["diagnostics"].as_array() else {
        panic!("helper didOpen should publish diagnostics");
    };
    assert!(helper_diagnostics.is_empty(), "{helper_diagnostics:?}");
    let main = notification_value(notify::<lsp_types::notification::DidOpenTextDocument>(
        &mut server,
        serde_json::json!({
            "textDocument": {
                "uri": "file:///workspace/scripts/game/main.vela",
                "languageId": "vela",
                "version": 1,
                "text": "use game::helper::grant\npub fn main() { return grant() }"
            }
        }),
    ));

    let Some(main_diagnostics) = main["params"]["diagnostics"].as_array() else {
        panic!("main didOpen should publish diagnostics");
    };
    assert!(
        main_diagnostics
            .iter()
            .all(|diagnostic| diagnostic["code"] != "hir::unresolved_module"
                && diagnostic["code"] != "hir::unresolved_import"),
        "{main_diagnostics:?}"
    );
}
