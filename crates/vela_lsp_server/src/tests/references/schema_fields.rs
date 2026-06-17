use std::fs;
use std::path::PathBuf;

use crate::tests::{LspServer, notification, notification_value, request, response_value};

use super::{assert_highlight, assert_reference, file_uri, line, temp_workspace};

#[test]
fn lsp_references_find_schema_record_constructor_shorthand_field_labels() {
    let schema_text = "pub fn level() { return 1 }";
    let text = "\
pub fn make(level: i64) -> Player {
    let player = Player { level }
    return player
}

pub fn main(player: Player) -> i64 {
    return player.level
}";
    let mut server = LspServer::new();
    let (root, schema_uri, uri) = open_schema_field_workspace(&mut server, schema_text, text);

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": schema_uri },
            "position": {
                "line": 0,
                "character": line(schema_text, 0).find("level").expect("schema field")
            },
            "context": { "includeDeclaration": true }
        }),
    )));
    let references = response["result"]
        .as_array()
        .expect("references response should be an array");

    assert_eq!(references.len(), 3, "{references:?}");
    assert_reference(
        references,
        &schema_uri,
        0,
        line(schema_text, 0).find("level").expect("schema field"),
    );
    assert_reference(
        references,
        &uri,
        1,
        line(text, 1)
            .find("level")
            .expect("constructor shorthand field label"),
    );
    assert_reference(
        references,
        &uri,
        6,
        line(text, 6).find("level").expect("member field read"),
    );

    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

#[test]
fn lsp_document_highlight_marks_schema_field_reads_and_writes() {
    let schema_text = "pub fn level() { return 1 }";
    let text = "\
pub fn main(player: Player) -> i64 {
    let first = player.level
    player.level += 1
    return player.level + first
}";
    let mut server = LspServer::new();
    let (root, _, uri) = open_schema_field_workspace(&mut server, schema_text, text);

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/documentHighlight",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": 1,
                "character": line(text, 1).find("level").expect("first field read")
            }
        }),
    )));
    let highlights = response["result"]
        .as_array()
        .expect("documentHighlight response should be an array");

    assert_eq!(highlights.len(), 3, "{highlights:?}");
    assert_highlight(
        highlights,
        1,
        line(text, 1).find("level").expect("first field read"),
        2,
    );
    assert_highlight(
        highlights,
        2,
        line(text, 2).find("level").expect("field write"),
        3,
    );
    assert_highlight(
        highlights,
        3,
        line(text, 3).find("level").expect("second field read"),
        2,
    );

    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

fn open_schema_field_workspace(
    server: &mut LspServer,
    schema_text: &str,
    text: &str,
) -> (PathBuf, String, String) {
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

    let target_start = schema_text
        .find("level")
        .expect("schema target marker should exist");
    let target_end = target_start + "level".len();
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

    (root, schema_uri, uri)
}
