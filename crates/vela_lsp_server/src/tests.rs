use crate::{JsonRpcResult, LspServer};
use serde_json::Value as JsonValue;

fn request(id: i64, method: &str, params: JsonValue) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params
    })
    .to_string()
}

fn notification(method: &str, params: JsonValue) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params
    })
    .to_string()
}

fn response_value(result: JsonRpcResult) -> JsonValue {
    let Some(response) = result.into_response() else {
        panic!("request should return a JSON-RPC response");
    };
    json_value(&response)
}

fn notification_value(result: JsonRpcResult) -> JsonValue {
    let Some(notification) = result.into_notification() else {
        panic!("notification should return a JSON-RPC notification");
    };
    json_value(&notification)
}

fn notification_values(result: JsonRpcResult) -> Vec<JsonValue> {
    let Some(notifications) = result.into_notifications() else {
        panic!("result should contain JSON-RPC notifications");
    };
    notifications
        .iter()
        .map(|notification| json_value(notification))
        .collect()
}

fn json_value(source: &str) -> JsonValue {
    match serde_json::from_str(source) {
        Ok(value) => value,
        Err(error) => panic!("message should be valid JSON: {error}"),
    }
}

fn publish_diagnostics_notifications(notifications: &[JsonValue]) -> Vec<&JsonValue> {
    notifications
        .iter()
        .filter(|notification| notification["method"] == "textDocument/publishDiagnostics")
        .collect()
}

fn assert_workspace_progress(notifications: &[JsonValue]) {
    let Some(begin) = notifications.first() else {
        panic!("workspace progress should include a begin notification");
    };
    let Some(end) = notifications.last() else {
        panic!("workspace progress should include an end notification");
    };

    assert_eq!(begin["method"], "$/progress");
    assert_eq!(begin["params"]["token"], "vela/workspace-diagnostics");
    assert_eq!(begin["params"]["value"]["kind"], "begin");
    assert_eq!(
        begin["params"]["value"]["title"],
        "Vela workspace diagnostics"
    );

    assert_eq!(end["method"], "$/progress");
    assert_eq!(end["params"]["token"], "vela/workspace-diagnostics");
    assert_eq!(end["params"]["value"]["kind"], "end");
}

mod lifecycle {
    use super::{JsonRpcResult, JsonValue, LspServer, notification, request, response_value};

    #[test]
    fn lsp_initialize_reports_capabilities() {
        let mut server = LspServer::new();
        let response = response_value(server.handle_json(&request(
            1,
            "initialize",
            serde_json::json!({
                "processId": null,
                "capabilities": {}
            }),
        )));

        assert!(server.is_initialized());
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 1);
        assert_eq!(response["result"]["serverInfo"]["name"], "vela_lsp_server");
        assert_eq!(response["result"]["capabilities"]["workDoneProgress"], true);
        assert_eq!(
            response["result"]["capabilities"]["textDocumentSync"]["openClose"],
            true
        );
        assert_eq!(
            response["result"]["capabilities"]["textDocumentSync"]["change"],
            2
        );
        assert_eq!(
            response["result"]["capabilities"]["completionProvider"]["resolveProvider"],
            false
        );
        assert_eq!(
            response["result"]["capabilities"]["completionProvider"]["triggerCharacters"],
            serde_json::json!([".", ":", "{", "(", ",", "|"])
        );
        assert_eq!(
            response["result"]["capabilities"]["signatureHelpProvider"]["triggerCharacters"],
            serde_json::json!(["(", ","])
        );
        assert_eq!(
            response["result"]["capabilities"]["signatureHelpProvider"]["retriggerCharacters"],
            serde_json::json!([","])
        );
        assert_eq!(
            response["result"]["capabilities"]["hoverProvider"],
            serde_json::json!(true)
        );
        assert_eq!(
            response["result"]["capabilities"]["definitionProvider"],
            serde_json::json!(true)
        );
        assert_eq!(
            response["result"]["capabilities"]["documentSymbolProvider"],
            serde_json::json!(true)
        );
        assert_eq!(
            response["result"]["capabilities"]["workspace"]["workspaceFolders"]["supported"],
            true
        );
        assert_eq!(
            response["result"]["capabilities"]["workspace"]["workspaceFolders"]["changeNotifications"],
            true
        );
    }

    #[test]
    fn lsp_initialized_notification_has_no_response() {
        let mut server = LspServer::new();
        let result = server.handle_json(&notification("initialized", serde_json::json!({})));

        assert!(server.is_initialized());
        assert_eq!(result, JsonRpcResult::None);
    }

    #[test]
    fn lsp_cancellation_discards_stale_request() {
        let mut server = LspServer::new();
        let cancel = server.handle_json(&notification(
            "$/cancelRequest",
            serde_json::json!({ "id": 7 }),
        ));
        assert_eq!(cancel, JsonRpcResult::None);

        let response = response_value(server.handle_json(&request(
            7,
            "initialize",
            serde_json::json!({
                "processId": null,
                "capabilities": {}
            }),
        )));

        assert!(!server.is_initialized());
        assert_eq!(response["id"], 7);
        assert_eq!(response["error"]["code"], -32800);
        assert!(
            response["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("cancelled"))
        );
    }

    #[test]
    fn lsp_shutdown_exits_without_background_tasks() {
        let mut server = LspServer::new();
        let response = response_value(server.handle_json(&request(2, "shutdown", JsonValue::Null)));
        let exit = server.handle_json(&notification("exit", JsonValue::Null));

        assert_eq!(response["result"], JsonValue::Null);
        assert!(server.is_shutdown_requested());
        assert!(server.is_exited());
        assert_eq!(exit, JsonRpcResult::None);
    }
}
mod document_sync {
    use super::{LspServer, notification, notification_value, request, response_value};

