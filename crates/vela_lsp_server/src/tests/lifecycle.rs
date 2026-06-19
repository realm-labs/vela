use crate::LaunchConfiguration;

use super::{
    JsonRpcResult, JsonValue, LspServer, handle_notification, handle_request, notification_value,
    response_value,
};

#[test]
fn lsp_initialize_reports_capabilities() {
    let mut server = LspServer::new();
    let response = response_value(handle_request(
        &mut server,
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "capabilities": {}
        }),
    ));

    assert!(server.is_initialized());
    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], 1);
    assert_eq!(response["result"]["serverInfo"]["name"], "vela_lsp_server");
    assert_eq!(
        response["result"]["serverInfo"]["version"],
        env!("CARGO_PKG_VERSION")
    );
    assert_eq!(response["result"]["capabilities"]["workDoneProgress"], true);
    let capabilities = response["result"]["capabilities"]
        .as_object()
        .expect("capabilities should be an object");
    let mut capability_keys = capabilities.keys().map(String::as_str).collect::<Vec<_>>();
    capability_keys.sort_unstable();
    assert_eq!(
        capability_keys,
        vec![
            "callHierarchyProvider",
            "codeActionProvider",
            "completionProvider",
            "declarationProvider",
            "definitionProvider",
            "documentFormattingProvider",
            "documentHighlightProvider",
            "documentOnTypeFormattingProvider",
            "documentRangeFormattingProvider",
            "documentSymbolProvider",
            "foldingRangeProvider",
            "hoverProvider",
            "inlayHintProvider",
            "referencesProvider",
            "renameProvider",
            "selectionRangeProvider",
            "semanticTokensProvider",
            "signatureHelpProvider",
            "textDocumentSync",
            "typeDefinitionProvider",
            "workDoneProgress",
            "workspace",
            "workspaceSymbolProvider",
        ]
    );
    assert_eq!(
        response["result"]["capabilities"]["textDocumentSync"]["openClose"],
        true
    );
    assert_eq!(
        response["result"]["capabilities"]["textDocumentSync"]["change"],
        2
    );
    assert_eq!(
        response["result"]["capabilities"]["textDocumentSync"]["save"],
        false
    );
    assert_eq!(
        response["result"]["capabilities"]["completionProvider"]["resolveProvider"],
        true
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
        response["result"]["capabilities"]["declarationProvider"],
        serde_json::json!(true)
    );
    assert_eq!(
        response["result"]["capabilities"]["typeDefinitionProvider"],
        serde_json::json!(true)
    );
    assert_eq!(
        response["result"]["capabilities"]["referencesProvider"],
        serde_json::json!(true)
    );
    assert_eq!(
        response["result"]["capabilities"]["renameProvider"]["prepareProvider"],
        serde_json::json!(true)
    );
    assert_eq!(
        response["result"]["capabilities"]["codeActionProvider"]["codeActionKinds"],
        serde_json::json!(["quickfix"])
    );
    assert_eq!(
        response["result"]["capabilities"]["callHierarchyProvider"],
        serde_json::json!(true)
    );
    assert_eq!(
        response["result"]["capabilities"]["documentHighlightProvider"],
        serde_json::json!(true)
    );
    assert_eq!(
        response["result"]["capabilities"]["documentFormattingProvider"],
        serde_json::json!(true)
    );
    assert_eq!(
        response["result"]["capabilities"]["documentRangeFormattingProvider"],
        serde_json::json!(true)
    );
    assert_eq!(
        response["result"]["capabilities"]["documentOnTypeFormattingProvider"],
        serde_json::json!({
            "firstTriggerCharacter": "}",
            "moreTriggerCharacter": ["\n"]
        })
    );
    assert_eq!(
        response["result"]["capabilities"]["documentSymbolProvider"],
        serde_json::json!(true)
    );
    assert_eq!(
        response["result"]["capabilities"]["foldingRangeProvider"],
        serde_json::json!(true)
    );
    assert_eq!(
        response["result"]["capabilities"]["selectionRangeProvider"],
        serde_json::json!(true)
    );
    assert_eq!(
        response["result"]["capabilities"]["semanticTokensProvider"]["full"]["delta"],
        serde_json::json!(true)
    );
    assert_eq!(
        response["result"]["capabilities"]["semanticTokensProvider"]["range"],
        serde_json::json!(true)
    );
    assert_eq!(
        response["result"]["capabilities"]["inlayHintProvider"]["resolveProvider"],
        serde_json::json!(false)
    );
    assert!(
        response["result"]["capabilities"]["semanticTokensProvider"]["legend"]["tokenTypes"]
            .as_array()
            .is_some_and(|types| types.iter().any(|token_type| token_type == "keyword"))
    );
    assert_eq!(
        response["result"]["capabilities"]["workspaceSymbolProvider"],
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
fn server_info_reports_version() {
    let mut server = LspServer::new();
    let response = response_value(handle_request(
        &mut server,
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "capabilities": {}
        }),
    ));

    assert_eq!(response["result"]["serverInfo"]["name"], "vela_lsp_server");
    assert_eq!(
        response["result"]["serverInfo"]["version"],
        env!("CARGO_PKG_VERSION")
    );
}

