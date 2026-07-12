use super::{TestServer, request, response_value};

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
fn lsp_completion_resolve_rejects_unknown_payload_kind() {
    let mut server = TestServer::new();
    initialize_server(&mut server);
    let response = response_value(request::<lsp_types::request::ResolveCompletionItem>(
        &mut server,
        1,
        serde_json::json!({
            "label": "Mystery",
            "data": {
                "source": "vela",
                "resolve": {
                    "kind": "mystery"
                }
            }
        }),
    ));

    assert_eq!(response["id"], 1);
    assert_eq!(response["error"]["code"], -32600);
    assert!(
        response["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("invalid completionItem/resolve payload")),
        "{response:?}"
    );
}

#[test]
fn lsp_completion_resolve_passes_through_items_without_payload() {
    let mut server = TestServer::new();
    initialize_server(&mut server);
    let response = response_value(request::<lsp_types::request::ResolveCompletionItem>(
        &mut server,
        2,
        serde_json::json!({
            "label": "plain",
            "kind": 6,
            "data": {
                "source": "vela"
            }
        }),
    ));

    assert_eq!(response["id"], 2);
    assert_eq!(response["result"]["label"], "plain");
    assert!(response["result"].get("documentation").is_none());
}
