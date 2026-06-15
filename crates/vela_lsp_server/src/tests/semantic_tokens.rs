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
    assert_token(&tokens, text, "b\"ok\"", bytes);
    assert_token(&tokens, text, "+", operator);
    assert_token(&tokens, text, "1", number);
}

#[test]
fn lsp_semantic_tokens_mark_resolved_symbols() {
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
    let token_modifiers = initialize["result"]["capabilities"]["semanticTokensProvider"]["legend"]
        ["tokenModifiers"]
        .as_array()
        .expect("semantic token legend should list token modifiers");
    let function = token_type_index(token_types, "function");
    let parameter = token_type_index(token_types, "parameter");
    let variable = token_type_index(token_types, "variable");
    let declaration = token_modifier_bit(token_modifiers, "declaration");
    let definition = token_modifier_bit(token_modifiers, "definition");

    let text = "\
use game::reward::grant
pub fn main(amount: i64) -> i64 {
    let next = grant(amount)
    return next
}";
    let uri = "file:///workspace/scripts/game/main.vela";
    let helper_uri = "file:///workspace/scripts/game/reward.vela";
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": helper_uri,
                "languageId": "vela",
                "version": 1,
                "text": "pub fn grant(amount: i64) -> i64 { return amount }"
            }
        }),
    )));
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

    assert_token_at(
        &tokens,
        1,
        line(text, 1).find("main").expect("main should exist"),
        "main".len(),
        function,
        declaration | definition,
    );
    assert_token_at(
        &tokens,
        1,
        line(text, 1)
            .find("amount")
            .expect("parameter should exist"),
        "amount".len(),
        parameter,
        declaration,
    );
    assert_token_at(
        &tokens,
        2,
        line(text, 2).find("next").expect("local should exist"),
        "next".len(),
        variable,
        declaration,
    );
    assert_token_at(
        &tokens,
        2,
        line(text, 2).find("grant").expect("call should exist"),
        "grant".len(),
        function,
        0,
    );
    assert_token_at(
        &tokens,
        3,
        line(text, 3)
            .find("next")
            .expect("return value should exist"),
        "next".len(),
        variable,
        0,
    );
}

#[test]
fn lsp_semantic_tokens_include_comments() {
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
    let comment = token_type_index(token_types, "comment");

    let text = "\
// setup
pub fn main() {
    let text = \"not // a comment\"
    /* block
       done */
    return text
}";
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

    assert_token_at(&tokens, 0, 0, line(text, 0).len(), comment, 0);
    assert_token_at(
        &tokens,
        3,
        line(text, 3)
            .find("/* block")
            .expect("block comment should exist"),
        "/* block".len(),
        comment,
        0,
    );
    assert_token_at(&tokens, 4, 0, line(text, 4).len(), comment, 0);
    assert!(
        tokens.iter().all(|token| {
            token.line != 2
                || token.token_type != comment
                || token.character
                    != line(text, 2)
                        .find("//")
                        .expect("string should contain comment marker")
                        as u64
        }),
        "{tokens:?}"
    );
}

fn token_type_index(token_types: &[serde_json::Value], name: &str) -> u64 {
    token_types
        .iter()
        .position(|token_type| token_type == name)
        .and_then(|index| u64::try_from(index).ok())
        .unwrap_or_else(|| panic!("semantic token legend should include {name}"))
}

fn token_modifier_bit(token_modifiers: &[serde_json::Value], name: &str) -> u64 {
    token_modifiers
        .iter()
        .position(|token_modifier| token_modifier == name)
        .and_then(|index| u32::try_from(index).ok())
        .map(|index| 1_u64 << index)
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
    assert_token_at(tokens, 0, start, needle.len(), token_type, 0);
}

fn assert_token_at(
    tokens: &[DecodedToken],
    line: usize,
    character: usize,
    length: usize,
    token_type: u64,
    modifiers: u64,
) {
    assert!(
        tokens.iter().any(|token| token.line == line as u64
            && token.character == character as u64
            && token.length == length as u64
            && token.token_type == token_type
            && token.modifiers == modifiers),
        "{tokens:?}"
    );
}

fn line(text: &str, line: usize) -> &str {
    text.lines().nth(line).expect("line should exist")
}
