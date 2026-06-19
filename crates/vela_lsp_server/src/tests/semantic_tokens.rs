use super::{LspServer, handle_notification, handle_request, notification_value, response_value};

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
    let initialize = response_value(handle_request(
        &mut server,
        1,
        "initialize",
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
    let keyword = token_type_index(token_types, "keyword");
    let bytes = token_type_index(token_types, "bytes");
    let operator = token_type_index(token_types, "arithmeticOperator");
    let number = token_type_index(token_types, "number");

    let text = "pub fn main() { let bytes = b\"ok\" return bytes + 1 }";
    let uri = "file:///workspace/scripts/game/main.vela";
    let _ = notification_value(handle_notification(
        &mut server,
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    ));

    let response = response_value(handle_request(
        &mut server,
        2,
        "textDocument/semanticTokens/full",
        serde_json::json!({
            "textDocument": { "uri": uri }
        }),
    ));
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
    let initialize = response_value(handle_request(
        &mut server,
        1,
        "initialize",
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
    let parameter = token_type_index(token_types, "parameter");
    let variable = token_type_index(token_types, "variable");
    let declaration = token_modifier_bit(token_modifiers, "declaration");
    let definition = token_modifier_bit(token_modifiers, "definition");
    let source = token_modifier_bit(token_modifiers, "source");

    let text = "\
use game::reward::grant
pub fn main(amount: i64) -> i64 {
    let next = grant(amount)
    return next
}";
    let uri = "file:///workspace/scripts/game/main.vela";
    let helper_uri = "file:///workspace/scripts/game/reward.vela";
    let _ = notification_value(handle_notification(
        &mut server,
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": helper_uri,
                "languageId": "vela",
                "version": 1,
                "text": "pub fn grant(amount: i64) -> i64 { return amount }"
            }
        }),
    ));
    let _ = notification_value(handle_notification(
        &mut server,
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    ));

    let response = response_value(handle_request(
        &mut server,
        2,
        "textDocument/semanticTokens/full",
        serde_json::json!({
            "textDocument": { "uri": uri }
        }),
    ));
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
        declaration | definition | source,
    );
    assert_token_at(
        &tokens,
        1,
        line(text, 1)
            .find("amount")
            .expect("parameter should exist"),
        "amount".len(),
        parameter,
        declaration | source,
    );
    assert_token_at(
        &tokens,
        2,
        line(text, 2).find("next").expect("local should exist"),
        "next".len(),
        variable,
        declaration | source,
    );
    assert_token_at(
        &tokens,
        2,
        line(text, 2).find("grant").expect("call should exist"),
        "grant".len(),
        function,
        source,
    );
    assert_token_at(
        &tokens,
        3,
        line(text, 3)
            .find("next")
            .expect("return value should exist"),
        "next".len(),
        variable,
        source,
    );
}

#[test]
fn lsp_semantic_tokens_classify_import_module_path_segments() {
    let mut server = LspServer::new();
    let initialize = response_value(handle_request(
        &mut server,
        1,
        "initialize",
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
    let namespace = token_type_index(token_types, "namespace");
    let function = token_type_index(token_types, "function");
    let token_modifiers = initialize["result"]["capabilities"]["semanticTokensProvider"]["legend"]
        ["tokenModifiers"]
        .as_array()
        .expect("semantic token legend should list token modifiers");
    let source = token_modifier_bit(token_modifiers, "source");

    let text = "\
use game::reward::grant
pub fn main() -> i64 {
    return grant()
}";
    let uri = "file:///workspace/scripts/game/main.vela";
    let helper_uri = "file:///workspace/scripts/game/reward.vela";
    let _ = notification_value(handle_notification(
        &mut server,
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": helper_uri,
                "languageId": "vela",
                "version": 1,
                "text": "pub fn grant() -> i64 { return 1 }"
            }
        }),
    ));
    let _ = notification_value(handle_notification(
        &mut server,
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    ));

    let response = response_value(handle_request(
        &mut server,
        2,
        "textDocument/semanticTokens/full",
        serde_json::json!({
            "textDocument": { "uri": uri }
        }),
    ));
    let tokens = decode_tokens(
        response["result"]["data"]
            .as_array()
            .expect("semantic token response should include data"),
    );

    assert_token_at(
        &tokens,
        0,
        line(text, 0).find("game").expect("module root"),
        "game".len(),
        namespace,
        0,
    );
    assert_token_at(
        &tokens,
        0,
        line(text, 0).find("reward").expect("module leaf"),
        "reward".len(),
        namespace,
        0,
    );
    assert_token_at(
        &tokens,
        0,
        line(text, 0).find("grant").expect("imported declaration"),
        "grant".len(),
        function,
        source,
    );
}