    #[test]
    fn lsp_did_open_publishes_diagnostics() {
        let mut server = LspServer::new();
        let notification = notification_value(server.handle_json(&notification(
            "textDocument/didOpen",
            serde_json::json!({
                "textDocument": {
                    "uri": "file:///workspace/main.vela",
                    "languageId": "vela",
                    "version": 1,
                    "text": "pub fn main(scores: Array<i64>) { return scores.frist() }"
                }
            }),
        )));

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
        let mut server = LspServer::new();
        let open = notification_value(server.handle_json(&notification(
            "textDocument/didOpen",
            serde_json::json!({
                "textDocument": {
                    "uri": "file:///workspace/main.vela",
                    "languageId": "vela",
                    "version": 1,
                    "text": "pub fn main(scores: Array<i64>) { return scores.frist() }"
                }
            }),
        )));
        let Some(open_diagnostics) = open["params"]["diagnostics"].as_array() else {
            panic!("didOpen should publish diagnostics");
        };
        assert_eq!(open_diagnostics.len(), 1);
        let change = notification_value(server.handle_json(&notification(
            "textDocument/didChange",
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
        )));

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
        let mut server = LspServer::new();
        let source = "pub fn main(scores: Array<i64>) { return scores.frist() }";
        let start = source
            .find("frist")
            .expect("test source should contain typo");
        let end = start + "frist".len();
        let open = notification_value(server.handle_json(&notification(
            "textDocument/didOpen",
            serde_json::json!({
                "textDocument": {
                    "uri": "file:///workspace/main.vela",
                    "languageId": "vela",
                    "version": 1,
                    "text": source
                }
            }),
        )));
        let Some(open_diagnostics) = open["params"]["diagnostics"].as_array() else {
            panic!("didOpen should publish diagnostics");
        };
        assert_eq!(open_diagnostics.len(), 1);

        let change = notification_value(server.handle_json(&notification(
            "textDocument/didChange",
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
        )));

        assert_eq!(change["method"], "textDocument/publishDiagnostics");
        assert_eq!(change["params"]["uri"], "file:///workspace/main.vela");
        let Some(change_diagnostics) = change["params"]["diagnostics"].as_array() else {
            panic!("incremental didChange should publish diagnostics");
        };
        assert!(change_diagnostics.is_empty(), "{change_diagnostics:?}");
    }