#[test]
fn lsp_rejects_repeated_initialize_without_resetting_state() {
    let mut server = LspServer::new();
    let first = response_value(handle_request(
        &mut server,
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": "file:///workspace",
            "initializationOptions": {
                "host": {
                    "schema": "target/vela/schema.json"
                }
            },
            "capabilities": {
                "workspace": {
                    "didChangeWatchedFiles": {
                        "dynamicRegistration": true
                    }
                }
            }
        }),
    ));
    let repeated = response_value(handle_request(
        &mut server,
        2,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": "file:///other",
            "capabilities": {}
        }),
    ));
    let registration = notification_value(handle_notification(
        &mut server,
        "initialized",
        serde_json::json!({}),
    ));
    let watchers = registration["params"]["registrations"][0]["registerOptions"]["watchers"]
        .as_array()
        .expect("watcher registration should include watchers");

    assert_eq!(first["id"], 1);
    assert_eq!(repeated["id"], 2);
    assert_eq!(repeated["error"]["code"], -32600);
    assert_eq!(
        repeated["error"]["message"],
        "server is already initialized"
    );
    assert!(watchers.iter().any(|watcher| {
        watcher["globPattern"]["baseUri"] == "file:///workspace"
            && watcher["globPattern"]["pattern"] == "**/*.vela"
    }));
    assert!(!watchers.iter().any(|watcher| {
        watcher["globPattern"]["baseUri"] == "file:///other"
            && watcher["globPattern"]["pattern"] == "**/*.vela"
    }));
}

#[test]
fn lsp_rejects_malformed_initialize_without_initializing() {
    let mut server = LspServer::new();
    let malformed = response_value(handle_request(
        &mut server,
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "capabilities": []
        }),
    ));
    let initialize = response_value(handle_request(
        &mut server,
        2,
        "initialize",
        serde_json::json!({
            "processId": null,
            "capabilities": {}
        }),
    ));

    assert_eq!(malformed["id"], 1);
    assert_eq!(malformed["error"]["code"], -32602);
    assert!(
        malformed["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("invalid initialize params"))
    );
    assert_eq!(initialize["id"], 2);
    assert_eq!(
        initialize["result"]["serverInfo"]["name"],
        "vela_lsp_server"
    );
    assert!(server.is_initialized());
}

#[test]
fn lsp_initialize_notification_does_not_initialize() {
    let mut server = LspServer::new();
    let notification_result = handle_notification(
        &mut server,
        "initialize",
        serde_json::json!({
            "processId": null,
            "capabilities": {}
        }),
    );
    let early_hover = response_value(handle_request(
        &mut server,
        1,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": "file:///workspace/scripts/main.vela" },
            "position": { "line": 0, "character": 0 }
        }),
    ));
    let initialize = response_value(handle_request(
        &mut server,
        2,
        "initialize",
        serde_json::json!({
            "processId": null,
            "capabilities": {}
        }),
    ));

    assert_eq!(notification_result, JsonRpcResult::None);
    assert_eq!(early_hover["id"], 1);
    assert_eq!(early_hover["error"]["code"], -32002);
    assert_eq!(
        early_hover["error"]["message"],
        "server has not been initialized"
    );
    assert_eq!(initialize["id"], 2);
    assert_eq!(
        initialize["result"]["serverInfo"]["name"],
        "vela_lsp_server"
    );
    assert!(server.is_initialized());
}

#[test]
fn lsp_initialized_notification_has_no_response() {
    let mut server = LspServer::new();
    let _ = response_value(handle_request(
        &mut server,
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "capabilities": {}
        }),
    ));
    let result = handle_notification(&mut server, "initialized", serde_json::json!({}));

    assert!(server.is_initialized());
    assert_eq!(result, JsonRpcResult::None);
}

