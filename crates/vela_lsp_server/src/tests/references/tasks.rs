use super::*;

#[test]
fn lsp_references_find_detached_worker_and_continuation_operands() {
    let mut server = TestServer::new();
    let _ = initialize(&mut server);
    let text = "\
async fn worker(value: i64) -> i64 { return value }
fn continuation(result: Result<i64, task::Error>) {}
pub fn main() { task::spawn_scoped_then(worker(7), continuation) }";
    let uri = "file:///workspace/scripts/game/main.vela";
    open_document(&mut server, uri, text);

    assert_task_references(&mut server, 2, uri, text, "worker", 0);
    assert_task_references(&mut server, 3, uri, text, "continuation", 1);
}

fn assert_task_references(
    server: &mut TestServer,
    id: i32,
    uri: &str,
    text: &str,
    name: &str,
    declaration_line: usize,
) {
    let response = response_value(request::<lsp_types::request::References>(
        server,
        id,
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": declaration_line,
                "character": line(text, declaration_line).find(name).expect("declaration")
            },
            "context": { "includeDeclaration": true }
        }),
    ));
    let references = response["result"]
        .as_array()
        .expect("references response should be an array");
    assert_eq!(references.len(), 2, "{references:?}");
    assert_reference(
        references,
        uri,
        declaration_line,
        line(text, declaration_line)
            .find(name)
            .expect("declaration"),
    );
    assert_reference(
        references,
        uri,
        2,
        line(text, 2)
            .rfind(name)
            .unwrap_or_else(|| panic!("{name} operand")),
    );
}
