use crate::tests::{
    TestServer, navigation_request, notification_values, notify, request, response_value,
};

#[test]
fn initialize_loads_unopened_package_sources_from_encoded_workspace_uri() {
    let temp = super::temp_workspace();
    let root = temp.join("中文 workspace");
    std::fs::create_dir_all(root.join("scripts")).expect("fixture directory");
    std::fs::write(root.join("vela.toml"), "[package]\nid = 'dev.vela.editor-test'\nname = 'editor_test'\nversion = '0.1.0'\n[source]\nroots = ['scripts']\n").expect("manifest");
    let caller = "use helpers::increment;\nfn main() { return increment(41); }\n";
    std::fs::write(root.join("scripts/main.vela"), caller).expect("caller");
    std::fs::write(
        root.join("scripts/helpers.vela"),
        "pub fn increment(value) { return value + 1; }\n",
    )
    .expect("target");
    let uri = |path: &std::path::Path| {
        let uri = lsp_types::Url::from_file_path(path)
            .expect("file URI")
            .to_string();
        if cfg!(windows) {
            let drive = uri[8..9].to_ascii_lowercase();
            format!("file:///{drive}%3A{}", &uri[10..])
        } else {
            uri
        }
    };
    let mut server = TestServer::new();
    let _ = request::<lsp_types::request::Initialize>(
        &mut server,
        1,
        serde_json::json!({
            "processId": null, "rootUri": uri(&root), "capabilities": {}
        }),
    );
    let caller_uri = uri(&root.join("scripts/main.vela"));
    let overlay = format!("// unsaved\n{caller}");
    let notifications = notification_values(
        notify::<lsp_types::notification::DidOpenTextDocument>(
            &mut server,
            serde_json::json!({
                "textDocument": { "uri": caller_uri, "languageId": "vela", "version": 1, "text": overlay }
            }),
        ),
    );
    for notification in notifications {
        if let Some(diagnostics) = notification["params"]["diagnostics"].as_array() {
            assert!(
                diagnostics.iter().all(|diagnostic| !diagnostic["message"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("duplicate module")),
                "{diagnostics:?}"
            );
        }
    }
    let response = response_value(navigation_request(
        &mut server,
        2,
        "textDocument/definition",
        serde_json::json!({
            "textDocument": { "uri": caller_uri }, "position": { "line": 2, "character": 20 }
        }),
    ));
    let target = response["result"]["uri"]
        .as_str()
        .expect("unopened file must resolve");
    assert_eq!(
        std::fs::canonicalize(
            lsp_types::Url::parse(target)
                .expect("target URI")
                .to_file_path()
                .expect("target file path")
        )
        .expect("target exists"),
        std::fs::canonicalize(root.join("scripts/helpers.vela")).expect("fixture exists")
    );
    assert_eq!(
        response["result"]["range"]["start"],
        serde_json::json!({ "line": 0, "character": 7 })
    );
    std::fs::remove_dir_all(temp).expect("fixture cleanup");
}

#[test]
fn navigation_projects_target_byte_columns_to_utf16_from_unsaved_cross_file_text() {
    for method in ["textDocument/definition", "textDocument/declaration"] {
        let mut server = TestServer::new();
        let _ = request::<lsp_types::request::Initialize>(
            &mut server,
            1,
            serde_json::json!({
                "processId": null,
                "rootUri": "file:///workspace/scripts",
                "capabilities": {}
            }),
        );
        let target_uri = "file:///workspace/scripts/helpers.vela";
        let target =
            "// unsaved overlay\r\n/* 中文 😀 */ pub fn increment(value) { return value + 1; }\r\n";
        let caller_uri = "file:///workspace/scripts/main.vela";
        let caller = "use helpers::increment;\nfn main() { return increment(41); }\n";
        for (uri, text) in [(target_uri, target), (caller_uri, caller)] {
            let _ = notify::<lsp_types::notification::DidOpenTextDocument>(
                &mut server,
                serde_json::json!({
                    "textDocument": { "uri": uri, "languageId": "vela", "version": 1, "text": text }
                }),
            );
        }
        let response = response_value(navigation_request(
            &mut server,
            2,
            method,
            serde_json::json!({
                "textDocument": { "uri": caller_uri },
                "position": { "line": 1, "character": 20 }
            }),
        ));
        let start = "/* 中文 😀 */ pub fn ".encode_utf16().count();
        assert_eq!(response["result"]["uri"], target_uri, "{response}");
        assert_eq!(
            response["result"]["range"],
            serde_json::json!({
                "start": { "line": 1, "character": start },
                "end": { "line": 1, "character": start + "increment".len() }
            })
        );
    }
}
