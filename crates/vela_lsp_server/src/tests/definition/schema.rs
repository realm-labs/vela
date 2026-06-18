use super::*;

#[test]
fn lsp_definition_follows_schema_source_span() {
    assert_schema_source_navigation("textDocument/definition");
}

#[test]
fn lsp_declaration_follows_schema_source_span() {
    assert_schema_source_navigation("textDocument/declaration");
}

#[test]
fn lsp_type_definition_follows_schema_source_span() {
    assert_schema_source_navigation("textDocument/typeDefinition");
}

#[test]
fn lsp_definition_returns_null_for_schema_type_without_source_span() {
    assert_schema_type_without_source_span_null("textDocument/definition");
}

#[test]
fn lsp_declaration_returns_null_for_schema_type_without_source_span() {
    assert_schema_type_without_source_span_null("textDocument/declaration");
}

#[test]
fn lsp_type_definition_returns_null_for_schema_type_without_source_span() {
    assert_schema_type_without_source_span_null("textDocument/typeDefinition");
}

#[test]
fn lsp_definition_follows_schema_field_source_span() {
    assert_schema_field_source_navigation("textDocument/definition");
}

#[test]
fn lsp_declaration_follows_schema_field_source_span() {
    assert_schema_field_source_navigation("textDocument/declaration");
}

#[test]
fn lsp_type_definition_follows_schema_field_type_source_span() {
    assert_schema_field_type_source_navigation();
}

#[test]
fn lsp_type_definition_returns_null_for_schema_primitive_field() {
    assert_schema_field_type_definition_null();
}

fn assert_schema_type_without_source_span_null(request_method: &str) {
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
        serde_json::json!({
            "formatVersion": 1,
            "facts": {
                "types": [
                    {
                        "name": "Player",
                        "fact": { "kind": "host", "name": "Player" }
                    }
                ]
            }
        })
        .to_string(),
    )
    .expect("schema should be writable");

    let mut server = LspServer::new();
    let _ = response_value(server.handle_json(&request(
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
    let main_uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    let main_text = "pub fn main(player: Player) { return 1 }";
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": main_uri,
                "languageId": "vela",
                "version": 1,
                "text": main_text
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        request_method,
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 0,
                "character": main_text.find("Player").expect("type hint should exist")
            }
        }),
    )));

    assert!(response["result"].is_null());
    fs::remove_dir_all(root).expect("temporary workspace should be removable");
}

fn assert_schema_field_source_navigation(request_method: &str) {
    let root = temp_workspace();
    let config_path = root.join("vela.toml");
    let schema_path = root.join("target").join("vela").join("schema.json");
    fs::create_dir_all(schema_path.parent().expect("schema should have parent"))
        .expect("schema directory should be creatable");
    let schema_text = "pub fn level_marker() { return 1 }";
    let target_start = schema_text
        .find("level_marker")
        .expect("schema target marker should exist");
    let target_end = target_start + "level_marker".len();
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
        serde_json::json!({
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
                        "fact": { "kind": "primitive", "name": "i64" },
                        "sourceSpan": {
                            "source": 1,
                            "start": target_start,
                            "end": target_end
                        }
                    }
                ]
            }
        })
        .to_string(),
    )
    .expect("schema should be writable");

    let mut server = LspServer::new();
    let _ = response_value(server.handle_json(&request(
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
    let schema_uri = file_uri(&root.join("scripts").join("_schema_defs.vela"));
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": schema_uri,
                "languageId": "vela",
                "version": 1,
                "text": schema_text
            }
        }),
    )));
    let main_uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    let main_text = "pub fn main(player: Player) { return player.level }";
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": main_uri,
                "languageId": "vela",
                "version": 1,
                "text": main_text
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        request_method,
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 0,
                "character": main_text.find("level").expect("field use should exist")
            }
        }),
    )));

    assert_eq!(response["result"]["uri"], schema_uri);
    assert_eq!(response["result"]["range"]["start"]["line"], 0);
    assert_eq!(
        response["result"]["range"]["start"]["character"],
        target_start
    );
    assert_eq!(response["result"]["range"]["end"]["character"], target_end);
    fs::remove_dir_all(root).expect("temporary workspace should be removable");
}

