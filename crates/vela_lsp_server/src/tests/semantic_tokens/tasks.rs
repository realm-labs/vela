use super::*;

#[test]
fn lsp_semantic_tokens_classify_static_detached_task_operands() {
    let mut server = TestServer::new();
    let initialize = response_value(request::<lsp_types::request::Initialize>(
        &mut server,
        1,
        serde_json::json!({
            "processId": null,
            "rootUri": "file:///workspace/scripts",
            "capabilities": {}
        }),
    ));
    let token_types =
        initialize["result"]["capabilities"]["semanticTokensProvider"]["legend"]["tokenTypes"]
            .as_array()
            .expect("semantic token legend should list token types");
    let token_modifiers = initialize["result"]["capabilities"]["semanticTokensProvider"]["legend"]
        ["tokenModifiers"]
        .as_array()
        .expect("semantic token legend should list token modifiers");
    let function = token_type_index(token_types, "function");
    let source = token_modifier_bit(token_modifiers, "source");
    let builtin = token_modifier_bit(token_modifiers, "defaultLibrary");

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
    let response = response_value(request::<lsp_types::request::SemanticTokensFullRequest>(
        &mut server,
        2,
        serde_json::json!({ "textDocument": { "uri": uri } }),
    ));
    let tokens = decode_tokens(
        response["result"]["data"]
            .as_array()
            .expect("semantic token response should include data"),
    );

    assert_token_at(
        &tokens,
        2,
        line(text, 2)
            .find("spawn_scoped_then")
            .expect("task builtin"),
        "spawn_scoped_then".len(),
        function,
        builtin,
    );
    assert_token_at(
        &tokens,
        2,
        line(text, 2).find("worker").expect("worker operand"),
        "worker".len(),
        function,
        source,
    );
    assert_token_at(
        &tokens,
        2,
        line(text, 2)
            .rfind("continuation")
            .expect("continuation operand"),
        "continuation".len(),
        function,
        source,
    );
}
