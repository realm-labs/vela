use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::{
    JsonValue, TestServer, assert_no_messages, assert_workspace_progress, notification_value,
    notification_values, notify, publish_diagnostics_notifications, request, response_value,
};

static NEXT_WORKSPACE_ID: AtomicU64 = AtomicU64::new(0);

fn temp_workspace() -> PathBuf {
    let suffix = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(error) => panic!("system time should be after UNIX_EPOCH: {error}"),
    };
    let sequence = NEXT_WORKSPACE_ID.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "vela_lsp_server_{}_{}_{}",
        std::process::id(),
        suffix,
        sequence
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
fn write_workspace(root: &Path, helper_name: &str) -> (PathBuf, PathBuf) {
    let config_path = root.join("vela.toml");
    let helper_path = root
        .join("scripts")
        .join("game")
        .join(format!("{helper_name}.vela"));
    if let Err(error) = fs::write(
        &config_path,
        r#"
                [workspace]
                roots = ["scripts"]
            "#,
    ) {
        panic!("vela.toml should be writable: {error}");
    }
    if let Err(error) = fs::write(&helper_path, "pub fn grant() { return 1 }") {
        panic!("helper source should be writable: {error}");
    }
    (config_path, helper_path)
}
fn initialized_server(root: &Path, config_path: &Path, helper_path: &Path) -> TestServer {
    let mut server = TestServer::new();
    let response = response_value(request::<lsp_types::request::Initialize>(
        &mut server,
        1,
        serde_json::json!({
            "processId": null,
            "rootUri": file_uri(root),
            "capabilities": {
                "window": {
                    "workDoneProgress": true
                }
            }
        }),
    ));
    assert_eq!(response["result"]["serverInfo"]["name"], "vela_lsp_server");

    let watched = notify::<lsp_types::notification::DidChangeWatchedFiles>(
        &mut server,
        serde_json::json!({
            "changes": [
                { "uri": file_uri(config_path), "type": 1 },
                { "uri": file_uri(helper_path), "type": 1 }
            ]
        }),
    );
    assert_no_messages(watched);
    server
}
fn open_main(server: &mut TestServer, root: &Path, import_module: &str) -> JsonValue {
    let main_uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    notification_value(notify::<lsp_types::notification::DidOpenTextDocument>(
        server,
        serde_json::json!({
            "textDocument": {
                "uri": main_uri,
                "languageId": "vela",
                "version": 1,
                "text": format!(
                    "use {import_module}::grant\npub fn main() {{ return grant() }}"
                )
            }
        }),
    ))
}
fn assert_no_unresolved_imports(notification: &JsonValue) {
    let Some(diagnostics) = notification["params"]["diagnostics"].as_array() else {
        panic!("didOpen should publish diagnostics");
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
        panic!("notification should publish diagnostics");
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic["code"] == "hir::unresolved_module"
                || diagnostic["code"] == "hir::unresolved_import"
                || diagnostic["code"] == "project::diagnostic"
                    && diagnostic["message"]
                        .as_str()
                        .is_some_and(|message| message.contains("unresolved module"))),
        "{diagnostics:?}"
    );
}

fn valid_schema_artifact() -> &'static str {
    r#"{
            "formatVersion": 1,
            "facts": {
                "types": [
                    {
                        "name": "Player",
                        "fact": { "kind": "host", "name": "Player" }
                    }
                ]
            }
        }"#
}

