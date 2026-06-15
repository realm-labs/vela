use super::{LspServer, notification, notification_value, request, response_value};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct DecodedToken {
    line: u64,
    character: u64,
    length: u64,
    token_type: u64,
    modifiers: u64,
}

#[test]
fn lsp_semantic_tokens_cover_lexical_classes() {
    let mut server = LspServer::new();
    let initialize = response_value(server.handle_json(&request(
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": "file:///workspace/scripts",
            "capabilities": {}
        }),
    )));
    let token_types =
        initialize["result"]["capabilities"]["semanticTokensProvider"]["legend"]["tokenTypes"]
            .as_array()
            .expect("semantic token legend should list token types");
    let keyword = token_type_index(token_types, "keyword");
    let variable = token_type_index(token_types, "variable");
    let bytes = token_type_index(token_types, "bytes");
    let operator = token_type_index(token_types, "operator");
    let number = token_type_index(token_types, "number");

    let text = "pub fn main() { let bytes = b\"ok\" return bytes + 1 }";
    let uri = "file:///workspace/scripts/game/main.vela";
    let _ = notification_value(server.handle_json(&notification(
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

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/semanticTokens/full",
        serde_json::json!({
            "textDocument": { "uri": uri }
        }),
    )));
    let tokens = decode_tokens(
        response["result"]["data"]
            .as_array()
            .expect("semantic token response should include data"),
    );

    assert_token(&tokens, text, "pub", keyword);
    assert_token(&tokens, text, "main", variable);
    assert_token(&tokens, text, "b\"ok\"", bytes);
    assert_token(&tokens, text, "+", operator);
    assert_token(&tokens, text, "1", number);
}

fn token_type_index(token_types: &[serde_json::Value], name: &str) -> u64 {
    token_types
        .iter()
        .position(|token_type| token_type == name)
        .and_then(|index| u64::try_from(index).ok())
        .unwrap_or_else(|| panic!("semantic token legend should include {name}"))
}

fn decode_tokens(data: &[serde_json::Value]) -> Vec<DecodedToken> {
    assert_eq!(data.len() % 5, 0, "semantic token data is encoded in fives");
    let mut tokens = Vec::new();
    let mut line = 0_u64;
    let mut character = 0_u64;
    for chunk in data.chunks(5) {
        let delta_line = number(&chunk[0]);
        let delta_start = number(&chunk[1]);
        line += delta_line;
        if delta_line == 0 {
            character += delta_start;
        } else {
            character = delta_start;
        }
        tokens.push(DecodedToken {
            line,
            character,
            length: number(&chunk[2]),
            token_type: number(&chunk[3]),
            modifiers: number(&chunk[4]),
        });
    }
    tokens
}

fn number(value: &serde_json::Value) -> u64 {
    value
        .as_u64()
        .expect("semantic token data should be numeric")
}

fn assert_token(tokens: &[DecodedToken], text: &str, needle: &str, token_type: u64) {
    let start = text.find(needle).expect("token text should exist");
    assert!(
        tokens.iter().any(|token| token.line == 0
            && token.character == start as u64
            && token.length == needle.len() as u64
            && token.token_type == token_type
            && token.modifiers == 0),
        "{tokens:?}"
    );
}
