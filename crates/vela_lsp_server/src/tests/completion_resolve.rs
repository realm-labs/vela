use super::{TestServer, request, response_value};

mod lifecycle;

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

#[test]
fn completion_resolve_rejects_malformed_payloads_without_poisoning_later_requests() {
    let mut server = TestServer::new();
    initialize_server(&mut server);
    for (index, resolve) in [
        serde_json::json!(null),
        serde_json::json!(false),
        serde_json::json!([]),
        serde_json::json!({}),
        serde_json::json!({"kind":7}),
        serde_json::json!({"kind":"mystery"}),
        serde_json::json!({"kind":"documentation"}),
        serde_json::json!({"kind":"documentation","symbol":null}),
        serde_json::json!({"kind":"documentation","symbol":{"kind":"schema"}}),
        serde_json::json!({"kind":"documentation","symbol":{"kind":"schema","name":1}}),
        serde_json::json!({"kind":"documentation","symbol":{"kind":1,"name":"Player"}}),
        serde_json::json!({"kind":"documentation","symbol":{"kind":"mystery","name":"Player"}}),
    ]
    .into_iter()
    .enumerate()
    {
        let id = 10 + i32::try_from(index).expect("bounded") * 2;
        let response = response_value(request::<lsp_types::request::ResolveCompletionItem>(
            &mut server,
            id,
            serde_json::json!({"label":"Player","data":{"source":"vela","resolve":resolve}}),
        ));
        assert_eq!(response["id"], id);
        assert_eq!(response["error"]["code"], -32600, "{response}");
        assert!(response.get("result").is_none(), "{response}");
        let unchanged = serde_json::json!({"label":"plain","kind":6,
            "documentation":{"kind":"markdown","value":"Client-provided docs."},
            "data":{"source":"vela"}});
        let recovered = response_value(request::<lsp_types::request::ResolveCompletionItem>(
            &mut server,
            id + 1,
            unchanged.clone(),
        ));
        assert_eq!(recovered["id"], id + 1);
        assert!(recovered.get("error").is_none());
        assert_eq!(recovered["result"], unchanged);
    }
}

#[test]
fn completion_resolve_unknown_symbols_preserve_every_existing_edit_and_label_field() {
    let mut server = TestServer::new();
    initialize_server(&mut server);
    for (index, owner) in ["source", "schema", "builtin", "local"]
        .into_iter()
        .enumerate()
    {
        let candidate = serde_json::json!({
            "label":"missing", "labelDetails":{"detail":"()","description":"fixture"},
            "kind":3,"detail":"unknown", "sortText":"0001_missing","filterText":"missing",
            "insertText":"missing($0)","insertTextFormat":2,"preselect":true,"tags":[1],
            "textEdit":{"range":{"start":{"line":1,"character":8},"end":{"line":1,"character":11}},"newText":"missing($0)"},
            "additionalTextEdits":[{"range":{"start":{"line":0,"character":0},"end":{"line":0,"character":0}},"newText":"// 中😀\n"}],
            "data":{"source":"vela","resolve":{"kind":"documentation","symbol":{"kind":owner,"name":"not_present_in_fixture"}}}
        });
        let id = 1 + i32::try_from(index).expect("bounded");
        let response = response_value(request::<lsp_types::request::ResolveCompletionItem>(
            &mut server,
            id,
            candidate.clone(),
        ));
        assert_eq!(response["id"], id);
        assert!(response.get("error").is_none(), "{response}");
        assert_eq!(response["result"], candidate);
        assert!(response["result"].get("documentation").is_none());
    }
}
