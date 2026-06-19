use super::{LspServer, notification, notification_value, request, response_value};

#[test]
fn lsp_semantic_tokens_classify_source_method_on_source_function_return() {
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
    let method = token_type_index(token_types, "method");
    let source = token_modifier_bit(token_modifiers, "source");

    let uri = "file:///workspace/scripts/game/main.vela";
    let text = "\
struct Player { level: i64 }
impl Player {
    fn grant(self, amount: i64) -> i64 { return amount }
}
fn current_player() -> Player { return Player { level: 1 } }
pub fn main() -> i64 {
    return current_player().grant(1)
}";
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
        6,
        line(text, 6)
            .find("current_player")
            .expect("source function call should exist"),
        "current_player".len(),
        function,
        source,
    );
    assert_token_at(
        &tokens,
        6,
        line(text, 6)
            .find("grant")
            .expect("source method call should exist"),
        "grant".len(),
        method,
        source,
    );
}

#[test]
fn lsp_semantic_tokens_classify_imported_source_method_on_source_function_return() {
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
    let method = token_type_index(token_types, "method");
    let source = token_modifier_bit(token_modifiers, "source");

    let player_uri = "file:///workspace/scripts/game/player.vela";
    let player_text = "\
pub struct Player { level: i64 }
impl Player {
    fn grant(self, amount: i64) -> i64 { return amount }
}
pub fn current_player() -> Player { return Player { level: 1 } }";
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": player_uri,
                "languageId": "vela",
                "version": 1,
                "text": player_text
            }
        }),
    )));
    let uri = "file:///workspace/scripts/game/main.vela";
    let text = "\
use game::player::current_player
pub fn main() -> i64 {
    return current_player().grant(1)
}";
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
        2,
        line(text, 2)
            .find("current_player")
            .expect("imported source function call should exist"),
        "current_player".len(),
        function,
        source,
    );
    assert_token_at(
        &tokens,
        2,
        line(text, 2)
            .find("grant")
            .expect("imported source method call should exist"),
        "grant".len(),
        method,
        source,
    );
}

#[test]
fn lsp_semantic_tokens_classify_source_method_on_source_method_return() {
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
    let method = token_type_index(token_types, "method");
    let source = token_modifier_bit(token_modifiers, "source");

    let uri = "file:///workspace/scripts/game/main.vela";
    let text = "\
struct Player { level: i64 }
struct Inventory { count: i64 }
impl Player {
    fn inventory(self) -> Inventory { return Inventory { count: 1 } }
}
impl Inventory {
    fn grant(self, amount: i64) -> i64 { return amount }
}
pub fn main(player: Player) -> i64 {
    return player.inventory().grant(1)
}";
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
        9,
        line(text, 9)
            .find("inventory")
            .expect("source method call should exist"),
        "inventory".len(),
        method,
        source,
    );
    assert_token_at(
        &tokens,
        9,
        line(text, 9)
            .find("grant")
            .expect("chained source method call should exist"),
        "grant".len(),
        method,
        source,
    );
}

#[test]
fn lsp_semantic_tokens_classify_source_trait_method_on_source_function_return() {
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
    let method = token_type_index(token_types, "method");
    let source = token_modifier_bit(token_modifiers, "source");

    let uri = "file:///workspace/scripts/game/main.vela";
    let text = "\
trait Rewardable {
    fn preview(self, amount: i64) -> i64 { return amount }
}
struct Player { level: i64 }
impl Rewardable for Player {}
fn current_player() -> Player { return Player { level: 1 } }
pub fn main() -> i64 {
    return current_player().preview(1)
}";
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
        7,
        line(text, 7)
            .find("current_player")
            .expect("source function call should exist"),
        "current_player".len(),
        function,
        source,
    );
    assert_token_at(
        &tokens,
        7,
        line(text, 7)
            .find("preview")
            .expect("source trait method call should exist"),
        "preview".len(),
        method,
        source,
    );
}

#[test]
fn lsp_semantic_tokens_classify_source_trait_method_on_source_method_return() {
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
    let method = token_type_index(token_types, "method");
    let source = token_modifier_bit(token_modifiers, "source");

    let uri = "file:///workspace/scripts/game/main.vela";
    let text = "\
trait Rewardable {
    fn preview(self, amount: i64) -> i64 { return amount }
}
struct Player { level: i64 }
struct Inventory { count: i64 }
impl Player {
    fn inventory(self) -> Inventory { return Inventory { count: 1 } }
}
impl Rewardable for Inventory {}
pub fn main(player: Player) -> i64 {
    return player.inventory().preview(1)
}";
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
        10,
        line(text, 10)
            .find("inventory")
            .expect("source method call should exist"),
        "inventory".len(),
        method,
        source,
    );
    assert_token_at(
        &tokens,
        10,
        line(text, 10)
            .find("preview")
            .expect("source trait method call should exist"),
        "preview".len(),
        method,
        source,
    );
}

#[derive(Debug)]
struct DecodedToken {
    line: u64,
    character: u64,
    length: u64,
    token_type: u64,
    modifiers: u64,
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
