use super::{LspServer, notification, notification_value, request, response_value};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
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

const HIGHLIGHTING_SHOWCASE: &str =
    include_str!("../../../../tests/fixtures/lsp_highlighting/showcase.vela");

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
                "traits": [
                    {
                        "name": "Rewardable",
                        "fact": { "kind": "trait", "name": "Rewardable" }
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
                ],
                "traitMethods": [
                    {
                        "owner": "Rewardable",
                        "name": "preview",
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
    let schema = token_modifier_bit(token_modifiers, "schema");
    let schema_host = host | schema;
    let builtin = token_modifier_bit(token_modifiers, "defaultLibrary");

    let text = "\
pub fn main(player: Player, names: Array<String>, rewardable: Rewardable) -> i64 {
    let level = player.level
    player.grant(level)
    return rewardable.preview(names.len())
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
        schema_host,
    );
    assert_token_at(
        &tokens,
        2,
        line(text, 2)
            .find("grant")
            .expect("host method use should exist"),
        "grant".len(),
        method,
        schema_host,
    );
    assert_token_at(
        &tokens,
        3,
        line(text, 3)
            .find("preview")
            .expect("schema trait method call should exist"),
        "preview".len(),
        method,
        schema_host,
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
    let schema = token_modifier_bit(token_modifiers, "schema");
    let schema_host = host | schema;
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
        schema_host,
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
fn lsp_semantic_tokens_classify_schema_method_on_schema_function_return() {
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
                        "name": "current_player",
                        "fact": {
                            "kind": "function",
                            "params": [],
                            "returns": { "kind": "host", "name": "Player" }
                        }
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
    let function = token_type_index(token_types, "function");
    let method = token_type_index(token_types, "method");
    let host = token_modifier_bit(token_modifiers, "host");
    let schema = token_modifier_bit(token_modifiers, "schema");
    let schema_host = host | schema;

    let text = "\
pub fn main() -> i64 {
    return current_player().grant(1)
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
            .find("current_player")
            .expect("schema function call should exist"),
        "current_player".len(),
        function,
        schema_host,
    );
    assert_token_at(
        &tokens,
        1,
        line(text, 1)
            .find("grant")
            .expect("schema method call should exist"),
        "grant".len(),
        method,
        schema_host,
    );

    fs::remove_dir_all(root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_semantic_tokens_classify_schema_enum_variant_uses() {
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
                        "name": "QuestState",
                        "fact": { "kind": "enum", "name": "QuestState", "variant": null }
                    }
                ],
                "variants": [
                    {
                        "owner": "QuestState",
                        "name": "Active",
                        "fact": { "kind": "enum", "name": "QuestState", "variant": "Active" }
                    },
                    {
                        "owner": "QuestState",
                        "name": "Done",
                        "fact": { "kind": "enum", "name": "QuestState", "variant": "Done" }
                    }
                ],
                "fields": [
                    {
                        "owner": "QuestState::Done",
                        "name": "0",
                        "fact": { "kind": "primitive", "name": "i64" }
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
    let enum_member = token_type_index(token_types, "enumMember");
    let host = token_modifier_bit(token_modifiers, "host");
    let schema = token_modifier_bit(token_modifiers, "schema");
    let schema_host = host | schema;

    let text = "\
pub fn main(state: QuestState) -> QuestState {
    let active = QuestState::Active
    let done = QuestState::Done(1)
    match state {
        QuestState::Active => active
        QuestState::Done(value) => done
    }
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

    for (line_index, variant) in [(1, "Active"), (2, "Done"), (4, "Active"), (5, "Done")] {
        assert_token_at(
            &tokens,
            line_index,
            line(text, line_index)
                .find(variant)
                .unwrap_or_else(|| panic!("{variant} should exist")),
            variant.len(),
            enum_member,
            schema_host,
        );
    }

    fs::remove_dir_all(root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_semantic_tokens_classify_host_and_builtin_type_hints() {
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
    let type_token = token_type_index(token_types, "type");
    let builtin_type = token_type_index(token_types, "builtinType");
    let host = token_modifier_bit(token_modifiers, "host");
    let schema = token_modifier_bit(token_modifiers, "schema");
    let schema_host = host | schema;
    let builtin = token_modifier_bit(token_modifiers, "defaultLibrary");

    let text = "\
pub fn main(player: Player, names: Array<String>) -> i64 {
    let next: Player = player
    return 1
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
        0,
        line(text, 0).find("Player").expect("schema type hint"),
        "Player".len(),
        type_token,
        schema_host,
    );
    assert_token_at(
        &tokens,
        0,
        line(text, 0).find("Array").expect("builtin array hint"),
        "Array".len(),
        builtin_type,
        builtin,
    );
    assert_token_at(
        &tokens,
        0,
        line(text, 0).find("String").expect("builtin string hint"),
        "String".len(),
        builtin_type,
        builtin,
    );
    assert_token_at(
        &tokens,
        0,
        line(text, 0).rfind("i64").expect("builtin return hint"),
        "i64".len(),
        builtin_type,
        builtin,
    );
    assert_token_at(
        &tokens,
        1,
        line(text, 1)
            .find("Player")
            .expect("local schema type hint"),
        "Player".len(),
        type_token,
        schema_host,
    );
    fs::remove_dir_all(root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_semantic_tokens_highlighting_showcase_pins_current_legend() {
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
                        "name": "SchemaPlayer",
                        "fact": { "kind": "host", "name": "SchemaPlayer" }
                    }
                ],
                "traits": [
                    {
                        "name": "Rewardable",
                        "fact": { "kind": "trait", "name": "Rewardable" }
                    }
                ],
                "fields": [
                    {
                        "owner": "SchemaPlayer",
                        "name": "level",
                        "fact": { "kind": "primitive", "name": "i64" }
                    }
                ],
                "methods": [
                    {
                        "owner": "SchemaPlayer",
                        "name": "grant",
                        "fact": {
                            "kind": "function",
                            "params": [{ "kind": "primitive", "name": "i64" }],
                            "returns": { "kind": "primitive", "name": "i64" }
                        }
                    }
                ],
                "traitMethods": [
                    {
                        "owner": "Rewardable",
                        "name": "preview",
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
    let struct_token = token_type_index(token_types, "struct");
    let const_token = token_type_index(token_types, "const");
    let keyword = token_type_index(token_types, "keyword");
    let boolean = token_type_index(token_types, "boolean");
    let property = token_type_index(token_types, "property");
    let function = token_type_index(token_types, "function");
    let method = token_type_index(token_types, "method");
    let unresolved_reference = token_type_index(token_types, "unresolvedReference");
    let host = token_modifier_bit(token_modifiers, "host");
    let schema = token_modifier_bit(token_modifiers, "schema");
    let schema_host = host | schema;
    let builtin = token_modifier_bit(token_modifiers, "defaultLibrary");
    let source = token_modifier_bit(token_modifiers, "source");
    let unresolved = token_modifier_bit(token_modifiers, "unresolved");
    let control_flow = token_modifier_bit(token_modifiers, "controlFlow");
    let declaration = token_modifier_bit(token_modifiers, "declaration");
    let definition = token_modifier_bit(token_modifiers, "definition");

    let helper_uri = file_uri(&root.join("scripts").join("game").join("support.vela"));
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": helper_uri,
                "languageId": "vela",
                "version": 1,
                "text": "pub fn source_helper(amount: i64) -> i64 { return amount }"
            }
        }),
    )));
    let uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": HIGHLIGHTING_SHOWCASE
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
        5,
        line(HIGHLIGHTING_SHOWCASE, 5)
            .find("Reward")
            .expect("struct name"),
        "Reward".len(),
        struct_token,
        declaration | definition | source,
    );
    assert_token_at(
        &tokens,
        28,
        line(HIGHLIGHTING_SHOWCASE, 28)
            .find("START_LEVEL")
            .expect("const declaration"),
        "START_LEVEL".len(),
        const_token,
        declaration | definition | source,
    );
    assert_token_at(
        &tokens,
        37,
        line(HIGHLIGHTING_SHOWCASE, 37)
            .find("true")
            .expect("boolean"),
        "true".len(),
        boolean,
        0,
    );
    assert_token_at(
        &tokens,
        41,
        line(HIGHLIGHTING_SHOWCASE, 41)
            .rfind("amount")
            .expect("source field from constructor local"),
        "amount".len(),
        property,
        source,
    );
    assert_token_at(
        &tokens,
        42,
        line(HIGHLIGHTING_SHOWCASE, 42)
            .rfind("level")
            .expect("host field"),
        "level".len(),
        property,
        schema_host,
    );
    assert_token_at(
        &tokens,
        43,
        line(HIGHLIGHTING_SHOWCASE, 43)
            .find("grant")
            .expect("host method"),
        "grant".len(),
        method,
        schema_host,
    );
    assert_token_at(
        &tokens,
        45,
        line(HIGHLIGHTING_SHOWCASE, 45)
            .find("max")
            .expect("stdlib function"),
        "max".len(),
        function,
        builtin,
    );
    assert_token_at(
        &tokens,
        49,
        line(HIGHLIGHTING_SHOWCASE, 49)
            .find("if")
            .expect("control-flow keyword"),
        "if".len(),
        keyword,
        control_flow,
    );
    assert_token_at(
        &tokens,
        46,
        line(HIGHLIGHTING_SHOWCASE, 46)
            .find("bonus")
            .expect("source method"),
        "bonus".len(),
        method,
        source,
    );
    assert_token_at(
        &tokens,
        60,
        line(HIGHLIGHTING_SHOWCASE, 60)
            .find("missing_symbol")
            .expect("unresolved match arm"),
        "missing_symbol".len(),
        unresolved_reference,
        unresolved,
    );
    assert_token_at(
        &tokens,
        63,
        line(HIGHLIGHTING_SHOWCASE, 63)
            .find("unknown_call")
            .expect("unresolved call"),
        "unknown_call".len(),
        unresolved_reference,
        unresolved,
    );

    fs::remove_dir_all(root).expect("temporary workspace should be removable");
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
    static NEXT_WORKSPACE_ID: AtomicU64 = AtomicU64::new(0);

    let mut path = std::env::temp_dir();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after epoch")
        .as_nanos();
    let sequence = NEXT_WORKSPACE_ID.fetch_add(1, Ordering::Relaxed);
    path.push(format!(
        "vela-lsp-semantic-schema-test-{}-{unique}-{sequence}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("temporary workspace should be creatable");
    path
}

fn file_uri(path: &Path) -> String {
    format!("file://{}", path.display())
}
