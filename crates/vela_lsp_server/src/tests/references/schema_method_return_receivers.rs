use std::{fs, path::PathBuf};

use crate::tests::{
    LspServer, handle_notification, handle_request, notification_value, response_value,
};

use super::{assert_highlight, assert_reference, file_uri, line, temp_workspace};

#[test]
fn lsp_references_find_schema_method_calls_on_schema_method_return_receivers() {
    let mut fixture = open_schema_method_return_fixture();

    let response = response_value(handle_request(
        &mut fixture.server,
        2,
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": fixture.uri },
            "position": {
                "line": 1,
                "character": line(fixture.text, 1)
                    .find("grant")
                    .expect("method call")
            },
            "context": { "includeDeclaration": true }
        }),
    ));
    let references = response["result"]
        .as_array()
        .expect("references response should be an array");

    assert_eq!(references.len(), 3, "{references:?}");
    assert_reference(
        references,
        &fixture.schema_uri,
        0,
        fixture
            .schema_text
            .find("grant")
            .expect("schema method declaration"),
    );
    assert_reference(
        references,
        &fixture.uri,
        1,
        line(fixture.text, 1)
            .find("grant")
            .expect("first method call"),
    );
    assert_reference(
        references,
        &fixture.uri,
        2,
        line(fixture.text, 2)
            .find("grant")
            .expect("second method call"),
    );

    fs::remove_dir_all(&fixture.root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_references_find_schema_trait_method_calls_on_schema_method_return_receivers() {
    let mut fixture = open_schema_trait_method_return_fixture();

    let response = response_value(handle_request(
        &mut fixture.server,
        2,
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": fixture.uri },
            "position": {
                "line": 1,
                "character": line(fixture.text, 1)
                    .find("preview")
                    .expect("method call")
            },
            "context": { "includeDeclaration": true }
        }),
    ));
    let references = response["result"]
        .as_array()
        .expect("references response should be an array");

    assert_eq!(references.len(), 3, "{references:?}");
    assert_reference(
        references,
        &fixture.schema_uri,
        0,
        fixture
            .schema_text
            .find("preview")
            .expect("schema trait method declaration"),
    );
    assert_reference(
        references,
        &fixture.uri,
        1,
        line(fixture.text, 1)
            .find("preview")
            .expect("first method call"),
    );
    assert_reference(
        references,
        &fixture.uri,
        2,
        line(fixture.text, 2)
            .find("preview")
            .expect("second method call"),
    );

    fs::remove_dir_all(&fixture.root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_document_highlight_marks_schema_method_calls_on_schema_method_return_receivers() {
    let mut fixture = open_schema_method_return_fixture();

    let response = response_value(handle_request(
        &mut fixture.server,
        2,
        "textDocument/documentHighlight",
        serde_json::json!({
            "textDocument": { "uri": fixture.uri },
            "position": {
                "line": 1,
                "character": line(fixture.text, 1)
                    .find("grant")
                    .expect("method call")
            }
        }),
    ));
    let highlights = response["result"]
        .as_array()
        .expect("documentHighlight response should be an array");

    assert_eq!(highlights.len(), 2, "{highlights:?}");
    assert_highlight(
        highlights,
        1,
        line(fixture.text, 1)
            .find("grant")
            .expect("first method call"),
        1,
    );
    assert_highlight(
        highlights,
        2,
        line(fixture.text, 2)
            .find("grant")
            .expect("second method call"),
        1,
    );

    fs::remove_dir_all(&fixture.root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_document_highlight_marks_schema_trait_method_calls_on_schema_method_return_receivers() {
    let mut fixture = open_schema_trait_method_return_fixture();

    let response = response_value(handle_request(
        &mut fixture.server,
        2,
        "textDocument/documentHighlight",
        serde_json::json!({
            "textDocument": { "uri": fixture.uri },
            "position": {
                "line": 1,
                "character": line(fixture.text, 1)
                    .find("preview")
                    .expect("method call")
            }
        }),
    ));
    let highlights = response["result"]
        .as_array()
        .expect("documentHighlight response should be an array");

    assert_eq!(highlights.len(), 2, "{highlights:?}");
    assert_highlight(
        highlights,
        1,
        line(fixture.text, 1)
            .find("preview")
            .expect("first method call"),
        1,
    );
    assert_highlight(
        highlights,
        2,
        line(fixture.text, 2)
            .find("preview")
            .expect("second method call"),
        1,
    );

    fs::remove_dir_all(&fixture.root).expect("temporary workspace should be removable");
}

struct SchemaReturnFixture {
    server: LspServer,
    root: PathBuf,
    uri: String,
    schema_uri: String,
    text: &'static str,
    schema_text: &'static str,
}

fn open_schema_method_return_fixture() -> SchemaReturnFixture {
    let schema_text = "pub fn grant() { return 1 }";
    let target_start = schema_text
        .find("grant")
        .expect("schema target marker should exist");
    let target_end = target_start + "grant".len();
    open_fixture(
        schema_text,
        serde_json::json!({
            "formatVersion": 1,
            "facts": {
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
                            "returns": { "kind": "primitive", "name": "i64" }
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
        "\
pub fn main(player: Player) -> i64 {
    let first = player.inventory().grant(1)
    return player.inventory().grant(first)
}",
    )
}

fn open_schema_trait_method_return_fixture() -> SchemaReturnFixture {
    let schema_text = "pub fn preview() { return 1 }";
    let target_start = schema_text
        .find("preview")
        .expect("schema target marker should exist");
    let target_end = target_start + "preview".len();
    open_fixture(
        schema_text,
        serde_json::json!({
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
                            "returns": { "kind": "primitive", "name": "i64" }
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
        "\
pub fn main(player: Player) -> i64 {
    let first = player.rewardable().preview(1)
    return player.rewardable().preview(first)
}",
    )
}

fn open_fixture(
    schema_text: &'static str,
    schema_artifact: String,
    text: &'static str,
) -> SchemaReturnFixture {
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
    fs::write(&schema_path, schema_artifact).expect("schema should be writable");

    let mut server = LspServer::new();
    let _ = response_value(handle_request(
        &mut server,
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": file_uri(&root),
            "capabilities": {}
        }),
    ));
    let _ = handle_notification(
        &mut server,
        "workspace/didChangeWatchedFiles",
        serde_json::json!({
            "changes": [{ "uri": file_uri(&config_path), "type": 1 }]
        }),
    );

    let schema_uri = file_uri(&root.join("scripts").join("_schema_defs.vela"));
    let _ = notification_value(handle_notification(
        &mut server,
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": schema_uri,
                "languageId": "vela",
                "version": 1,
                "text": schema_text
            }
        }),
    ));

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

    SchemaReturnFixture {
        server,
        root,
        uri,
        schema_uri,
        text,
        schema_text,
    }
}