#[test]
fn invalid_vela_toml_publishes_config_diagnostic() {
    let root = temp_workspace();
    let config_path = root.join("vela.toml");
    fs::write(&config_path, "[workspace]\nroots = \"scripts\"\n")
        .expect("invalid vela.toml should be writable");
    let mut server = TestServer::new();
    let _ = request::<lsp_types::request::Initialize>(
        &mut server,
        1,
        serde_json::json!({
            "processId": null,
            "rootUri": file_uri(&root),
            "capabilities": {}
        }),
    );
    let notifications =
        notification_values(notify::<lsp_types::notification::DidChangeWatchedFiles>(
            &mut server,
            serde_json::json!({
                "changes": [{ "uri": file_uri(&config_path), "type": 1 }]
            }),
        ));
    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0]["params"]["uri"], file_uri(&config_path));
    let diagnostics = notifications[0]["params"]["diagnostics"]
        .as_array()
        .expect("config diagnostics should be an array");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic["code"] == "project::diagnostic"
            && diagnostic["message"]
                .as_str()
                .is_some_and(|message| message.contains("workspace.roots"))
    }));
    fs::write(&config_path, "[workspace]\nroots = [\"scripts\"]\n")
        .expect("valid vela.toml should be writable");
    let cleared = notification_values(notify::<lsp_types::notification::DidChangeWatchedFiles>(
        &mut server,
        serde_json::json!({
            "changes": [{ "uri": file_uri(&config_path), "type": 2 }]
        }),
    ));
    assert_eq!(cleared.len(), 1);
    assert!(
        cleared[0]["params"]["diagnostics"]
            .as_array()
            .is_some_and(Vec::is_empty),
        "{cleared:?}"
    );
    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn deleting_vela_toml_clears_config_diagnostic() {
    let root = temp_workspace();
    let config_path = root.join("vela.toml");
    fs::write(&config_path, "[workspace]\nroots = \"scripts\"\n")
        .expect("invalid vela.toml should be writable");
    let mut server = TestServer::new();
    let _ = request::<lsp_types::request::Initialize>(
        &mut server,
        1,
        serde_json::json!({
            "processId": null,
            "rootUri": file_uri(&root),
            "capabilities": {}
        }),
    );
    let invalid = notification_values(notify::<lsp_types::notification::DidChangeWatchedFiles>(
        &mut server,
        serde_json::json!({
            "changes": [{ "uri": file_uri(&config_path), "type": 1 }]
        }),
    ));
    assert_eq!(invalid.len(), 1);
    assert_eq!(invalid[0]["params"]["uri"], file_uri(&config_path));
    let diagnostics = invalid[0]["params"]["diagnostics"]
        .as_array()
        .expect("config diagnostics should be an array");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic["code"] == "project::diagnostic"),
        "{diagnostics:?}"
    );

    fs::remove_file(&config_path).expect("invalid vela.toml should be removable");
    let cleared = notification_values(notify::<lsp_types::notification::DidChangeWatchedFiles>(
        &mut server,
        serde_json::json!({
            "changes": [{ "uri": file_uri(&config_path), "type": 3 }]
        }),
    ));

    assert_eq!(cleared.len(), 1);
    assert_eq!(cleared[0]["params"]["uri"], file_uri(&config_path));
    assert!(
        cleared[0]["params"]["diagnostics"]
            .as_array()
            .is_some_and(Vec::is_empty),
        "{cleared:?}"
    );
    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn schema_watch_publishes_invalid_schema_diagnostic() {
    let root = temp_workspace();
    let config_path = root.join("vela.toml");
    let schema_path = root.join("target").join("vela").join("schema.json");
    fs::create_dir_all(schema_path.parent().expect("schema should have parent"))
        .expect("schema directory should be creatable");
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
    fs::write(&schema_path, "{").expect("invalid schema should be writable");

    let mut server = TestServer::new();
    let _ = request::<lsp_types::request::Initialize>(
        &mut server,
        1,
        serde_json::json!({
            "processId": null,
            "rootUri": file_uri(&root),
            "capabilities": {}
        }),
    );
    let notifications =
        notification_values(notify::<lsp_types::notification::DidChangeWatchedFiles>(
            &mut server,
            serde_json::json!({
                "changes": [{ "uri": file_uri(&config_path), "type": 1 }]
            }),
        ));

    assert_eq!(notifications.len(), 1, "{notifications:?}");
    assert_eq!(notifications[0]["params"]["uri"], file_uri(&schema_path));
    let diagnostics = notifications[0]["params"]["diagnostics"]
        .as_array()
        .expect("schema diagnostics should be an array");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic["code"] == "schema::diagnostic"
            && diagnostic["message"]
                .as_str()
                .is_some_and(|message| message.contains("host schema"))
    }));
    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn schema_watch_clears_diagnostic_after_valid_reload() {
    let root = temp_workspace();
    let config_path = root.join("vela.toml");
    let schema_path = root.join("target").join("vela").join("schema.json");
    fs::create_dir_all(schema_path.parent().expect("schema should have parent"))
        .expect("schema directory should be creatable");
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
    fs::write(&schema_path, "{").expect("invalid schema should be writable");

    let mut server = TestServer::new();
    let _ = request::<lsp_types::request::Initialize>(
        &mut server,
        1,
        serde_json::json!({
            "processId": null,
            "rootUri": file_uri(&root),
            "capabilities": {}
        }),
    );
    let invalid = notification_values(notify::<lsp_types::notification::DidChangeWatchedFiles>(
        &mut server,
        serde_json::json!({
            "changes": [{ "uri": file_uri(&config_path), "type": 1 }]
        }),
    ));
    assert_eq!(invalid.len(), 1, "{invalid:?}");
    fs::write(&schema_path, valid_schema_artifact()).expect("valid schema should be writable");

    let cleared = notification_values(notify::<lsp_types::notification::DidChangeWatchedFiles>(
        &mut server,
        serde_json::json!({
            "changes": [{ "uri": file_uri(&schema_path), "type": 2 }]
        }),
    ));

    assert_eq!(cleared.len(), 1, "{cleared:?}");
    assert_eq!(cleared[0]["params"]["uri"], file_uri(&schema_path));
    assert!(
        cleared[0]["params"]["diagnostics"]
            .as_array()
            .is_some_and(Vec::is_empty),
        "{cleared:?}"
    );
    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn schema_delete_publishes_missing_schema_diagnostic() {
    let root = temp_workspace();
    let config_path = root.join("vela.toml");
    let schema_path = root.join("target").join("vela").join("schema.json");
    fs::create_dir_all(schema_path.parent().expect("schema should have parent"))
        .expect("schema directory should be creatable");
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
    fs::write(&schema_path, valid_schema_artifact()).expect("schema should be writable");

    let mut server = TestServer::new();
    let _ = request::<lsp_types::request::Initialize>(
        &mut server,
        1,
        serde_json::json!({
            "processId": null,
            "rootUri": file_uri(&root),
            "capabilities": {}
        }),
    );
    let loaded = notification_values(notify::<lsp_types::notification::DidChangeWatchedFiles>(
        &mut server,
        serde_json::json!({
            "changes": [{ "uri": file_uri(&config_path), "type": 1 }]
        }),
    ));
    assert_eq!(loaded.len(), 1, "{loaded:?}");
    assert!(
        loaded[0]["params"]["diagnostics"]
            .as_array()
            .is_some_and(Vec::is_empty),
        "{loaded:?}"
    );

    fs::remove_file(&schema_path).expect("schema should be removable");
    let missing = notification_values(notify::<lsp_types::notification::DidChangeWatchedFiles>(
        &mut server,
        serde_json::json!({
            "changes": [{ "uri": file_uri(&schema_path), "type": 3 }]
        }),
    ));

    assert_eq!(missing.len(), 1, "{missing:?}");
    assert_eq!(missing[0]["params"]["uri"], file_uri(&schema_path));
    let diagnostics = missing[0]["params"]["diagnostics"]
        .as_array()
        .expect("schema diagnostics should be an array");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic["code"] == "schema::diagnostic"
                && diagnostic["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("host schema"))
        }),
        "{diagnostics:?}"
    );
    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn file_create_adds_module() {
    let root = temp_workspace();
    let (config_path, helper_path) = write_workspace(&root, "helper");
    let mut server = initialized_server(&root, &config_path, &helper_path);
    let main = open_main(&mut server, &root, "game::helper");
    assert_no_unresolved_imports(&main);
    if let Err(error) = fs::remove_dir_all(&root) {
        panic!("temporary workspace should be removable: {error}");
    }
}

