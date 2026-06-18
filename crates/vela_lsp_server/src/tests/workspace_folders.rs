use super::{
    JsonRpcResult, JsonValue, LspServer, assert_workspace_progress, notification,
    notification_value, notification_values, publish_diagnostics_notifications, request,
    response_value,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_WORKSPACE_ID: AtomicU64 = AtomicU64::new(0);

#[test]
fn workspace_folder_removal_clears_disk_facts_but_keeps_open_overlay() {
    let root = temp_workspace();
    let scripts_root = root.join("scripts");
    let helper_path = scripts_root.join("game").join("helper.vela");
    fs::write(&helper_path, "pub fn grant() { return 1 }").expect("source should be writable");

    let mut server = LspServer::new();
    let response = response_value(server.handle_json(&request(
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": file_uri(&scripts_root),
            "capabilities": {
                "window": {
                    "workDoneProgress": true
                }
            }
        }),
    )));
    assert_eq!(response["result"]["serverInfo"]["name"], "vela_lsp_server");

    let watched = server.handle_json(&notification(
        "workspace/didChangeWatchedFiles",
        serde_json::json!({
            "changes": [{ "uri": file_uri(&helper_path), "type": 1 }]
        }),
    ));
    assert_eq!(watched, JsonRpcResult::None);

    let main = open_main(&mut server, &root, "game::helper");
    assert_no_unresolved_imports(&main);

    let notifications = notification_values(server.handle_json(&notification(
        "workspace/didChangeWorkspaceFolders",
        serde_json::json!({
            "event": {
                "added": [],
                "removed": [{ "uri": file_uri(&scripts_root), "name": "scripts" }]
            }
        }),
    )));
    assert_workspace_progress(&notifications);
    let published = publish_diagnostics_notifications(&notifications);
    assert_eq!(published.len(), 1);
    assert_eq!(
        published[0]["params"]["uri"],
        file_uri(&root.join("scripts").join("game").join("main.vela"))
    );
    assert_has_unresolved_import(published[0]);

    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

fn temp_workspace() -> PathBuf {
    let suffix = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(error) => panic!("system time should be after UNIX_EPOCH: {error}"),
    };
    let sequence = NEXT_WORKSPACE_ID.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "vela_lsp_workspace_folders_{}_{}_{}",
        std::process::id(),
        suffix,
        sequence
    ));
    fs::create_dir_all(root.join("scripts").join("game"))
        .expect("temporary workspace should be creatable");
    root
}

fn open_main(server: &mut LspServer, root: &Path, import_module: &str) -> JsonValue {
    notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": file_uri(&root.join("scripts").join("game").join("main.vela")),
                "languageId": "vela",
                "version": 1,
                "text": format!("use {import_module}::grant\npub fn main() {{ return grant() }}")
            }
        }),
    )))
}

fn assert_no_unresolved_imports(notification: &JsonValue) {
    let Some(diagnostics) = notification["params"]["diagnostics"].as_array() else {
        panic!("diagnostics should be an array");
    };
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic["code"] != "hir::unresolved_module"
                && diagnostic["code"] != "hir::unresolved_import"),
        "{diagnostics:?}"
    );
}

fn assert_has_unresolved_import(notification: &JsonValue) {
    let Some(diagnostics) = notification["params"]["diagnostics"].as_array() else {
        panic!("diagnostics should be an array");
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic["code"] == "hir::unresolved_module"
                || diagnostic["code"] == "hir::unresolved_import"),
        "{diagnostics:?}"
    );
}

fn file_uri(path: &Path) -> String {
    let path = path.display().to_string().replace('\\', "/");
    if path.starts_with('/') {
        format!("file://{path}")
    } else {
        format!("file:///{path}")
    }
}
