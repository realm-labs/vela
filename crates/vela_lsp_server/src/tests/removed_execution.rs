use crate::matrix_fixture::{FixtureWorkspace, load};
use crate::tests::{TestServer, notification_values, notify, request, response_value};
use lsp_types::{notification as n, request as r};
use serde_json::json;

pub(super) fn verify_fixture(name: &str) {
    for crlf in [false, true] {
        let mut spec = load(name);
        assert_eq!(spec.oracle["removedSurface"], true);
        if crlf {
            for text in spec.files.values_mut() {
                *text = text.replace('\n', "\r\n");
            }
        }
        let fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let owner = crate::tests::support::unique_temp_root("removed-execution");
        let root = owner.join("workspace");
        fixture.materialize(&root).expect("materialize");
        let uri = |file: &str| {
            lsp_types::Url::from_file_path(root.join(file))
                .expect("URI")
                .to_string()
        };
        let mut server = TestServer::new();
        let initialized = response_value(request::<r::Initialize>(
            &mut server,
            1,
            json!({"processId":null,"rootUri":uri(""),"capabilities":{}}),
        ));
        assert!(initialized["error"].is_null());
        let mut id = 2;
        for case in spec.oracle["cases"].as_array().expect("cases") {
            let file = case["file"].as_str().expect("file");
            let source = fixture.document(file).expect("source");
            let messages = notification_values(notify::<n::DidOpenTextDocument>(
                &mut server,
                json!({"textDocument":{"uri":uri(file),"languageId":"vela","version":1,"text":source.text}}),
            ));
            let publication = messages
                .iter()
                .find(|m| {
                    m["method"] == "textDocument/publishDiagnostics"
                        && m["params"]["uri"] == uri(file)
                })
                .expect("diagnostics");
            let code = if case["syntaxError"] == true {
                "syntax::removed_async"
            } else {
                "syntax::removed_task"
            };
            assert!(
                publication["params"]["diagnostics"]
                    .as_array()
                    .expect("diagnostics")
                    .iter()
                    .any(|d| d["severity"] == 1 && d["code"] == code),
                "{name}: {case}: {publication}"
            );
            if case["signatureNull"] == true {
                let point = source.markers["cursor"].start;
                let result = response_value(request::<r::SignatureHelpRequest>(
                    &mut server,
                    id,
                    json!({"textDocument":{"uri":uri(file)},"position":{"line":point.line,"character":point.character}}),
                ));
                id += 1;
                assert!(result["error"].is_null(), "{result}");
                assert!(
                    result["result"].is_null(),
                    "removed task signature: {result}"
                );
                let point = source.markers["removed"].start;
                let result = response_value(request::<r::HoverRequest>(
                    &mut server,
                    id,
                    json!({"textDocument":{"uri":uri(file)},"position":{"line":point.line,"character":point.character}}),
                ));
                id += 1;
                assert!(result["error"].is_null(), "{result}");
                assert!(result["result"].is_null(), "removed task hover: {result}");
                let result = response_value(request::<r::Completion>(
                    &mut server,
                    id,
                    json!({"textDocument":{"uri":uri(file)},"position":{"line":point.line,"character":point.character}}),
                ));
                id += 1;
                assert!(result["error"].is_null(), "{result}");
                let items = result["result"]["items"]
                    .as_array()
                    .expect("completion items");
                assert!(
                    items.iter().all(|item| !matches!(
                        item["label"].as_str(),
                        Some("spawn_scoped" | "spawn_scoped_then")
                    )),
                    "removed task completions: {result}"
                );
            }
        }
        std::fs::remove_dir_all(owner).expect("cleanup");
    }
}