    #[test]
    fn lsp_initialize_uses_workspace_root_for_document_sync() {
        let mut server = LspServer::new();
        let response = response_value(server.handle_json(&request(
            1,
            "initialize",
            serde_json::json!({
                "processId": null,
                "rootUri": "file:///workspace/scripts",
                "capabilities": {}
            }),
        )));
        assert_eq!(response["result"]["serverInfo"]["name"], "vela_lsp_server");
        let helper = notification_value(server.handle_json(&notification(
            "textDocument/didOpen",
            serde_json::json!({
                "textDocument": {
                    "uri": "file:///workspace/scripts/game/helper.vela",
                    "languageId": "vela",
                    "version": 1,
                    "text": "pub fn grant() { return 1 }"
                }
            }),
        )));
        let Some(helper_diagnostics) = helper["params"]["diagnostics"].as_array() else {
            panic!("helper didOpen should publish diagnostics");
        };
        assert!(helper_diagnostics.is_empty(), "{helper_diagnostics:?}");
        let main = notification_value(server.handle_json(&notification(
            "textDocument/didOpen",
            serde_json::json!({
                "textDocument": {
                    "uri": "file:///workspace/scripts/game/main.vela",
                    "languageId": "vela",
                    "version": 1,
                    "text": "use game::helper::grant\npub fn main() { return grant() }"
                }
            }),
        )));

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
}

mod completion {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{LspServer, notification, notification_value, request, response_value};

    #[test]
    fn lsp_completion_uses_open_overlay_declarations() {
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
        let _ = notification_value(server.handle_json(&notification(
            "textDocument/didOpen",
            serde_json::json!({
                "textDocument": {
                    "uri": "file:///workspace/scripts/game/main.vela",
                    "languageId": "vela",
                    "version": 1,
                    "text": "pub fn overlay_only() { return 2 }"
                }
            }),
        )));

        let response = response_value(server.handle_json(&request(
            2,
            "textDocument/completion",
            serde_json::json!({
                "textDocument": { "uri": "file:///workspace/scripts/game/main.vela" },
                "position": { "line": 0, "character": 7 }
            }),
        )));

        assert_completion(
            &response,
            "game::main::overlay_only",
            3,
            "Function() -> unknown",
        );
    }

    #[test]
    fn lsp_completion_uses_loaded_schema_facts() {
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
        fs::write(
            &schema_path,
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
            }"#,
        )
        .expect("schema should be writable");

        let mut server = LspServer::new();
        let _ = response_value(server.handle_json(&request(
            1,
            "initialize",
            serde_json::json!({
                "processId": null,
                "rootUri": file_uri(&root),
                "capabilities": {}
            }),
        )));
        let _ = server.handle_json(&notification(
            "workspace/didChangeWatchedFiles",
            serde_json::json!({
                "changes": [{ "uri": file_uri(&config_path), "type": 1 }]
            }),
        ));
        let main_uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
        let _ = notification_value(server.handle_json(&notification(
            "textDocument/didOpen",
            serde_json::json!({
                "textDocument": {
                    "uri": main_uri,
                    "languageId": "vela",
                    "version": 1,
                    "text": "pub fn main() { Pla }"
                }
            }),
        )));

        let response = response_value(server.handle_json(&request(
            2,
            "textDocument/completion",
            serde_json::json!({
                "textDocument": { "uri": main_uri },
                "position": { "line": 0, "character": 18 }
            }),
        )));

        assert_completion(&response, "Player", 22, "Player");
        fs::remove_dir_all(&root).expect("temporary workspace should be removable");
    }

    #[test]
    fn lsp_member_completion_uses_host_schema_facts() {
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
        fs::write(
            &schema_path,
            r#"{
                "formatVersion": 1,
                "facts": {
                    "types": [
                        {
                            "name": "Player",
                            "fact": { "kind": "host", "name": "Player" }
                        }
                    ],
                    "fields": [
                        {
                            "owner": "Player",
                            "name": "level",
                            "fact": { "kind": "primitive", "name": "i64" }
                        }
                    ],
                    "methods": [
                        {
                            "owner": "Player",
                            "name": "level_up",
                            "fact": {
                                "kind": "function",
                                "params": [{ "kind": "primitive", "name": "i64" }],
                                "returns": { "kind": "primitive", "name": "bool" }
                            }
                        }
                    ]
                }
            }"#,
        )
        .expect("schema should be writable");

