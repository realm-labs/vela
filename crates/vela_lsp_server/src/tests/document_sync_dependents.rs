use super::{TestServer, notification_values, notify, request};
use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};

#[test]
fn source_sync_publishes_affected_importers_without_republishing_unrelated_files() {
    let root = super::support::unique_temp_root("sync-dependent-diagnostics");
    std::fs::create_dir_all(root.join("scripts")).expect("workspace");
    std::fs::write(
        root.join("vela.toml"),
        "[package]\nid = \"dev.vela.sync_diagnostics\"\nname = \"sync_diagnostics\"\nversion = \"0.1.0\"\n[source]\nroots = [\"scripts\"]\n",
    )
    .expect("config");
    let valid = "pub fn make() -> i64 { 1 }\n";
    std::fs::write(root.join("scripts/helper.vela"), valid).expect("disk source");
    let main_text = "use helper::make; fn main() { make(); }";
    std::fs::write(root.join("scripts/main.vela"), main_text).expect("importer");
    std::fs::write(root.join("scripts/unrelated.vela"), "fn unrelated() {}")
        .expect("unrelated source");
    let uri = |file: &str| {
        lsp_types::Url::from_file_path(root.join(file))
            .expect("URI")
            .to_string()
    };
    let mut server = TestServer::new();
    let _ = request::<r::Initialize>(
        &mut server,
        1,
        json!({"processId":null,"rootUri":uri(""),"capabilities":{}}),
    );
    let main = uri("scripts/main.vela");
    let helper = uri("scripts/helper.vela");
    let unrelated = uri("scripts/unrelated.vela");
    for (document, text) in [(&main, main_text), (&unrelated, "fn unrelated() {}")] {
        let _ = notify::<n::DidOpenTextDocument>(
            &mut server,
            json!({"textDocument":{"uri":document,"languageId":"vela","version":1,"text":text}}),
        );
    }
    let messages = notification_values(notify::<n::DidOpenTextDocument>(
        &mut server,
        json!({"textDocument":{"uri":helper,"languageId":"vela","version":1,"text":""}}),
    ));
    assert_publications(&messages, &main, &helper, &unrelated, true);
    for (version, text, missing) in [(2, valid, false), (3, "", true)] {
        let messages = notification_values(notify::<n::DidChangeTextDocument>(
            &mut server,
            json!({"textDocument":{"uri":helper,"version":version},"contentChanges":[{"text":text}]}),
        ));
        assert_publications(&messages, &main, &helper, &unrelated, missing);
    }
    let messages = notification_values(notify::<n::DidCloseTextDocument>(
        &mut server,
        json!({"textDocument":{"uri":helper}}),
    ));
    assert_publications(&messages, &main, &helper, &unrelated, false);
    std::fs::remove_dir_all(root).expect("cleanup fixture");
}

fn assert_publications(
    messages: &[Value],
    main: &str,
    helper: &str,
    unrelated: &str,
    missing: bool,
) {
    let mut documents = messages
        .iter()
        .map(|message| {
            assert_eq!(message["method"], "textDocument/publishDiagnostics");
            message["params"]["uri"].as_str().expect("document")
        })
        .collect::<Vec<_>>();
    documents.sort();
    let mut expected = vec![main, helper];
    expected.sort();
    assert_eq!(documents, expected);
    assert!(!documents.contains(&unrelated));
    let diagnostics = messages
        .iter()
        .find(|message| message["params"]["uri"] == main)
        .expect("importer publication")["params"]["diagnostics"]
        .as_array()
        .expect("diagnostics");
    assert_eq!(diagnostics.len(), usize::from(missing), "{diagnostics:?}");
    if missing {
        assert_eq!(diagnostics[0]["code"], "hir::unresolved_import");
        assert_eq!(diagnostics[0]["severity"], 1);
        assert_eq!(
            diagnostics[0]["message"],
            "unresolved import `make` in module `helper`"
        );
        assert_eq!(
            diagnostics[0]["range"],
            json!({"start":{"line":0,"character":0},"end":{"line":0,"character":17}})
        );
        assert_eq!(
            diagnostics[0]["data"]["labels"],
            json!([{"uri":main,"range":diagnostics[0]["range"],"message":"no similar declarations found"}])
        );
    }
}
