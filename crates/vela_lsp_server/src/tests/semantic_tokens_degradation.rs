use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::tests::{
    LspServer, handle_notification, handle_request, notification_value, notification_values,
    response_value,
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
fn lsp_semantic_tokens_degrade_schema_type_hints_when_schema_is_missing() {
    let root = temp_workspace();
    let config_path = root.join("vela.toml");
    let schema_path = root.join("target").join("vela").join("schema.json");
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

    let mut server = LspServer::new();
    let initialize = response_value(handle_request(
        &mut server,
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": file_uri(&root),
            "capabilities": {}
        }),
    ));
    let notifications = notification_values(handle_notification(
        &mut server,
        "workspace/didChangeWatchedFiles",
        serde_json::json!({
            "changes": [{ "uri": file_uri(&config_path), "type": 1 }]
        }),
    ));
    assert!(
        notifications.iter().any(|notification| {
            notification["method"] == "textDocument/publishDiagnostics"
                && notification["params"]["uri"] == file_uri(&schema_path)
                && notification["params"]["diagnostics"]
                    .as_array()
                    .is_some_and(|diagnostics| {
                        diagnostics.iter().any(|diagnostic| {
                            diagnostic["code"] == "schema::diagnostic"
                                && diagnostic["message"]
                                    .as_str()
                                    .is_some_and(|message| message.contains("host schema"))
                        })
                    })
        }),
        "{notifications:?}"
    );

    let token_types =
        initialize["result"]["capabilities"]["semanticTokensProvider"]["legend"]["tokenTypes"]
            .as_array()
            .expect("semantic token legend should list token types");
    let token_modifiers = initialize["result"]["capabilities"]["semanticTokensProvider"]["legend"]
        ["tokenModifiers"]
        .as_array()
        .expect("semantic token legend should list token modifiers");
    let type_token = token_type_index(token_types, "type");
    let builtin_type = token_type_index(token_types, "builtinType");
    let host = token_modifier_bit(token_modifiers, "host");
    let schema = token_modifier_bit(token_modifiers, "schema");
    let default_library = token_modifier_bit(token_modifiers, "defaultLibrary");

    let text = "\
pub fn main(player: Player, names: Array<String>) -> i64 {
    let level = 1
    return level
}";
    let uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
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
        line(text, 0).find("Player").expect("schema type hint"),
        "Player".len(),
        type_token,
        0,
    );
    assert_token_at(
        &tokens,
        0,
        line(text, 0).find("Array").expect("builtin array hint"),
        "Array".len(),
        builtin_type,
        default_library,
    );
    assert!(
        tokens.iter().all(|token| {
            token.line != 0
                || token.character != line(text, 0).find("Player").expect("Player") as u64
                || token.modifiers & (host | schema) == 0
        }),
        "{tokens:?}"
    );

    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
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

fn temp_workspace() -> PathBuf {
    let suffix = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(error) => panic!("system time should be after UNIX_EPOCH: {error}"),
    };
    let root = std::env::temp_dir().join(format!(
        "vela_lsp_semantic_tokens_missing_schema_{}_{}",
        std::process::id(),
        suffix
    ));
    fs::create_dir_all(root.join("scripts").join("game"))
        .expect("temporary workspace should be creatable");
    root
}

fn file_uri(path: &Path) -> String {
    let path = path.display().to_string().replace('\\', "/");
    if path.starts_with('/') {
        format!("file://{path}")
    } else {
        format!("file:///{path}")
    }
}