#[test]
fn lsp_rejects_requests_before_initialize() {
    let mut server = LspServer::new();

    let response = response_value(handle_request(
        &mut server,
        1,
        "textDocument/completion",
        serde_json::json!({
            "textDocument": { "uri": "file:///workspace/scripts/main.vela" },
            "position": { "line": 0, "character": 0 }
        }),
    ));

    assert!(!server.is_initialized());
    assert_eq!(response["id"], 1);
    assert_eq!(response["error"]["code"], -32002);
    assert_eq!(
        response["error"]["message"],
        "server has not been initialized"
    );
}

#[test]
fn lsp_initialized_notification_before_initialize_does_not_unlock_requests() {
    let mut server = LspServer::new();
    let initialized = handle_notification(&mut server, "initialized", serde_json::json!({}));
    let response = response_value(handle_request(
        &mut server,
        1,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": "file:///workspace/scripts/main.vela" },
            "position": { "line": 0, "character": 0 }
        }),
    ));

    assert_eq!(initialized, JsonRpcResult::None);
    assert!(!server.is_initialized());
    assert_eq!(response["id"], 1);
    assert_eq!(response["error"]["code"], -32002);
    assert_eq!(
        response["error"]["message"],
        "server has not been initialized"
    );
}

#[test]
fn lsp_did_save_is_not_advertised_and_has_no_response() {
    let mut server = LspServer::new();
    let initialize = response_value(handle_request(
        &mut server,
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "capabilities": {}
        }),
    ));

    assert_eq!(
        initialize["result"]["capabilities"]["textDocumentSync"]["save"],
        false
    );

    let result = handle_notification(
        &mut server,
        "textDocument/didSave",
        serde_json::json!({
            "textDocument": { "uri": "file:///workspace/scripts/main.vela" }
        }),
    );

    assert_eq!(result, JsonRpcResult::None);
}

#[test]
fn lsp_initialized_registers_watched_files_when_supported() {
    let mut server = LspServer::new();
    let _ = response_value(handle_request(
        &mut server,
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": "file:///workspace",
            "initializationOptions": {
                "host": {
                    "schema": "target/vela/schema.json"
                }
            },
            "capabilities": {
                "workspace": {
                    "didChangeWatchedFiles": {
                        "dynamicRegistration": true
                    }
                }
            }
        }),
    ));

    let registration = notification_value(handle_notification(
        &mut server,
        "initialized",
        serde_json::json!({}),
    ));

    assert_eq!(registration["jsonrpc"], "2.0");
    assert_eq!(registration["method"], "client/registerCapability");
    let watched_files = &registration["params"]["registrations"][0];
    assert_eq!(watched_files["id"], "vela/watched-files");
    assert_eq!(watched_files["method"], "workspace/didChangeWatchedFiles");
    let watchers = watched_files["registerOptions"]["watchers"]
        .as_array()
        .expect("watcher registration should include watchers");
    assert!(watchers.iter().all(|watcher| watcher["kind"] == 7));
    assert!(watchers.iter().any(|watcher| {
        watcher["globPattern"]["baseUri"] == "file:///workspace"
            && watcher["globPattern"]["pattern"] == "**/*.vela"
    }));
    assert!(watchers.iter().any(|watcher| {
        watcher["globPattern"]["baseUri"] == "file:///workspace"
            && watcher["globPattern"]["pattern"] == "vela.toml"
    }));
    assert!(watchers.iter().any(|watcher| {
        watcher["globPattern"]
            .as_str()
            .is_some_and(|pattern| pattern.ends_with("/workspace/target/vela/schema.json"))
    }));

    let repeated = handle_notification(&mut server, "initialized", serde_json::json!({}));
    assert_eq!(repeated, JsonRpcResult::None);
}

#[test]
fn lsp_initialized_skips_watched_files_when_disabled() {
    let mut configuration = LaunchConfiguration::new();
    configuration.set_watch_files_enabled(false);
    let mut server = LspServer::with_launch_configuration(configuration);
    let _ = response_value(handle_request(
        &mut server,
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": "file:///workspace",
            "capabilities": {
                "workspace": {
                    "didChangeWatchedFiles": {
                        "dynamicRegistration": true
                    }
                }
            }
        }),
    ));

    let result = handle_notification(&mut server, "initialized", serde_json::json!({}));

    assert_eq!(result, JsonRpcResult::None);
}