fn assert_schema_field_type_source_navigation() {
    assert_schema_member_source_navigation(
        "textDocument/typeDefinition",
        "pub fn inventory_type_marker() { return 1 }",
        "inventory_type_marker",
        "pub fn main(player: Player) { return player.inventory }",
        "inventory",
        |target_start, target_end| {
            serde_json::json!({
                "types": [
                    {
                        "name": "Player",
                        "fact": { "kind": "host", "name": "Player" }
                    },
                    {
                        "name": "Inventory",
                        "fact": { "kind": "host", "name": "Inventory" },
                        "sourceSpan": {
                            "source": 1,
                            "start": target_start,
                            "end": target_end
                        }
                    }
                ],
                "fields": [
                    {
                        "owner": "Player",
                        "name": "inventory",
                        "fact": { "kind": "host", "name": "Inventory" }
                    }
                ]
            })
        },
    );
}

fn assert_schema_field_type_definition_null() {
    assert_schema_member_source_navigation_null(
        "pub fn level_marker() { return 1 }",
        "pub fn main(player: Player) { return player.level }",
        "level",
        |target_start, target_end| {
            serde_json::json!({
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
                        "fact": { "kind": "primitive", "name": "i64" },
                        "sourceSpan": {
                            "source": 1,
                            "start": target_start,
                            "end": target_end
                        }
                    }
                ]
            })
        },
    );
}

#[test]
fn lsp_definition_follows_schema_method_source_span() {
    assert_schema_host_method_source_navigation("textDocument/definition");
}

#[test]
fn lsp_declaration_follows_schema_method_source_span() {
    assert_schema_host_method_source_navigation("textDocument/declaration");
}

#[test]
fn lsp_definition_follows_schema_method_on_schema_method_return_receiver() {
    assert_schema_method_on_schema_method_return_navigation("textDocument/definition");
}

#[test]
fn lsp_declaration_follows_schema_method_on_schema_method_return_receiver() {
    assert_schema_method_on_schema_method_return_navigation("textDocument/declaration");
}

#[test]
fn lsp_type_definition_returns_null_for_schema_method() {
    assert_schema_host_method_type_definition_null();
}

fn assert_schema_host_method_source_navigation(request_method: &str) {
    assert_schema_member_source_navigation(
        request_method,
        "pub fn grant_marker() { return 1 }",
        "grant_marker",
        "pub fn main(player: Player) { return player.grant(1) }",
        "grant",
        |target_start, target_end| {
            serde_json::json!({
                "types": [
                    {
                        "name": "Player",
                        "fact": { "kind": "host", "name": "Player" }
                    }
                ],
                "methods": [
                    {
                        "owner": "Player",
                        "name": "grant",
                        "fact": {
                            "kind": "function",
                            "params": [{ "kind": "primitive", "name": "i64" }],
                            "returns": { "kind": "primitive", "name": "bool" }
                        },
                        "sourceSpan": {
                            "source": 1,
                            "start": target_start,
                            "end": target_end
                        }
                    }
                ]
            })
        },
    );
}

