use super::{LspServer, notification, notification_value, request, response_value};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

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

#[test]
fn lsp_semantic_tokens_degrade_under_parse_errors() {
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
    let number = token_type_index(token_types, "number");
    let comment = token_type_index(token_types, "comment");

    let text = "\
pub fn main( {
    let value = 1 +
    // keep tokenization alive
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
    let field = token_type_index(token_types, "field");
    let enum_member = token_type_index(token_types, "enumMember");
    let method = token_type_index(token_types, "method");
    let member_modifiers = token_modifier_bit(token_modifiers, "declaration")
        | token_modifier_bit(token_modifiers, "definition");

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
fn lsp_semantic_tokens_classify_host_and_builtin_member_uses() {
    let root = temp_workspace();
    let config_path = root.join("vela.toml");
    let schema_path = root.join("target").join("vela").join("schema.json");
    fs::create_dir_all(schema_path.parent().expect("schema should have parent"))
        .expect("schema directory should be creatable");
    fs::write(
        &config_path,
        r#"
            [workspace]
            roots = ["scripts"]

            [host]
            schema = "target/vela/schema.json"
        "#,
    )
    .expect("vela.toml should be writable");
    fs::write(
        &schema_path,
        r#"{
            "formatVersion": 1,
            "facts": {
                "types": [
                    {
                        "name": "Player",
                        "fact": { "kind": "host", "name": "Player" }
                    }
                ],
                "fields": [
                    {
                        "owner": "Player",
                        "name": "level",
                        "fact": { "kind": "primitive", "name": "i64" }
                    }
                ],
                "methods": [
                    {
                        "owner": "Player",
                        "name": "grant",
                        "fact": {
                            "kind": "function",
                            "params": [{ "kind": "primitive", "name": "i64" }],
                            "returns": { "kind": "primitive", "name": "i64" }
                        }
                    }
                ]
            }
        }"#,
    )
    .expect("schema should be writable");

    let mut server = LspServer::new();
    let initialize = response_value(server.handle_json(&request(
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": file_uri(&root),
            "capabilities": {}
        }),
    )));
    let _ = server.handle_json(&notification(
        "workspace/didChangeWatchedFiles",
        serde_json::json!({
            "changes": [{ "uri": file_uri(&config_path), "type": 1 }]
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
    let property = token_type_index(token_types, "property");
    let method = token_type_index(token_types, "method");
    let host = token_modifier_bit(token_modifiers, "host");
    let builtin = token_modifier_bit(token_modifiers, "defaultLibrary");

    let text = "\
pub fn main(player: Player, names: Array<String>) -> i64 {
    let level = player.level
    player.grant(level)
    return names.len()
}";
    let uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
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
        line(text, 1)
            .rfind("level")
            .expect("host field use should exist"),
        "level".len(),
        property,
        host,
    );
    assert_token_at(
        &tokens,
        2,
        line(text, 2)
            .find("grant")
            .expect("host method use should exist"),
        "grant".len(),
        method,
        host,
    );
    assert_token_at(
        &tokens,
        3,
        line(text, 3)
            .find("len")
            .expect("stdlib method use should exist"),
        "len".len(),
        method,
        builtin,
    );
}

#[test]
fn lsp_semantic_tokens_classify_host_and_builtin_function_calls() {
    let root = temp_workspace();
    let config_path = root.join("vela.toml");
    let schema_path = root.join("target").join("vela").join("schema.json");
    fs::create_dir_all(schema_path.parent().expect("schema should have parent"))
        .expect("schema directory should be creatable");
    fs::write(
        &config_path,
        r#"
            [workspace]
            roots = ["scripts"]

            [host]
            schema = "target/vela/schema.json"
        "#,
    )
    .expect("vela.toml should be writable");
    fs::write(
        &schema_path,
        r#"{
            "formatVersion": 1,
            "facts": {
                "types": [
                    {
                        "name": "Player",
                        "fact": { "kind": "host", "name": "Player" }
                    }
                ],
                "functions": [
                    {
                        "name": "grant_reward",
                        "fact": {
                            "kind": "function",
                            "params": [{ "kind": "host", "name": "Player" }],
                            "returns": { "kind": "primitive", "name": "i64" }
                        }
                    }
                ]
            }
        }"#,
    )
    .expect("schema should be writable");

    let mut server = LspServer::new();
    let initialize = response_value(server.handle_json(&request(
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": file_uri(&root),
            "capabilities": {}
        }),
    )));
    let _ = server.handle_json(&notification(
        "workspace/didChangeWatchedFiles",
        serde_json::json!({
            "changes": [{ "uri": file_uri(&config_path), "type": 1 }]
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
    let host = token_modifier_bit(token_modifiers, "host");
    let builtin = token_modifier_bit(token_modifiers, "defaultLibrary");

    let text = "\
pub fn main(player: Player) -> i64 {
    let reward = grant_reward(player)
    return math::max(reward, 10)
}";
    let uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
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
        line(text, 1)
            .find("grant_reward")
            .expect("schema function call should exist"),
        "grant_reward".len(),
        function,
        host,
    );
    assert_token_at(
        &tokens,
        2,
        line(text, 2)
            .find("max")
            .expect("stdlib function call should exist"),
        "max".len(),
        function,
        builtin,
    );
}

#[test]
fn lsp_semantic_token_delta_matches_full_tokens() {
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
    assert_eq!(
        initialize["result"]["capabilities"]["semanticTokensProvider"]["full"]["delta"],
        true
    );

    let uri = "file:///workspace/scripts/game/main.vela";
    let original = "pub fn main() { let value = 1 return value }";
    let changed = "pub fn main() { let value = 20 return value }";
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": original
            }
        }),
    )));

    let full = response_value(server.handle_json(&request(
        2,
        "textDocument/semanticTokens/full",
        serde_json::json!({
            "textDocument": { "uri": uri }
        }),
    )));
    let previous_result_id = full["result"]["resultId"]
        .as_str()
        .expect("full semantic tokens should include resultId")
        .to_owned();
    let previous_len = full["result"]["data"]
        .as_array()
        .expect("full semantic tokens should include data")
        .len();

    let unchanged = response_value(server.handle_json(&request(
        3,
        "textDocument/semanticTokens/full/delta",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "previousResultId": previous_result_id.clone()
        }),
    )));
    assert_eq!(unchanged["result"]["edits"], serde_json::json!([]));

    let _ = notification_value(server.handle_json(&notification(
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
    )));
    let changed_full = response_value(server.handle_json(&request(
        4,
        "textDocument/semanticTokens/full",
        serde_json::json!({
            "textDocument": { "uri": uri }
        }),
    )));
    let changed_full_data = changed_full["result"]["data"]
        .as_array()
        .expect("changed full semantic tokens should include data");

    let delta = response_value(server.handle_json(&request(
        5,
        "textDocument/semanticTokens/full/delta",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "previousResultId": previous_result_id
        }),
    )));
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

fn temp_workspace() -> PathBuf {
    let mut path = std::env::temp_dir();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after epoch")
        .as_nanos();
    path.push(format!("vela-lsp-semantic-token-test-{unique}"));
    fs::create_dir_all(&path).expect("temporary workspace should be creatable");
    path
}

fn file_uri(path: &Path) -> String {
    format!("file://{}", path.display())
}
