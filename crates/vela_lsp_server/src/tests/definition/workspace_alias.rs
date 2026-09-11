use std::fs;

use crate::tests::{TestServer, notification_values, notify, request, response_value};

#[test]
fn package_workspace_alias_preserves_overlay_navigation_and_deleted_file_events()
-> std::io::Result<()> {
    let temp = super::temp_workspace();
    let physical = temp.join("physical");
    let alias = temp.join("中文 linked workspace");
    fs::create_dir_all(physical.join("scripts"))?;
    std::os::unix::fs::symlink(&physical, &alias)?;
    fs::write(
        physical.join("vela.toml"),
        "[package]\nid = 'dev.vela.alias-test'\nname = 'alias_test'\nversion = '0.1.0'\n[source]\nroots = ['scripts']\n",
    )?;
    let caller = "use helper::increment;\nfn main() { return increment(41); }\n";
    fs::write(physical.join("scripts/main.vela"), caller)?;
    fs::write(
        physical.join("scripts/helper.vela"),
        "pub fn increment(value) { return value + 1; }\n",
    )?;
    let uri = |path: &std::path::Path| {
        lsp_types::Url::from_file_path(path)
            .expect("absolute fixture path should form a file URI")
            .to_string()
    };
    let main_uri = uri(&alias.join("scripts/main.vela"));
    let helper_uri = uri(&alias.join("scripts/helper.vela"));
    let mut server = TestServer::new();
    let _ = request::<lsp_types::request::Initialize>(
        &mut server,
        1,
        serde_json::json!({ "processId": null, "rootUri": uri(&alias), "capabilities": {} }),
    );
    let messages = notification_values(notify::<lsp_types::notification::DidOpenTextDocument>(
        &mut server,
        serde_json::json!({ "textDocument": {
            "uri": main_uri, "languageId": "vela", "version": 1,
            "text": format!("// unsaved\n{caller}")
        } }),
    ));
    assert!(
        messages.iter().any(|message| {
            message["method"] == "textDocument/publishDiagnostics"
                && message["params"]["uri"] == main_uri
                && message["params"]["diagnostics"] == serde_json::json!([])
        }),
        "{messages:?}"
    );
    let target = response_value(request::<lsp_types::request::GotoDefinition>(
        &mut server,
        2,
        serde_json::json!({ "textDocument": { "uri": main_uri },
            "position": { "line": 2, "character": 20 } }),
    ));
    assert_eq!(
        target["result"],
        serde_json::json!({
            "uri": helper_uri,
            "range": { "start": { "line": 0, "character": 7 },
                       "end": { "line": 0, "character": 16 } }
        })
    );

    // Watchers may report the physical spelling after the file is gone. The
    // source retained by package discovery must still be removed.
    let physical_helper = fs::canonicalize(physical.join("scripts/helper.vela"))?;
    fs::remove_file(&physical_helper)?;
    let messages = notification_values(notify::<lsp_types::notification::DidChangeWatchedFiles>(
        &mut server,
        serde_json::json!({ "changes": [{ "uri": uri(&physical_helper), "type": 3 }] }),
    ));
    assert!(
        messages.iter().any(|message| {
            message["params"]["uri"] == main_uri
                && message["params"]["diagnostics"]
                    .as_array()
                    .is_some_and(|diagnostics| {
                        diagnostics.iter().any(|diagnostic| {
                            diagnostic["code"] == "project::diagnostic"
                                && diagnostic["message"]
                                    .as_str()
                                    .is_some_and(|text| text.contains("unresolved module"))
                        })
                    })
        }),
        "{messages:?}"
    );
    let target = response_value(request::<lsp_types::request::GotoDefinition>(
        &mut server,
        3,
        serde_json::json!({ "textDocument": { "uri": main_uri },
            "position": { "line": 2, "character": 20 } }),
    ));
    assert_eq!(target["result"], serde_json::Value::Null);
    fs::remove_dir_all(temp)
}
