use super::*;

#[test]
fn lsp_call_hierarchy_models_detached_worker_and_continuation_edges() {
    let mut server = TestServer::new();
    let _ = initialize(&mut server, "file:///workspace/scripts");
    let text = "\
async fn worker(value: i64) -> i64 { return value }
fn continuation(result: Result<i64, task::Error>) {}
pub fn main() { task::spawn_scoped_then(worker(7), continuation) }";
    let uri = "file:///workspace/scripts/game/main.vela";
    open_document(&mut server, uri, text);

    let continuation_items = prepare(&mut server, 2, uri, text, 1, "continuation");
    let incoming = response_value(request::<lsp_types::request::CallHierarchyIncomingCalls>(
        &mut server,
        3,
        serde_json::json!({ "item": continuation_items[0].clone() }),
    ));
    let incoming_calls = incoming["result"]
        .as_array()
        .expect("incomingCalls response should be an array");
    assert_eq!(incoming_calls.len(), 1, "{incoming_calls:?}");
    assert_eq!(incoming_calls[0]["from"]["name"], "main");
    assert_call_range(
        incoming_calls[0]["fromRanges"]
            .as_array()
            .expect("continuation edge should include a range"),
        2,
        line(text, 2)
            .rfind("continuation")
            .expect("continuation operand"),
    );

    let main_items = prepare(&mut server, 4, uri, text, 2, "main");
    let outgoing = response_value(request::<lsp_types::request::CallHierarchyOutgoingCalls>(
        &mut server,
        5,
        serde_json::json!({ "item": main_items[0].clone() }),
    ));
    let outgoing_calls = outgoing["result"]
        .as_array()
        .expect("outgoingCalls response should be an array");
    assert_eq!(outgoing_calls.len(), 2, "{outgoing_calls:?}");
    assert_outgoing_call(
        outgoing_calls,
        "worker",
        uri,
        2,
        line(text, 2).find("worker").expect("worker operand"),
    );
    assert_outgoing_call(
        outgoing_calls,
        "continuation",
        uri,
        2,
        line(text, 2)
            .rfind("continuation")
            .expect("continuation operand"),
    );
}

fn prepare(
    server: &mut TestServer,
    id: i32,
    uri: &str,
    text: &str,
    target_line: usize,
    name: &str,
) -> Vec<serde_json::Value> {
    let response = response_value(request::<lsp_types::request::CallHierarchyPrepare>(
        server,
        id,
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": target_line,
                "character": line(text, target_line).find(name).expect("callable name")
            }
        }),
    ));
    response["result"]
        .as_array()
        .expect("prepareCallHierarchy response should be an array")
        .clone()
}