#[test]
fn lsp_initialized_ignores_empty_host_schema_setting() {
    let mut server = LspServer::new();
    let _ = response_value(handle_request(
        &mut server,
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": "file:///workspace",
            "initializationOptions": {
                "host": {
                    "schema": ""
                }
            },
            "capabilities": {
                "workspace": {
                    "didChangeWatchedFiles": {
                        "dynamicRegistration": true
                    }
                }
            }
        }),
    ));

    let registration = notification_value(handle_notification(
        &mut server,
        "initialized",
        serde_json::json!({}),
    ));
    let watchers = registration["params"]["registrations"][0]["registerOptions"]["watchers"]
        .as_array()
        .expect("watcher registration should include watchers");

    assert!(watchers.iter().any(|watcher| {
        watcher["globPattern"]["baseUri"] == "file:///workspace"
            && watcher["globPattern"]["pattern"] == "**/*.vela"
    }));
    assert!(watchers.iter().any(|watcher| {
        watcher["globPattern"]["baseUri"] == "file:///workspace"
            && watcher["globPattern"]["pattern"] == "vela.toml"
    }));
    assert!(
        watchers
            .iter()
            .all(|watcher| watcher["globPattern"] != "/workspace")
    );
    assert_eq!(watchers.len(), 2, "{watchers:?}");
}

#[test]
fn lsp_did_open_with_empty_host_schema_has_no_schema_diagnostic() {
    let mut server = LspServer::new();
    let _ = response_value(handle_request(
        &mut server,
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": "file:///workspace",
            "initializationOptions": {
                "host": {
                    "schema": ""
                }
            },
            "capabilities": {}
        }),
    ));

    let diagnostics = notification_value(handle_notification(
        &mut server,
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": "file:///workspace/main.vela",
                "languageId": "vela",
                "version": 1,
                "text": "fn main() { return 1; }"
            }
        }),
    ));
    let diagnostics = diagnostics["params"]["diagnostics"]
        .as_array()
        .expect("didOpen should publish diagnostics");

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic["code"] != "schema::unavailable"
                && diagnostic["code"] != "schema::diagnostic"),
        "{diagnostics:?}"
    );
}

#[test]
fn lsp_ignores_client_response_to_server_request() {
    let mut server = LspServer::new();

    let result = server.handle_json(
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": "vela/watched-files",
            "result": null
        })
        .to_string(),
    );

    assert_eq!(result, JsonRpcResult::None);
}

#[test]
fn lsp_missing_method_request_reports_invalid_request() {
    let mut server = LspServer::new();

    let response = response_value(
        server.handle_json(
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": 7,
                "params": {}
            })
            .to_string(),
        ),
    );

    assert_eq!(response["id"], 7);
    assert_eq!(response["error"]["code"], -32600);
    assert_eq!(response["error"]["message"], "missing JSON-RPC method");
}

#[test]
fn lsp_implementation_request_is_not_advertised_or_supported() {
    let mut server = LspServer::new();
    let initialize = response_value(handle_request(
        &mut server,
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "capabilities": {}
        }),
    ));

    assert!(
        initialize["result"]["capabilities"]["implementationProvider"].is_null(),
        "{initialize:?}"
    );
    assert!(
        initialize["result"]["capabilities"]["documentLinkProvider"].is_null(),
        "{initialize:?}"
    );

    let implementation = response_value(handle_request(
        &mut server,
        2,
        "textDocument/implementation",
        serde_json::json!({
            "textDocument": { "uri": "file:///workspace/scripts/main.vela" },
            "position": { "line": 0, "character": 0 }
        }),
    ));
    let document_link = response_value(handle_request(
        &mut server,
        3,
        "textDocument/documentLink",
        serde_json::json!({
            "textDocument": { "uri": "file:///workspace/scripts/main.vela" }
        }),
    ));
    let unsupported_notification = handle_notification(
        &mut server,
        "textDocument/documentLink",
        serde_json::json!({
            "textDocument": { "uri": "file:///workspace/scripts/main.vela" }
        }),
    );
    let hover = response_value(handle_request(
        &mut server,
        4,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": "file:///workspace/scripts/main.vela" },
            "position": { "line": 0, "character": 0 }
        }),
    ));

    assert_eq!(implementation["id"], 2);
    assert_eq!(implementation["error"]["code"], -32601);
    assert_eq!(document_link["id"], 3);
    assert_eq!(document_link["error"]["code"], -32601);
    assert_eq!(unsupported_notification, JsonRpcResult::None);
    assert_eq!(hover["id"], 4);
    assert!(hover["result"].is_null());
}