fn assert_schema_method_on_schema_method_return_navigation(request_method: &str) {
    assert_schema_member_source_navigation(
        request_method,
        "pub fn grant_marker() { return 1 }",
        "grant_marker",
        "pub fn main(player: Player) { return player.inventory().grant(1) }",
        "grant",
        |target_start, target_end| {
            serde_json::json!({
                "types": [
                    {
                        "name": "Player",
                        "fact": { "kind": "host", "name": "Player" }
                    },
                    {
                        "name": "Inventory",
                        "fact": { "kind": "host", "name": "Inventory" }
                    }
                ],
                "methods": [
                    {
                        "owner": "Player",
                        "name": "inventory",
                        "fact": {
                            "kind": "function",
                            "params": [],
                            "returns": { "kind": "host", "name": "Inventory" }
                        }
                    },
                    {
                        "owner": "Inventory",
                        "name": "grant",
                        "fact": {
                            "kind": "function",
                            "params": [{ "kind": "primitive", "name": "i64" }],
                            "returns": { "kind": "primitive", "name": "bool" }
                        },
                        "sourceSpan": {
                            "source": 1,
                            "start": target_start,
                            "end": target_end
                        }
                    }
                ]
            })
        },
    );
}

fn assert_schema_host_method_type_definition_null() {
    assert_schema_member_source_navigation_null(
        "pub fn grant_marker() { return 1 }",
        "pub fn main(player: Player) { return player.grant(1) }",
        "grant",
        |target_start, target_end| {
            serde_json::json!({
                "types": [
                    {
                        "name": "Player",
                        "fact": { "kind": "host", "name": "Player" }
                    }
                ],
                "methods": [
                    {
                        "owner": "Player",
                        "name": "grant",
                        "fact": {
                            "kind": "function",
                            "params": [{ "kind": "primitive", "name": "i64" }],
                            "returns": { "kind": "primitive", "name": "bool" }
                        },
                        "sourceSpan": {
                            "source": 1,
                            "start": target_start,
                            "end": target_end
                        }
                    }
                ]
            })
        },
    );
}

#[test]
fn lsp_definition_follows_schema_trait_method_source_span() {
    assert_schema_trait_method_source_navigation("textDocument/definition");
}

#[test]
fn lsp_declaration_follows_schema_trait_method_source_span() {
    assert_schema_trait_method_source_navigation("textDocument/declaration");
}

#[test]
fn lsp_definition_follows_schema_trait_method_on_schema_method_return_receiver() {
    assert_schema_trait_method_on_schema_method_return_navigation("textDocument/definition");
}

#[test]
fn lsp_declaration_follows_schema_trait_method_on_schema_method_return_receiver() {
    assert_schema_trait_method_on_schema_method_return_navigation("textDocument/declaration");
}

#[test]
fn lsp_type_definition_returns_null_for_schema_trait_method() {
    assert_schema_trait_method_type_definition_null();
}

fn assert_schema_trait_method_source_navigation(request_method: &str) {
    assert_schema_member_source_navigation(
        request_method,
        "pub fn preview_marker() { return 1 }",
        "preview_marker",
        "pub fn main(rewardable: Rewardable) { return rewardable.preview(1) }",
        "preview",
        |target_start, target_end| {
            serde_json::json!({
                "traits": [
                    {
                        "name": "Rewardable",
                        "fact": { "kind": "trait", "name": "Rewardable" }
                    }
                ],
                "traitMethods": [
                    {
                        "owner": "Rewardable",
                        "name": "preview",
                        "fact": {
                            "kind": "function",
                            "params": [{ "kind": "primitive", "name": "i64" }],
                            "returns": { "kind": "primitive", "name": "bool" }
                        },
                        "sourceSpan": {
                            "source": 1,
                            "start": target_start,
                            "end": target_end
                        }
                    }
                ]
            })
        },
    );
}

fn assert_schema_trait_method_on_schema_method_return_navigation(request_method: &str) {
    assert_schema_member_source_navigation(
        request_method,
        "pub fn preview_marker() { return 1 }",
        "preview_marker",
        "pub fn main(player: Player) { return player.rewardable().preview(1) }",
        "preview",
        |target_start, target_end| {
            serde_json::json!({
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
                "methods": [
                    {
                        "owner": "Player",
                        "name": "rewardable",
                        "fact": {
                            "kind": "function",
                            "params": [],
                            "returns": { "kind": "trait", "name": "Rewardable" }
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
                            "returns": { "kind": "primitive", "name": "bool" }
                        },
                        "sourceSpan": {
                            "source": 1,
                            "start": target_start,
                            "end": target_end
                        }
                    }
                ]
            })
        },
    );
}

