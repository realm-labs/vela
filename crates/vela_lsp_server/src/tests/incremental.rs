use super::{LspServer, notification, notification_value, request, response_value};

#[test]
fn lsp_did_change_body_edit_avoids_project_and_hir_rebuild() {
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

    let main_uri = "file:///workspace/scripts/game/main.vela";
    let reward_uri = "file:///workspace/scripts/game/reward.vela";
    let config_uri = "file:///workspace/scripts/game/config.vela";
    open_document(&mut server, reward_uri, "pub fn grant() { return 1 }");
    open_document(&mut server, config_uri, "pub const value = 1");
    open_document(
        &mut server,
        main_uri,
        "use game::reward::grant\npub fn main() { return grant() }",
    );

    let before_parse_count = server.databases.parse_db().parse_count();
    let before_project_rebuild_count = server.databases.project_db().rebuild_count();
    let before_hir_rebuild_count = server.databases.hir_db().rebuild_count();

    let change = notification_value(server.handle_json(&notification(
        "textDocument/didChange",
        serde_json::json!({
            "textDocument": {
                "uri": reward_uri,
                "version": 2
            },
            "contentChanges": [
                { "text": "pub fn grant() { return 2 }" }
            ]
        }),
    )));

    assert_eq!(change["method"], "textDocument/publishDiagnostics");
    assert_eq!(change["params"]["uri"], reward_uri);
    assert_eq!(change["params"]["diagnostics"], serde_json::json!([]));
    assert_eq!(
        server.databases.parse_db().parse_count(),
        before_parse_count + 1,
        "body-only didChange should reparse only the edited document"
    );
    assert_eq!(
        server.databases.project_db().rebuild_count(),
        before_project_rebuild_count,
        "body-only didChange must preserve declaration/import indexes"
    );
    assert_eq!(
        server.databases.hir_db().rebuild_count(),
        before_hir_rebuild_count,
        "body-only didChange must not force a HIR graph rebuild"
    );
}

fn open_document(server: &mut LspServer, uri: &str, text: &str) {
    let diagnostics = notification_value(server.handle_json(&notification(
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
    assert_eq!(diagnostics["method"], "textDocument/publishDiagnostics");
    assert_eq!(diagnostics["params"]["uri"], uri);
    assert_eq!(diagnostics["params"]["diagnostics"], serde_json::json!([]));
}