#[test]
fn workspace_folder_change_reindexes_project() {
    let root = temp_workspace();
    let (_, helper_path) = write_workspace(&root, "helper");
    let game_root = root.join("scripts").join("game");
    let scripts_root = root.join("scripts");
    let mut server = TestServer::new();
    let response = response_value(request::<lsp_types::request::Initialize>(
        &mut server,
        1,
        serde_json::json!({
            "processId": null,
            "rootUri": file_uri(&game_root),
            "capabilities": {
                "window": {
                    "workDoneProgress": true
                }
            }
        }),
    ));
    assert_eq!(response["result"]["serverInfo"]["name"], "vela_lsp_server");
    let watched = notify::<lsp_types::notification::DidChangeWatchedFiles>(
        &mut server,
        serde_json::json!({
            "changes": [{ "uri": file_uri(&helper_path), "type": 1 }]
        }),
    );
    assert_no_messages(watched);
    let main = open_main(&mut server, &root, "game::helper");
    assert_has_unresolved_import(&main);

    let notifications = notification_values(notify::<
        lsp_types::notification::DidChangeWorkspaceFolders,
    >(
        &mut server,
        serde_json::json!({
            "event": {
                "added": [{ "uri": file_uri(&scripts_root), "name": "scripts" }],
                "removed": [{ "uri": file_uri(&game_root), "name": "game" }]
            }
        }),
    ));
    assert_workspace_progress(&notifications);
    let published = publish_diagnostics_notifications(&notifications);
    assert_eq!(published.len(), 1);
    assert_no_unresolved_imports(published[0]);
    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn file_delete_reports_removed_imports() {
    let root = temp_workspace();
    let (config_path, helper_path) = write_workspace(&root, "helper");
    let mut server = initialized_server(&root, &config_path, &helper_path);
    let main = open_main(&mut server, &root, "game::helper");
    assert_no_unresolved_imports(&main);
    if let Err(error) = fs::remove_file(&helper_path) {
        panic!("helper source should be removable: {error}");
    }
    let notifications =
        notification_values(notify::<lsp_types::notification::DidChangeWatchedFiles>(
            &mut server,
            serde_json::json!({
                "changes": [
                    { "uri": file_uri(&helper_path), "type": 3 }
                ]
            }),
        ));

    assert_workspace_progress(&notifications);
    let published = publish_diagnostics_notifications(&notifications);
    assert_eq!(published.len(), 1);
    let Some(diagnostics) = published[0]["params"]["diagnostics"].as_array() else {
        panic!("file delete should publish diagnostics");
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic["code"] == "project::diagnostic"
                && diagnostic["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("unresolved module"))),
        "{diagnostics:?}"
    );
    if let Err(error) = fs::remove_dir_all(&root) {
        panic!("temporary workspace should be removable: {error}");
    }
}
#[test]
fn lsp_progress_wraps_workspace_diagnostics() {
    let root = temp_workspace();
    let (config_path, helper_path) = write_workspace(&root, "helper");
    let mut server = initialized_server(&root, &config_path, &helper_path);
    let main = open_main(&mut server, &root, "game::helper");
    assert_no_unresolved_imports(&main);
    if let Err(error) = fs::remove_file(&helper_path) {
        panic!("helper source should be removable: {error}");
    }

    let notifications =
        notification_values(notify::<lsp_types::notification::DidChangeWatchedFiles>(
            &mut server,
            serde_json::json!({
                "changes": [
                    { "uri": file_uri(&helper_path), "type": 3 }
                ]
            }),
        ));

    assert_eq!(notifications.len(), 3);
    assert_workspace_progress(&notifications);
    let published = publish_diagnostics_notifications(&notifications);
    assert_eq!(published.len(), 1);
    assert_eq!(
        published[0]["params"]["uri"],
        file_uri(&root.join("scripts").join("game").join("main.vela"))
    );
    assert_has_unresolved_import(published[0]);
    if let Err(error) = fs::remove_dir_all(&root) {
        panic!("temporary workspace should be removable: {error}");
    }
}
#[test]
fn file_rename_updates_module_path() {
    let root = temp_workspace();
    let (config_path, helper_path) = write_workspace(&root, "helper");
    let reward_path = root.join("scripts").join("game").join("reward.vela");
    let mut server = initialized_server(&root, &config_path, &helper_path);
    let main = open_main(&mut server, &root, "game::helper");
    assert_no_unresolved_imports(&main);
    if let Err(error) = fs::rename(&helper_path, &reward_path) {
        panic!("helper source should be renameable: {error}");
    }
    let _ = notification_values(notify::<lsp_types::notification::DidChangeWatchedFiles>(
        &mut server,
        serde_json::json!({
            "changes": [
                { "uri": file_uri(&helper_path), "type": 3 },
                { "uri": file_uri(&reward_path), "type": 1 }
            ]
        }),
    ));

    let main_uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    let main = notification_value(notify::<lsp_types::notification::DidChangeTextDocument>(
        &mut server,
        serde_json::json!({
            "textDocument": {
                "uri": main_uri,
                "version": 2
            },
            "contentChanges": [
                {
                    "text": "use game::reward::grant\npub fn main() { return grant() }"
                }
            ]
        }),
    ));
    assert_no_unresolved_imports(&main);
    if let Err(error) = fs::remove_dir_all(&root) {
        panic!("temporary workspace should be removable: {error}");
    }
}