        let mut server = LspServer::new();
        let _ = response_value(server.handle_json(&request(
            1,
            "initialize",
            serde_json::json!({
                "processId": null,
                "rootUri": file_uri(&root),
                "capabilities": {}
            }),
        )));
        let _ = server.handle_json(&notification(
            "workspace/didChangeWatchedFiles",
            serde_json::json!({
                "changes": [{ "uri": file_uri(&config_path), "type": 1 }]
            }),
        ));
        let main_uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
        let text = "pub fn main(player: Player) { player.le }";
        let _ = notification_value(server.handle_json(&notification(
            "textDocument/didOpen",
            serde_json::json!({
                "textDocument": {
                    "uri": main_uri,
                    "languageId": "vela",
                    "version": 1,
                    "text": text
                }
            }),
        )));

        let response = response_value(server.handle_json(&request(
            2,
            "textDocument/completion",
            serde_json::json!({
                "textDocument": { "uri": main_uri },
                "position": {
                    "line": 0,
                    "character": text.find("le }").expect("member prefix should exist") + 2
                }
            }),
        )));

        assert_completion(&response, "level", 5, "i64");
        assert_completion(&response, "level_up", 2, "Function(i64) -> bool");
        fs::remove_dir_all(&root).expect("temporary workspace should be removable");
    }

    fn assert_completion(response: &serde_json::Value, label: &str, kind: u8, detail: &str) {
        assert_eq!(response["result"]["isIncomplete"], false);
        let Some(items) = response["result"]["items"].as_array() else {
            panic!("completion response should contain items");
        };
        assert!(
            items.iter().any(|item| {
                item["label"] == label && item["kind"] == kind && item["detail"] == detail
            }),
            "{items:?}"
        );
    }

    fn temp_workspace() -> PathBuf {
        let suffix = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => duration.as_nanos(),
            Err(error) => panic!("system time should be after UNIX_EPOCH: {error}"),
        };
        let root =
            std::env::temp_dir().join(format!("vela_lsp_server_{}_{}", std::process::id(), suffix));
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
}

mod signature {
    use super::{LspServer, notification, notification_value, request, response_value};

    #[test]
    fn lsp_signature_help_tracks_active_parameter() {
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
        let text = "pub fn grant(amount: i64, bonus: i64) -> bool { return true } pub fn main() { grant(1, 2) }";
        let _ = notification_value(server.handle_json(&notification(
            "textDocument/didOpen",
            serde_json::json!({
                "textDocument": {
                    "uri": "file:///workspace/scripts/game/main.vela",
                    "languageId": "vela",
                    "version": 1,
                    "text": text
                }
            }),
        )));

        let response = response_value(server.handle_json(&request(
            2,
            "textDocument/signatureHelp",
            serde_json::json!({
                "textDocument": { "uri": "file:///workspace/scripts/game/main.vela" },
                "position": {
                    "line": 0,
                    "character": text.find("2)").unwrap_or_else(|| {
                        panic!("signature fixture should contain second argument")
                    })
                }
            }),
        )));

        assert_eq!(response["result"]["activeSignature"], 0);
        assert_eq!(response["result"]["activeParameter"], 1);
        assert_eq!(
            response["result"]["signatures"][0]["label"],
            "grant(amount: i64, bonus: i64) -> bool"
        );
        assert_eq!(
            response["result"]["signatures"][0]["parameters"][1]["label"],
            "bonus: i64"
        );
    }
}

mod definition;
mod hover;
mod symbols;

