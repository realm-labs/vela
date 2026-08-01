use super::*;

#[test]
fn lsp_signature_help_describes_static_scoped_task_operands() {
    let mut server = TestServer::new();
    let _ = response_value(request::<lsp_types::request::Initialize>(
        &mut server,
        1,
        serde_json::json!({
            "processId": null,
            "rootUri": "file:///workspace/scripts",
            "capabilities": {}
        }),
    ));
    let uri = "file:///workspace/scripts/game/main.vela";
    let text = "async fn worker() -> i64 { return 1; } fn done(result: Result<i64, task::Error>) {} fn main() { task::spawn_scoped_then(worker(), done); }";
    let _ = notification_value(notify::<lsp_types::notification::DidOpenTextDocument>(
        &mut server,
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    ));

    let response = response_value(request::<lsp_types::request::SignatureHelpRequest>(
        &mut server,
        2,
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 0,
                "character": text.rfind("done").expect("continuation argument")
            }
        }),
    ));

    assert_eq!(response["result"]["activeParameter"], 1);
    assert_eq!(
        response["result"]["signatures"][0]["label"],
        "task::spawn_scoped_then(invocation: Any, continuation: Any) -> ()"
    );
    assert_eq!(
        response["result"]["signatures"][0]["parameters"][0]["label"],
        "invocation: Any"
    );
    assert_eq!(
        response["result"]["signatures"][0]["parameters"][1]["label"],
        "continuation: Any"
    );
}
