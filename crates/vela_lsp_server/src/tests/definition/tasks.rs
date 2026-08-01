use super::*;

#[test]
fn lsp_definition_follows_detached_worker_and_continuation_operands() {
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
    let text = "\
async fn worker(value: i64) -> i64 { return value }
fn continuation(result: Result<i64, task::Error>) {}
pub fn main() { task::spawn_scoped_then(worker(7), continuation) }";
    let uri = "file:///workspace/scripts/game/main.vela";
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

    assert_definition(&mut server, 2, uri, text, "worker", 0);
    assert_definition(&mut server, 3, uri, text, "continuation", 1);
}

fn assert_definition(
    server: &mut TestServer,
    id: i32,
    uri: &str,
    text: &str,
    name: &str,
    declaration_line: usize,
) {
    let use_character = line(text, 2)
        .rfind(name)
        .unwrap_or_else(|| panic!("{name} operand should exist"));
    let response = response_value(request::<lsp_types::request::GotoDefinition>(
        server,
        id,
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 2, "character": use_character }
        }),
    ));

    assert_eq!(response["result"]["uri"], uri);
    assert_eq!(
        response["result"]["range"]["start"]["line"],
        declaration_line
    );
    assert_eq!(
        response["result"]["range"]["start"]["character"],
        line(text, declaration_line)
            .find(name)
            .expect("declaration should exist")
    );
}

fn line(text: &str, line: usize) -> &str {
    text.lines().nth(line).expect("line should exist")
}