mod file_watching {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        JsonRpcResult, JsonValue, LspServer, assert_workspace_progress, notification,
        notification_value, notification_values, publish_diagnostics_notifications, request,
        response_value,
    };

    fn temp_workspace() -> PathBuf {
        let suffix = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => duration.as_nanos(),
            Err(error) => panic!("system time should be after UNIX_EPOCH: {error}"),
        };
        let root =
            std::env::temp_dir().join(format!("vela_lsp_server_{}_{}", std::process::id(), suffix));
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
    fn initialized_server(root: &Path, config_path: &Path, helper_path: &Path) -> LspServer {
        let mut server = LspServer::new();
        let response = response_value(server.handle_json(&request(
            1,
            "initialize",
            serde_json::json!({
                "processId": null,
                "rootUri": file_uri(root),
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
                "changes": [
                    { "uri": file_uri(config_path), "type": 1 },
                    { "uri": file_uri(helper_path), "type": 1 }
                ]
            }),
        ));
        assert_eq!(watched, JsonRpcResult::None);
        server
    }
    fn open_main(server: &mut LspServer, root: &Path, import_module: &str) -> JsonValue {
        let main_uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
        notification_value(server.handle_json(&notification(
            "textDocument/didOpen",
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
        )))
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
        let mut server = LspServer::new();
        let _ = server.handle_json(&request(
            1,
            "initialize",
            serde_json::json!({
                "processId": null,
                "rootUri": file_uri(&root),
                "capabilities": {}
            }),
        ));
        let notifications = notification_values(server.handle_json(&notification(
            "workspace/didChangeWatchedFiles",
            serde_json::json!({
                "changes": [{ "uri": file_uri(&config_path), "type": 1 }]
            }),
        )));
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
        let cleared = notification_values(server.handle_json(&notification(
            "workspace/didChangeWatchedFiles",
            serde_json::json!({
                "changes": [{ "uri": file_uri(&config_path), "type": 2 }]
            }),
        )));
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

        let mut server = LspServer::new();
        let _ = server.handle_json(&request(
            1,
            "initialize",
            serde_json::json!({
                "processId": null,
                "rootUri": file_uri(&root),
                "capabilities": {}
            }),
        ));
        let notifications = notification_values(server.handle_json(&notification(
            "workspace/didChangeWatchedFiles",
            serde_json::json!({
                "changes": [{ "uri": file_uri(&config_path), "type": 1 }]
            }),
        )));

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

        let mut server = LspServer::new();
        let _ = server.handle_json(&request(
            1,
            "initialize",
            serde_json::json!({
                "processId": null,
                "rootUri": file_uri(&root),
                "capabilities": {}
            }),
        ));
        let invalid = notification_values(server.handle_json(&notification(
            "workspace/didChangeWatchedFiles",
            serde_json::json!({
                "changes": [{ "uri": file_uri(&config_path), "type": 1 }]
            }),
        )));
        assert_eq!(invalid.len(), 1, "{invalid:?}");
        fs::write(&schema_path, valid_schema_artifact()).expect("valid schema should be writable");

        let cleared = notification_values(server.handle_json(&notification(
            "workspace/didChangeWatchedFiles",
            serde_json::json!({
                "changes": [{ "uri": file_uri(&schema_path), "type": 2 }]
            }),
        )));

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
        let mut server = LspServer::new();
        let response = response_value(server.handle_json(&request(
            1,
            "initialize",
            serde_json::json!({
                "processId": null,
                "rootUri": file_uri(&game_root),
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
        assert_has_unresolved_import(&main);

        let notifications = notification_values(server.handle_json(&notification(
            "workspace/didChangeWorkspaceFolders",
            serde_json::json!({
                "event": {
                    "added": [{ "uri": file_uri(&scripts_root), "name": "scripts" }],
                    "removed": [{ "uri": file_uri(&game_root), "name": "game" }]
                }
            }),
        )));
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
        let notifications = notification_values(server.handle_json(&notification(
            "workspace/didChangeWatchedFiles",
            serde_json::json!({
                "changes": [
                    { "uri": file_uri(&helper_path), "type": 3 }
                ]
            }),
        )));

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

        let notifications = notification_values(server.handle_json(&notification(
            "workspace/didChangeWatchedFiles",
            serde_json::json!({
                "changes": [
                    { "uri": file_uri(&helper_path), "type": 3 }
                ]
            }),
        )));

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
        let _ = notification_values(server.handle_json(&notification(
            "workspace/didChangeWatchedFiles",
            serde_json::json!({
                "changes": [
                    { "uri": file_uri(&helper_path), "type": 3 },
                    { "uri": file_uri(&reward_path), "type": 1 }
                ]
            }),
        )));

        let main_uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
        let main = notification_value(server.handle_json(&notification(
            "textDocument/didChange",
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
        )));
        assert_no_unresolved_imports(&main);
        if let Err(error) = fs::remove_dir_all(&root) {
            panic!("temporary workspace should be removable: {error}");
        }
    }
}
