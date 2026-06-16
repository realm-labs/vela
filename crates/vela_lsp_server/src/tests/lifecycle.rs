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
        response["result"]["capabilities"]["semanticTokensProvider"]["full"]["delta"],
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
