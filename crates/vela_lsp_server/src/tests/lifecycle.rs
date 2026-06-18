use super::{
    JsonRpcResult, JsonValue, LspServer, notification, notification_value, request, response_value,
};

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
    let response = response_value(server.handle_json(&request(
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "capabilities": {}
        }),
    )));

    assert_eq!(response["result"]["serverInfo"]["name"], "vela_lsp_server");
    assert_eq!(
        response["result"]["serverInfo"]["version"],
        env!("CARGO_PKG_VERSION")
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
fn lsp_initialized_registers_watched_files_when_supported() {
    let mut server = LspServer::new();
    let _ = response_value(server.handle_json(&request(
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
    )));

    let registration =
        notification_value(server.handle_json(&notification("initialized", serde_json::json!({}))));

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
    let initialize = response_value(server.handle_json(&request(
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "capabilities": {}
        }),
    )));

    assert!(
        initialize["result"]["capabilities"]["implementationProvider"].is_null(),
        "{initialize:?}"
    );

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/implementation",
        serde_json::json!({
            "textDocument": { "uri": "file:///workspace/scripts/main.vela" },
            "position": { "line": 0, "character": 0 }
        }),
    )));

    assert_eq!(response["id"], 2);
    assert_eq!(response["error"]["code"], -32601);
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