#[test]
fn lsp_cancellation_before_request_does_not_poison_request_id() {
    let mut server = LspServer::new();
    let cancel = handle_notification(
        &mut server,
        "$/cancelRequest",
        serde_json::json!({ "id": 7 }),
    );
    assert_eq!(cancel, JsonRpcResult::None);

    let response = response_value(handle_request(
        &mut server,
        7,
        "initialize",
        serde_json::json!({
            "processId": null,
            "capabilities": {}
        }),
    ));

    assert!(server.is_initialized());
    assert_eq!(response["id"], 7);
    assert_eq!(response["result"]["serverInfo"]["name"], "vela_lsp_server");
}

#[test]
fn lsp_cancellation_ignores_unknown_request_id() {
    let mut server = LspServer::new();
    let cancel = handle_notification(
        &mut server,
        "$/cancelRequest",
        serde_json::json!({ "id": 404 }),
    );
    let response = response_value(handle_request(
        &mut server,
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "capabilities": {}
        }),
    ));

    assert_eq!(cancel, JsonRpcResult::None);
    assert!(server.is_initialized());
    assert_eq!(response["id"], 1);
    assert_eq!(response["result"]["serverInfo"]["name"], "vela_lsp_server");
}

#[test]
fn lsp_cancellation_request_is_rejected_without_poisoning_later_requests() {
    let mut server = LspServer::new();
    let cancel = response_value(handle_request(
        &mut server,
        1,
        "$/cancelRequest",
        serde_json::json!({ "id": 2 }),
    ));
    let initialize = response_value(handle_request(
        &mut server,
        2,
        "initialize",
        serde_json::json!({
            "processId": null,
            "capabilities": {}
        }),
    ));

    assert_eq!(cancel["id"], 1);
    assert_eq!(cancel["error"]["code"], -32600);
    assert_eq!(
        cancel["error"]["message"],
        "`$/cancelRequest` must be sent as a notification"
    );
    assert_eq!(initialize["id"], 2);
    assert_eq!(
        initialize["result"]["serverInfo"]["name"],
        "vela_lsp_server"
    );
    assert!(server.is_initialized());
}

#[test]
fn lsp_cancellation_ignores_malformed_params() {
    let mut server = LspServer::new();
    let cancel = handle_notification(
        &mut server,
        "$/cancelRequest",
        serde_json::json!({ "id": { "nested": 7 } }),
    );
    let initialize = response_value(handle_request(
        &mut server,
        7,
        "initialize",
        serde_json::json!({
            "processId": null,
            "capabilities": {}
        }),
    ));

    assert_eq!(cancel, JsonRpcResult::None);
    assert_eq!(initialize["id"], 7);
    assert_eq!(
        initialize["result"]["serverInfo"]["name"],
        "vela_lsp_server"
    );
    assert!(server.is_initialized());
}

#[test]
fn lsp_cancellation_ignores_completed_request_id() {
    let mut server = LspServer::new();
    let initialize = response_value(handle_request(
        &mut server,
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "capabilities": {}
        }),
    ));
    let cancel = handle_notification(
        &mut server,
        "$/cancelRequest",
        serde_json::json!({ "id": 1 }),
    );
    let hover = response_value(handle_request(
        &mut server,
        2,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": "file:///workspace/scripts/main.vela" },
            "position": { "line": 0, "character": 0 }
        }),
    ));

    assert_eq!(initialize["id"], 1);
    assert_eq!(cancel, JsonRpcResult::None);
    assert_eq!(hover["id"], 2);
    assert!(hover["result"].is_null());
}

#[test]
fn lsp_shutdown_exits_without_background_tasks() {
    let mut server = LspServer::new();
    let _ = response_value(handle_request(
        &mut server,
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "capabilities": {}
        }),
    ));
    let response = response_value(handle_request(&mut server, 2, "shutdown", JsonValue::Null));
    let exit = handle_notification(&mut server, "exit", JsonValue::Null);

    assert_eq!(response["result"], JsonValue::Null);
    assert!(server.is_shutdown_requested());
    assert!(server.is_exited());
    assert_eq!(exit, JsonRpcResult::None);
}