#[test]
fn lsp_semantic_tokens_include_comments() {
    let mut server = LspServer::new();
    let initialize = response_value(handle_request(
        &mut server,
        1,
        "initialize",
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
    let _ = notification_value(handle_notification(
        &mut server,
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    ));

    let response = response_value(handle_request(
        &mut server,
        2,
        "textDocument/semanticTokens/full",
        serde_json::json!({
            "textDocument": { "uri": uri }
        }),
    ));
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

#[test]
fn lsp_semantic_tokens_degrade_under_parse_errors() {
    let mut server = LspServer::new();
    let initialize = response_value(handle_request(
        &mut server,
        1,
        "initialize",
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
    let keyword = token_type_index(token_types, "keyword");
    let number = token_type_index(token_types, "number");
    let comment = token_type_index(token_types, "comment");

    let text = "\
pub fn main( {
    let value = 1 +
    // keep tokenization alive
}";
    let uri = "file:///workspace/scripts/game/main.vela";
    let _ = notification_value(handle_notification(
        &mut server,
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    ));

    let response = response_value(handle_request(
        &mut server,
        2,
        "textDocument/semanticTokens/full",
        serde_json::json!({
            "textDocument": { "uri": uri }
        }),
    ));
    let tokens = decode_tokens(
        response["result"]["data"]
            .as_array()
            .expect("semantic token response should include data"),
    );

    assert_token_at(
        &tokens,
        0,
        line(text, 0).find("pub").expect("keyword should exist"),
        "pub".len(),
        keyword,
        0,
    );
    assert_token_at(
        &tokens,
        1,
        line(text, 1).find("let").expect("keyword should exist"),
        "let".len(),
        keyword,
        0,
    );
    assert_token_at(
        &tokens,
        1,
        line(text, 1).find('1').expect("number should exist"),
        1,
        number,
        0,
    );
    assert_token_at(
        &tokens,
        2,
        line(text, 2).find("// keep").expect("comment should exist"),
        line(text, 2).trim_start().len(),
        comment,
        0,
    );
}

#[test]
fn lsp_semantic_tokens_classify_script_members() {
    let mut server = LspServer::new();
    let initialize = response_value(handle_request(
        &mut server,
        1,
        "initialize",
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
    let field = token_type_index(token_types, "field");
    let enum_member = token_type_index(token_types, "enumMember");
    let method = token_type_index(token_types, "method");
    let source = token_modifier_bit(token_modifiers, "source");
    let member_modifiers = token_modifier_bit(token_modifiers, "declaration")
        | token_modifier_bit(token_modifiers, "definition")
        | source;

    let text = "\
pub struct Reward {
    amount: i64
}

pub enum Progress {
    Started
    Active { quest_id: String }
    Finished(result: String)
}

pub trait Scored {
    fn score(value: Reward) -> i64
}

impl Reward {
    fn bonus(value: Reward) -> i64 { return 1 }
}";
    let uri = "file:///workspace/scripts/game/main.vela";
    let _ = notification_value(handle_notification(
        &mut server,
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    ));

    let response = response_value(handle_request(
        &mut server,
        2,
        "textDocument/semanticTokens/full",
        serde_json::json!({
            "textDocument": { "uri": uri }
        }),
    ));
    let tokens = decode_tokens(
        response["result"]["data"]
            .as_array()
            .expect("semantic token response should include data"),
    );

    assert_token_at(
        &tokens,
        1,
        line(text, 1).find("amount").expect("field should exist"),
        "amount".len(),
        field,
        member_modifiers,
    );
    assert_token_at(
        &tokens,
        5,
        line(text, 5).find("Started").expect("variant should exist"),
        "Started".len(),
        enum_member,
        member_modifiers,
    );
    assert_token_at(
        &tokens,
        6,
        line(text, 6)
            .find("quest_id")
            .expect("record variant field should exist"),
        "quest_id".len(),
        field,
        member_modifiers,
    );
    assert_token_at(
        &tokens,
        7,
        line(text, 7)
            .find("result")
            .expect("tuple variant field should exist"),
        "result".len(),
        field,
        member_modifiers,
    );
    assert_token_at(
        &tokens,
        11,
        line(text, 11)
            .find("score")
            .expect("trait method should exist"),
        "score".len(),
        method,
        member_modifiers,
    );
    assert_token_at(
        &tokens,
        15,
        line(text, 15)
            .find("bonus")
            .expect("impl method should exist"),
        "bonus".len(),
        method,
        member_modifiers,
    );
}

#[test]
fn lsp_semantic_tokens_classify_script_member_uses() {
    let mut server = LspServer::new();
    let initialize = response_value(handle_request(
        &mut server,
        1,
        "initialize",
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
    let property = token_type_index(token_types, "property");
    let method = token_type_index(token_types, "method");
    let token_modifiers = initialize["result"]["capabilities"]["semanticTokensProvider"]["legend"]
        ["tokenModifiers"]
        .as_array()
        .expect("semantic token legend should list token modifiers");
    let source = token_modifier_bit(token_modifiers, "source");

    let text = "\
pub struct Reward {
    amount: i64
}

impl Reward {
    fn bonus(value: Reward) -> i64 { return value.amount }
}

pub fn main(reward: Reward) -> i64 {
    return reward.amount + reward.bonus()
}";
    let uri = "file:///workspace/scripts/game/main.vela";
    let _ = notification_value(handle_notification(
        &mut server,
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    ));

    let response = response_value(handle_request(
        &mut server,
        2,
        "textDocument/semanticTokens/full",
        serde_json::json!({
            "textDocument": { "uri": uri }
        }),
    ));
    let tokens = decode_tokens(
        response["result"]["data"]
            .as_array()
            .expect("semantic token response should include data"),
    );

    assert_token_at(
        &tokens,
        9,
        line(text, 9)
            .find("amount")
            .expect("field use should exist"),
        "amount".len(),
        property,
        source,
    );
    assert_token_at(
        &tokens,
        9,
        line(text, 9)
            .find("bonus")
            .expect("method use should exist"),
        "bonus".len(),
        method,
        source,
    );
}

#[test]
fn lsp_semantic_tokens_classify_script_trait_method_uses() {
    let mut server = LspServer::new();
    let initialize = response_value(handle_request(
        &mut server,
        1,
        "initialize",
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
    let method = token_type_index(token_types, "method");
    let token_modifiers = initialize["result"]["capabilities"]["semanticTokensProvider"]["legend"]
        ["tokenModifiers"]
        .as_array()
        .expect("semantic token legend should list token modifiers");
    let source = token_modifier_bit(token_modifiers, "source");

    let text = "\
pub trait Rewardable {
    fn preview(self, amount: i64) -> i64
}

pub fn main(rewardable: Rewardable) -> i64 {
    return rewardable.preview(1)
}";
    let uri = "file:///workspace/scripts/game/main.vela";
    let _ = notification_value(handle_notification(
        &mut server,
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    ));

    let response = response_value(handle_request(
        &mut server,
        2,
        "textDocument/semanticTokens/full",
        serde_json::json!({
            "textDocument": { "uri": uri }
        }),
    ));
    let tokens = decode_tokens(
        response["result"]["data"]
            .as_array()
            .expect("semantic token response should include data"),
    );

    assert_token_at(
        &tokens,
        5,
        line(text, 5)
            .find("preview")
            .expect("trait method call should exist"),
        "preview".len(),
        method,
        source,
    );
}

#[test]
fn lsp_semantic_tokens_range_filters_tokens() {
    let mut server = LspServer::new();
    let initialize = response_value(handle_request(
        &mut server,
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": "file:///workspace/scripts",
            "capabilities": {}
        }),
    ));
    assert_eq!(
        initialize["result"]["capabilities"]["semanticTokensProvider"]["range"],
        true
    );
    let token_types =
        initialize["result"]["capabilities"]["semanticTokensProvider"]["legend"]["tokenTypes"]
            .as_array()
            .expect("semantic token legend should list token types");
    let token_modifiers = initialize["result"]["capabilities"]["semanticTokensProvider"]["legend"]
        ["tokenModifiers"]
        .as_array()
        .expect("semantic token legend should list token modifiers");
    let variable = token_type_index(token_types, "variable");
    let declaration = token_modifier_bit(token_modifiers, "declaration");
    let source = token_modifier_bit(token_modifiers, "source");

    let text = "\
pub fn main() {
    let first = 1
    let second = first + 2
    return second
}";
    let uri = "file:///workspace/scripts/game/main.vela";
    let _ = notification_value(handle_notification(
        &mut server,
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    ));

    let response = response_value(handle_request(
        &mut server,
        2,
        "textDocument/semanticTokens/range",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 2, "character": 0 },
                "end": { "line": 3, "character": 0 }
            }
        }),
    ));
    let tokens = decode_tokens(
        response["result"]["data"]
            .as_array()
            .expect("semantic token range response should include data"),
    );

    assert!(!tokens.is_empty(), "range should include line 2 tokens");
    assert!(
        tokens.iter().all(|token| token.line == 2),
        "range tokens should stay inside requested line: {tokens:?}"
    );
    assert_token_at(
        &tokens,
        2,
        line(text, 2).find("second").expect("local declaration"),
        "second".len(),
        variable,
        declaration | source,
    );
}

