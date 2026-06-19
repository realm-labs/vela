use crate::tests::{
    LspServer, handle_notification, handle_request, notification_value, response_value,
};

#[test]
fn lsp_private_function_rename_updates_aliased_import_path() {
    let mut server = LspServer::new();
    let _ = response_value(handle_request(
        &mut server,
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": "file:///workspace/scripts",
            "capabilities": {}
        }),
    ));
    let main_text = "\
use game::reward::grant as award
pub fn main(amount: i64) -> i64 {
    return award(amount)
}";
    let helper_text = "pub fn grant(amount: i64) -> i64 { return amount }";
    let main_uri = "file:///workspace/scripts/game/main.vela";
    let helper_uri = "file:///workspace/scripts/game/reward.vela";
    for (uri, text) in [(helper_uri, helper_text), (main_uri, main_text)] {
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
    }

    let rename = response_value(handle_request(
        &mut server,
        2,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": helper_uri },
            "position": {
                "line": 0,
                "character": helper_text.find("grant").expect("grant declaration")
            },
            "newName": "grant_reward"
        }),
    ));
    let main_edits = rename["result"]["changes"][main_uri]
        .as_array()
        .expect("rename should return main edits");
    let helper_edits = rename["result"]["changes"][helper_uri]
        .as_array()
        .expect("rename should return helper edits");

    assert_eq!(main_edits.len(), 1);
    assert_text_edit(main_edits, 0, 18, "grant_reward");
    assert_eq!(helper_edits.len(), 1);
    assert_text_edit(helper_edits, 0, 7, "grant_reward");
}

fn assert_text_edit(edits: &[serde_json::Value], line: usize, character: usize, new_text: &str) {
    assert!(
        edits.iter().any(|edit| {
            edit["range"]["start"]["line"] == line
                && edit["range"]["start"]["character"] == character
                && edit["newText"] == new_text
        }),
        "{edits:?}"
    );
}
