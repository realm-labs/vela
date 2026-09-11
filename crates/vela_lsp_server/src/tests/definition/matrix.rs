use lsp_types::{notification as n, request as r};
use serde_json::json;

use crate::matrix_fixture::{FixtureWorkspace, load};
use crate::tests::{TestServer, navigation_request, notify, request, response_value};

#[test]
fn navigation_declaration_matrix_projects_exact_utf16_targets_and_nulls() {
    assert_navigation_matrix("navigation-declarations");
}

#[test]
fn navigation_member_matrix_projects_exact_source_member_targets_and_nulls() {
    assert_navigation_matrix("navigation-members");
}

fn assert_navigation_matrix(fixture_id: &str) {
    for crlf in [false, true] {
        let mut spec = load(fixture_id);
        if crlf {
            for source in spec.files.values_mut() {
                *source = source.replace('\n', "\r\n");
            }
        }
        let fixture = FixtureWorkspace::new(&spec).expect("navigation fixture");
        let temp = crate::tests::support::unique_temp_root("navigation-matrix");
        let root = temp.join("中文 workspace");
        fixture.materialize(&root).expect("isolated workspace");
        let uri = |file: &str| {
            lsp_types::Url::from_file_path(root.join(file))
                .expect("URI")
                .to_string()
        };
        let mut server = TestServer::new();
        let _ = request::<r::Initialize>(
            &mut server,
            1,
            json!({
                "processId":null,"rootUri":uri("scripts"),"capabilities":{}
            }),
        );
        for (file, document) in &fixture.disk {
            let _ = notify::<n::DidOpenTextDocument>(
                &mut server,
                json!({
                    "textDocument":{"uri":uri(file),"languageId":"vela","version":1,"text":document.text}
                }),
            );
        }
        let mut id = 2;
        for query in spec.oracle["queries"].as_array().expect("matrix") {
            let file = query["file"].as_str().expect("file");
            let document = fixture.document(file).expect("document");
            let point = document.markers[query["cursor"].as_str().expect("cursor")].start;
            for (key, method) in [
                ("definition", "textDocument/definition"),
                ("declaration", "textDocument/declaration"),
                ("type-definition", "textDocument/typeDefinition"),
            ] {
                let response = response_value(navigation_request(
                    &mut server,
                    id,
                    method,
                    json!({
                        "textDocument":{"uri":uri(file)},"position":{"line":point.line,"character":point.character}
                    }),
                ));
                id += 1;
                let expected = query.get(key).expect("explicit method oracle");
                let result = if expected.is_null() {
                    serde_json::Value::Null
                } else {
                    let target_file = query["target-file"].as_str().unwrap_or(file);
                    let target = fixture.document(target_file).expect("target").markers
                        [expected.as_str().expect("target marker or explicit null")];
                    json!({"uri":uri(target_file),"range":{
                        "start":{"line":target.start.line,"character":target.start.character},
                        "end":{"line":target.end.line,"character":target.end.character}
                    }})
                };
                assert!(response.get("error").is_none(), "{response}");
                assert_eq!(
                    response.get("result"),
                    Some(&result),
                    "{}: {method}, CRLF={crlf}",
                    query["id"]
                );
            }
        }
        std::fs::remove_dir_all(temp).expect("fixture cleanup");
    }
}