#[test]
fn lsp_semantic_tokens_range_returns_empty_for_empty_prefix_range() {
    let mut server = LspServer::new();
    let _ = response_value(handle_request(
        &mut server,
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": "file:///workspace/scripts",
            "capabilities": {}
        }),
    ));
    let text = "\
pub fn main() {
    let value = 1
    return value
}";
    let uri = "file:///workspace/scripts/game/main.vela";
    let _ = notification_value(handle_notification(
        &mut server,
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    ));

    let response = response_value(handle_request(
        &mut server,
        2,
        "textDocument/semanticTokens/range",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 1, "character": 0 },
                "end": { "line": 1, "character": 0 }
            }
        }),
    ));

    assert_eq!(
        response["result"]["data"],
        serde_json::json!([]),
        "{response:?}"
    );
}

#[test]
fn lsp_semantic_tokens_project_custom_tokens_to_client_fallbacks() {
    let mut server = LspServer::new();
    let initialize = response_value(handle_request(
        &mut server,
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": "file:///workspace/scripts",
            "capabilities": {
                "textDocument": {
                    "semanticTokens": {
                        "tokenTypes": [
                            "namespace",
                            "function",
                            "method",
                            "property",
                            "variable",
                            "parameter",
                            "type",
                            "enumMember",
                            "keyword",
                            "number",
                            "string",
                            "comment",
                            "operator",
                            "decorator",
                            "macro",
                            "struct",
                            "enum",
                            "interface"
                        ],
                        "tokenModifiers": [
                            "declaration",
                            "definition",
                            "readonly",
                            "deprecated",
                            "defaultLibrary",
                            "modification",
                            "static",
                            "documentation"
                        ]
                    }
                }
            }
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
    assert!(
        !token_types
            .iter()
            .any(|token_type| token_type == "builtinType")
    );
    assert!(
        !token_types
            .iter()
            .any(|token_type| token_type == "arithmeticOperator")
    );
    assert!(
        !token_modifiers
            .iter()
            .any(|token_modifier| token_modifier == "source")
    );
    let keyword = token_type_index(token_types, "keyword");
    let variable = token_type_index(token_types, "variable");
    let type_token = token_type_index(token_types, "type");
    let operator = token_type_index(token_types, "operator");
    let declaration = token_modifier_bit(token_modifiers, "declaration");
    let default_library = token_modifier_bit(token_modifiers, "defaultLibrary");

    let text = "pub fn main(flag: bool) { let value = flag == true return value + 1 }";
    let uri = "file:///workspace/scripts/game/main.vela";
    let _ = notification_value(handle_notification(
        &mut server,
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    ));

    let response = response_value(handle_request(
        &mut server,
        2,
        "textDocument/semanticTokens/full",
        serde_json::json!({
            "textDocument": { "uri": uri }
        }),
    ));
    let tokens = decode_tokens(
        response["result"]["data"]
            .as_array()
            .expect("semantic token response should include data"),
    );

    assert_token_at(
        &tokens,
        0,
        text.find("bool").expect("builtin type should exist"),
        "bool".len(),
        type_token,
        default_library,
    );
    assert_token(&tokens, text, "true", keyword);
    assert_token(&tokens, text, "==", operator);
    assert_token(&tokens, text, "+", operator);
    assert_token_at(
        &tokens,
        0,
        text.find("value").expect("local declaration should exist"),
        "value".len(),
        variable,
        declaration,
    );
}

#[test]
fn lsp_semantic_token_delta_matches_full_tokens() {
    let mut server = LspServer::new();
    let initialize = response_value(handle_request(
        &mut server,
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": "file:///workspace/scripts",
            "capabilities": {}
        }),
    ));
    assert_eq!(
        initialize["result"]["capabilities"]["semanticTokensProvider"]["full"]["delta"],
        true
    );

    let uri = "file:///workspace/scripts/game/main.vela";
    let original = "pub fn main() { let value = 1 return value }";
    let changed = "pub fn main() { let value = 20 return value }";
    let _ = notification_value(handle_notification(
        &mut server,
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": original
            }
        }),
    ));

    let full = response_value(handle_request(
        &mut server,
        2,
        "textDocument/semanticTokens/full",
        serde_json::json!({
            "textDocument": { "uri": uri }
        }),
    ));
    let previous_result_id = full["result"]["resultId"]
        .as_str()
        .expect("full semantic tokens should include resultId")
        .to_owned();
    let previous_len = full["result"]["data"]
        .as_array()
        .expect("full semantic tokens should include data")
        .len();

    let unchanged = response_value(handle_request(
        &mut server,
        3,
        "textDocument/semanticTokens/full/delta",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "previousResultId": previous_result_id.clone()
        }),
    ));
    assert_eq!(unchanged["result"]["edits"], serde_json::json!([]));

    let _ = notification_value(handle_notification(
        &mut server,
        "textDocument/didChange",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "version": 2
            },
            "contentChanges": [
                { "text": changed }
            ]
        }),
    ));
    let changed_full = response_value(handle_request(
        &mut server,
        4,
        "textDocument/semanticTokens/full",
        serde_json::json!({
            "textDocument": { "uri": uri }
        }),
    ));
    let changed_full_data = changed_full["result"]["data"]
        .as_array()
        .expect("changed full semantic tokens should include data");

    let delta = response_value(handle_request(
        &mut server,
        5,
        "textDocument/semanticTokens/full/delta",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "previousResultId": previous_result_id
        }),
    ));
    let edits = delta["result"]["edits"]
        .as_array()
        .expect("delta semantic tokens should include edits");
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0]["start"], 0);
    assert_eq!(edits[0]["deleteCount"], previous_len);
    assert_eq!(
        edits[0]["data"]
            .as_array()
            .expect("delta replacement should include data"),
        changed_full_data
    );
    assert_eq!(
        delta["result"]["resultId"],
        changed_full["result"]["resultId"]
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
