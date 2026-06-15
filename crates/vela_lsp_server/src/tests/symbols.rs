use super::{LspServer, notification, notification_value, request, response_value};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn lsp_document_symbols_include_nested_script_members() {
    let mut server = LspServer::new();
    let _ = response_value(server.handle_json(&request(
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": "file:///workspace/scripts",
            "capabilities": {}
        }),
    )));
    let text = "\
pub struct Player {
    level: i64
}
pub enum Reward {
    Coins(amount: i64)
}
pub fn main(amount: i64) -> i64 { return amount }";
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": "file:///workspace/scripts/game/main.vela",
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "textDocument/documentSymbol",
        serde_json::json!({
            "textDocument": { "uri": "file:///workspace/scripts/game/main.vela" }
        }),
    )));

    let symbols = response["result"]
        .as_array()
        .expect("documentSymbol should return an array");
    assert_eq!(symbols.len(), 3, "{symbols:?}");
    assert_eq!(symbols[0]["name"], "Player");
    assert_eq!(symbols[0]["kind"], 23);
    assert_eq!(symbols[0]["children"][0]["name"], "level");
    assert_eq!(symbols[0]["children"][0]["kind"], 8);
    assert_eq!(
        symbols[0]["children"][0]["selectionRange"]["start"]["line"],
        1
    );
    assert_eq!(symbols[1]["name"], "Reward");
    assert_eq!(symbols[1]["children"][0]["name"], "Coins");
    assert_eq!(symbols[1]["children"][0]["kind"], 22);
    assert_eq!(symbols[1]["children"][0]["children"][0]["name"], "amount");
    assert_eq!(symbols[2]["name"], "main");
    assert_eq!(symbols[2]["kind"], 12);
    assert_eq!(symbols[2]["detail"], "(amount: i64) -> i64");
}

#[test]
fn lsp_workspace_symbols_include_script_and_schema_symbols() {
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
                ]
            }
        }"#,
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
    let main_uri = file_uri(&root.join("scripts").join("game").join("reward.vela"));
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": main_uri,
                "languageId": "vela",
                "version": 1,
                "text": "pub fn grant() -> i64 { return 1 }"
            }
        }),
    )));

    let response = response_value(server.handle_json(&request(
        2,
        "workspace/symbol",
        serde_json::json!({ "query": "grant" }),
    )));
    let symbols = response["result"]
        .as_array()
        .expect("workspace/symbol should return an array");
    assert!(
        symbols.iter().any(|symbol| {
            symbol["name"] == "game::reward::grant"
                && symbol["kind"] == 12
                && symbol["containerName"] == "game::reward"
                && symbol["location"]["uri"] == main_uri
        }),
        "{symbols:?}"
    );

    let response = response_value(server.handle_json(&request(
        3,
        "workspace/symbol",
        serde_json::json!({ "query": "Player" }),
    )));
    let symbols = response["result"]
        .as_array()
        .expect("workspace/symbol should return an array");
    assert!(
        symbols.iter().any(|symbol| {
            symbol["name"] == "Player"
                && symbol["kind"] == 23
                && symbol["location"]["uri"] == "vela-schema:"
        }),
        "{symbols:?}"
    );
    assert!(
        symbols.iter().any(|symbol| {
            symbol["name"] == "Player::level"
                && symbol["kind"] == 8
                && symbol["detail"] == "i64"
                && symbol["containerName"] == "Player"
        }),
        "{symbols:?}"
    );
    fs::remove_dir_all(&root).expect("temporary workspace should be removable");
}

fn temp_workspace() -> PathBuf {
    let suffix = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(error) => panic!("system time should be after UNIX_EPOCH: {error}"),
    };
    let root = std::env::temp_dir().join(format!(
        "vela_lsp_symbols_{}_{}",
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