fn assert_schema_trait_method_type_definition_null() {
    assert_schema_member_source_navigation_null(
        "pub fn preview_marker() { return 1 }",
        "pub fn main(rewardable: Rewardable) { return rewardable.preview(1) }",
        "preview",
        |target_start, target_end| {
            serde_json::json!({
                "traits": [
                    {
                        "name": "Rewardable",
                        "fact": { "kind": "trait", "name": "Rewardable" }
                    }
                ],
                "traitMethods": [
                    {
                        "owner": "Rewardable",
                        "name": "preview",
                        "fact": {
                            "kind": "function",
                            "params": [{ "kind": "primitive", "name": "i64" }],
                            "returns": { "kind": "primitive", "name": "bool" }
                        },
                        "sourceSpan": {
                            "source": 1,
                            "start": target_start,
                            "end": target_end
                        }
                    }
                ]
            })
        },
    );
}

#[test]
fn lsp_definition_follows_schema_variant_source_span() {
    assert_schema_variant_source_navigation("textDocument/definition");
}

#[test]
fn lsp_declaration_follows_schema_variant_source_span() {
    assert_schema_variant_source_navigation("textDocument/declaration");
}

#[test]
fn lsp_type_definition_returns_null_for_schema_variant_without_owner_type_span() {
    assert_schema_variant_type_definition_null();
}

fn assert_schema_variant_source_navigation(request_method: &str) {
    let root = temp_workspace();
    let config_path = root.join("vela.toml");
    let schema_path = root.join("target").join("vela").join("schema.json");
    fs::create_dir_all(schema_path.parent().expect("schema should have parent"))
        .expect("schema directory should be creatable");
    let schema_text = "pub fn active_marker() { return 1 }";
    let target_start = schema_text
        .find("active_marker")
        .expect("schema target marker should exist");
    let target_end = target_start + "active_marker".len();
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
        serde_json::json!({
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
                        "fact": {
                            "kind": "enum",
                            "name": "QuestState",
                            "variant": "Active"
                        },
                        "sourceSpan": {
                            "source": 1,
                            "start": target_start,
                            "end": target_end
                        }
                    }
                ]
            }
        })
        .to_string(),
    )
    .expect("schema should be writable");

    let mut server = LspServer::new();
    let _ = response_value(server.handle_json(&request(
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
    let schema_uri = file_uri(&root.join("scripts").join("_schema_defs.vela"));
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": schema_uri,
                "languageId": "vela",
                "version": 1,
                "text": schema_text
            }
        }),
    )));
    let main_uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    let main_text = "pub fn main() { return QuestState::Active }";
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": main_uri,
                "languageId": "vela",
                "version": 1,
                "text": main_text
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        request_method,
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 0,
                "character": main_text.find("Active").expect("variant use should exist")
            }
        }),
    )));

    assert_eq!(response["result"]["uri"], schema_uri);
    assert_eq!(response["result"]["range"]["start"]["line"], 0);
    assert_eq!(
        response["result"]["range"]["start"]["character"],
        target_start
    );
    assert_eq!(response["result"]["range"]["end"]["character"], target_end);
    fs::remove_dir_all(root).expect("temporary workspace should be removable");
}