#[test]
fn lsp_rejects_requests_after_shutdown_until_exit() {
    let mut server = LspServer::new();
    let _ = response_value(handle_request(
        &mut server,
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "capabilities": {}
        }),
    ));
    let shutdown = response_value(handle_request(&mut server, 2, "shutdown", JsonValue::Null));

    assert_eq!(shutdown["result"], JsonValue::Null);
    assert!(server.is_shutdown_requested());

    let completion = response_value(handle_request(
        &mut server,
        3,
        "textDocument/completion",
        serde_json::json!({
            "textDocument": { "uri": "file:///workspace/scripts/main.vela" },
            "position": { "line": 0, "character": 0 }
        }),
    ));
    assert_eq!(completion["id"], 3);
    assert_eq!(completion["error"]["code"], -32600);
    assert_eq!(completion["error"]["message"], "server has shut down");

    let exit = handle_notification(&mut server, "exit", JsonValue::Null);
    assert!(server.is_exited());
    assert_eq!(exit, JsonRpcResult::None);
}

#[test]
fn lsp_shutdown_notification_does_not_shutdown() {
    let mut server = LspServer::new();
    let _ = response_value(handle_request(
        &mut server,
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "capabilities": {}
        }),
    ));
    let notification_result = handle_notification(&mut server, "shutdown", JsonValue::Null);
    assert_eq!(notification_result, JsonRpcResult::None);
    assert!(!server.is_shutdown_requested());

    let hover = response_value(handle_request(
        &mut server,
        2,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": "file:///workspace/scripts/main.vela" },
            "position": { "line": 0, "character": 0 }
        }),
    ));
    let shutdown = response_value(handle_request(&mut server, 3, "shutdown", JsonValue::Null));

    assert_eq!(hover["id"], 2);
    assert!(hover["result"].is_null());
    assert_eq!(shutdown["id"], 3);
    assert_eq!(shutdown["result"], JsonValue::Null);
    assert!(server.is_shutdown_requested());
}

#[test]
fn lsp_rejects_shutdown_before_initialize_without_closing() {
    let mut server = LspServer::new();
    let shutdown = response_value(handle_request(&mut server, 1, "shutdown", JsonValue::Null));
    let initialize = response_value(handle_request(
        &mut server,
        2,
        "initialize",
        serde_json::json!({
            "processId": null,
            "capabilities": {}
        }),
    ));

    assert_eq!(shutdown["id"], 1);
    assert_eq!(shutdown["error"]["code"], -32002);
    assert_eq!(
        shutdown["error"]["message"],
        "server has not been initialized"
    );
    assert!(!server.is_shutdown_requested());
    assert_eq!(initialize["id"], 2);
    assert_eq!(
        initialize["result"]["serverInfo"]["name"],
        "vela_lsp_server"
    );
    assert!(server.is_initialized());
}

#[test]
fn lsp_exit_request_reports_invalid_request_and_exits() {
    let mut server = LspServer::new();
    let _ = response_value(handle_request(
        &mut server,
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "capabilities": {}
        }),
    ));
    let exit = response_value(handle_request(&mut server, 2, "exit", JsonValue::Null));
    let hover = handle_request(
        &mut server,
        3,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": "file:///workspace/scripts/main.vela" },
            "position": { "line": 0, "character": 0 }
        }),
    );

    assert_eq!(exit["id"], 2);
    assert_eq!(exit["error"]["code"], -32600);
    assert_eq!(
        exit["error"]["message"],
        "`exit` must be sent as a notification"
    );
    assert!(server.is_exited());
    assert_eq!(hover, JsonRpcResult::None);
}

#[test]
fn lsp_ignores_messages_after_exit() {
    let mut server = LspServer::new();
    let _ = response_value(handle_request(
        &mut server,
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "capabilities": {}
        }),
    ));
    let exit = handle_notification(&mut server, "exit", JsonValue::Null);

    let hover = handle_request(
        &mut server,
        2,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": "file:///workspace/scripts/main.vela" },
            "position": { "line": 0, "character": 0 }
        }),
    );
    let did_open = handle_notification(
        &mut server,
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": "file:///workspace/scripts/main.vela",
                "languageId": "vela",
                "version": 1,
                "text": "let broken ="
            }
        }),
    );
    let malformed = server.handle_json("{not json");

    assert!(server.is_exited());
    assert_eq!(exit, JsonRpcResult::None);
    assert_eq!(hover, JsonRpcResult::None);
    assert_eq!(did_open, JsonRpcResult::None);
    assert_eq!(malformed, JsonRpcResult::None);
}
