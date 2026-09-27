use crate::matrix_fixture::{FixtureWorkspace, hover_signature as oracle, load};
use crate::tests::{TestServer, notify, request, response_value, sync_diagnostics};
use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};
use std::{collections::BTreeSet, fs, path::Path};

fn uri(root: &Path, file: &str) -> String {
    lsp_types::Url::from_file_path(root.join(file))
        .expect("encoded URI")
        .to_string()
}

fn hover(server: &mut TestServer, uri: &str, position: &Value) -> Value {
    let response = response_value(request::<r::HoverRequest>(
        server,
        2,
        json!({"textDocument":{"uri":uri},"position":position}),
    ));
    assert!(response.get("error").is_none(), "{response}");
    assert!(response.get("result").is_some(), "explicit JSON null");
    response["result"].clone()
}

pub(super) fn verify_fixture(name: &str, expected_queries: usize, expected_positions: usize) {
    let mut positions = 0;
    for crlf in [false, true] {
        for missing_schema in [false, true] {
            let mut spec = load(name);
            if crlf {
                for source in spec.files.values_mut() {
                    *source = source.replace('\n', "\r\n");
                }
            }
            if missing_schema {
                spec.files.remove("schema.json");
            }
            let fixture = FixtureWorkspace::new(&spec).expect("fixture");
            let parent = crate::tests::support::unique_temp_root(name);
            let root = parent.join("中文 % hover");
            fixture.materialize(&root).expect("isolated files");
            let mut server = TestServer::new();
            let initialize = response_value(request::<r::Initialize>(
                &mut server,
                1,
                json!({"processId":null,"rootUri":uri(&root,""),"capabilities":{}}),
            ));
            assert!(initialize.get("error").is_none());
            assert_eq!(initialize["result"]["capabilities"]["hoverProvider"], true);
            notify::<n::Initialized>(&mut server, json!({}));
            let queries = spec.oracle["queries"].as_array().expect("queries");
            assert_eq!(queries.len(), expected_queries);
            let mut opened = BTreeSet::new();
            for authored in queries {
                let mut case = authored.clone();
                if missing_schema && case.get("missingResult").is_some() {
                    case["result"] = case["missingResult"].clone();
                }
                let file = case["file"].as_str().expect("file");
                let document = &fixture.disk[file];
                let target = uri(&root, file);
                if opened.insert(file.to_owned()) {
                    let _ = sync_diagnostics::<n::DidOpenTextDocument>(
                        &mut server,
                        json!({"textDocument":{"uri":target,"languageId":"vela","version":1,"text":document.text}}),
                    );
                }
                for offset in 0..if case["result"].is_null() { 1 } else { 2 } {
                    positions += 1;
                    let point = oracle::position(
                        document,
                        case["marker"].as_str().expect("marker"),
                        true,
                        offset,
                    );
                    let actual = hover(&mut server, &target, &point);
                    assert_eq!(
                        actual,
                        oracle::hover_result(document, &case, true),
                        "{}, missing={missing_schema}, CRLF={crlf}",
                        case["id"]
                    );
                    assert_eq!(hover(&mut server, &target, &point), actual, "repeat hover");
                }
                assert_eq!(
                    fs::read_to_string(root.join(file)).expect("disk bytes"),
                    document.text
                );
            }
            for (file, document) in &fixture.disk {
                assert_eq!(
                    fs::read_to_string(root.join(file)).expect("all physical inputs"),
                    document.text
                );
            }
            fs::remove_dir_all(parent).expect("remove owned isolated workspace");
        }
    }
    assert_eq!(
        positions, expected_positions,
        "every authored token position and schema variant"
    );
}