fn assert_schema_variant_type_definition_null() {
    let root = temp_workspace();
    let config_path = root.join("vela.toml");
    let schema_path = root.join("target").join("vela").join("schema.json");
    fs::create_dir_all(schema_path.parent().expect("schema should have parent"))
        .expect("schema directory should be creatable");
    let schema_text = "pub fn active_marker() { return 1 }";
    let target_start = schema_text
        .find("active_marker")
        .expect("schema target marker should exist");
    let target_end = target_start + "active_marker".len();
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
        serde_json::json!({
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
                        "fact": {
                            "kind": "enum",
                            "name": "QuestState",
                            "variant": "Active"
                        },
                        "sourceSpan": {
                            "source": 1,
                            "start": target_start,
                            "end": target_end
                        }
                    }
                ]
            }
        })
        .to_string(),
    )
    .expect("schema should be writable");

    let mut server = LspServer::new();
    let _ = response_value(server.handle_json(&request(
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
    let schema_uri = file_uri(&root.join("scripts").join("_schema_defs.vela"));
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": schema_uri,
                "languageId": "vela",
                "version": 1,
                "text": schema_text
            }
        }),
    )));
    let main_uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    let main_text = "pub fn main() { return QuestState::Active }";
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": main_uri,
                "languageId": "vela",
                "version": 1,
                "text": main_text
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/typeDefinition",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 0,
                "character": main_text.find("Active").expect("variant use should exist")
            }
        }),
    )));

    assert!(response["result"].is_null());
    fs::remove_dir_all(root).expect("temporary workspace should be removable");
}

fn assert_schema_member_source_navigation<F>(
    request_method: &str,
    schema_text: &str,
    schema_marker: &str,
    main_text: &str,
    usage_needle: &str,
    facts: F,
) where
    F: FnOnce(usize, usize) -> serde_json::Value,
{
    let root = temp_workspace();
    let config_path = root.join("vela.toml");
    let schema_path = root.join("target").join("vela").join("schema.json");
    fs::create_dir_all(schema_path.parent().expect("schema should have parent"))
        .expect("schema directory should be creatable");
    let target_start = schema_text
        .find(schema_marker)
        .expect("schema target marker should exist");
    let target_end = target_start + schema_marker.len();
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
        serde_json::json!({
            "formatVersion": 1,
            "facts": facts(target_start, target_end)
        })
        .to_string(),
    )
    .expect("schema should be writable");

    let mut server = LspServer::new();
    let _ = response_value(server.handle_json(&request(
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
    let schema_uri = file_uri(&root.join("scripts").join("_schema_defs.vela"));
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": schema_uri,
                "languageId": "vela",
                "version": 1,
                "text": schema_text
            }
        }),
    )));
    let main_uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": main_uri,
                "languageId": "vela",
                "version": 1,
                "text": main_text
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        request_method,
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 0,
                "character": main_text.find(usage_needle).expect("member use should exist")
            }
        }),
    )));

    assert_eq!(response["result"]["uri"], schema_uri);
    assert_eq!(response["result"]["range"]["start"]["line"], 0);
    assert_eq!(
        response["result"]["range"]["start"]["character"],
        target_start
    );
    assert_eq!(response["result"]["range"]["end"]["character"], target_end);
    fs::remove_dir_all(root).expect("temporary workspace should be removable");
}

fn assert_schema_member_source_navigation_null<F>(
    schema_text: &str,
    main_text: &str,
    usage_needle: &str,
    facts: F,
) where
    F: FnOnce(usize, usize) -> serde_json::Value,
{
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
        serde_json::json!({
            "formatVersion": 1,
            "facts": facts(0, schema_text.len())
        })
        .to_string(),
    )
    .expect("schema should be writable");

    let mut server = LspServer::new();
    let _ = response_value(server.handle_json(&request(
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
    let schema_uri = file_uri(&root.join("scripts").join("_schema_defs.vela"));
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": schema_uri,
                "languageId": "vela",
                "version": 1,
                "text": schema_text
            }
        }),
    )));
    let main_uri = file_uri(&root.join("scripts").join("game").join("main.vela"));
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": main_uri,
                "languageId": "vela",
                "version": 1,
                "text": main_text
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/typeDefinition",
        serde_json::json!({
            "textDocument": { "uri": main_uri },
            "position": {
                "line": 0,
                "character": main_text.find(usage_needle).expect("member use should exist")
            }
        }),
    )));

    assert!(response["result"].is_null());
    fs::remove_dir_all(root).expect("temporary workspace should be removable");
}
