use crate::matrix_fixture::{Edit, FixtureWorkspace, apply_edits, load};
use crate::tests::{TestServer, notify, request, response_value};
use lsp_types::{notification as n, request as r};
use serde_json::json;

#[test]
fn call_expression_matrix_projects_complete_choices_and_applies_utf16_edits() {
    verify_fixture("completion-call-expressions");
}

#[test]
fn task_operand_matrix_projects_calls_and_static_continuation_paths() {
    verify_fixture("completion-task-operands");
}

fn verify_fixture(name: &str) {
    for crlf in [false, true] {
        let mut spec = load(name);
        if crlf {
            for text in spec.files.values_mut() {
                *text = text.replace('\n', "\r\n");
            }
        }
        let fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let temp = crate::tests::support::unique_temp_root("call-expressions");
        let root = temp.join("中文 % workspace");
        fixture.materialize(&root).expect("materialize");
        let uri = |file: &str| {
            lsp_types::Url::from_file_path(root.join(file))
                .expect("URI")
                .to_string()
        };
        let mut server = TestServer::new();
        let _ = request::<r::Initialize>(
            &mut server,
            1,
            json!({"processId":null,"rootUri":uri(""),"capabilities":{"textDocument":{"completion":{"completionItem":{"snippetSupport":true,"labelDetailsSupport":true}}}}}),
        );
        let mut id = 2;
        for case in spec.oracle["queries"].as_array().expect("queries") {
            let file = case["file"].as_str().expect("file");
            let source = fixture.document(file).expect("source");
            let point = source.markers["cursor"].start;
            let range = source.markers["replace"];
            let _ = notify::<n::DidOpenTextDocument>(
                &mut server,
                json!({"textDocument":{"uri":uri(file),"languageId":"vela","version":1,"text":source.text}}),
            );
            let params = json!({"textDocument":{"uri":uri(file)},"position":{"line":point.line,"character":point.character}});
            let response =
                response_value(request::<r::Completion>(&mut server, id, params.clone()));
            id += 1;
            assert!(response["error"].is_null(), "{case}: {response}");
            let repeat = response_value(request::<r::Completion>(&mut server, id, params.clone()));
            id += 1;
            assert_eq!(repeat["result"], response["result"]);
            let items = response["result"]["items"].as_array().expect("items");
            let expected = case["items"].as_array().expect("items");
            let mut actual_keys = items
                .iter()
                .map(|i| {
                    (
                        i["label"].as_str().expect("fixture string"),
                        i["textEdit"]["newText"].as_str().expect("fixture string"),
                    )
                })
                .collect::<Vec<_>>();
            let mut expected_keys = expected
                .iter()
                .map(|i| {
                    (
                        i["label"].as_str().expect("fixture string"),
                        i["insert"].as_str().expect("fixture string"),
                    )
                })
                .collect::<Vec<_>>();
            actual_keys.sort();
            expected_keys.sort();
            assert_eq!(actual_keys, expected_keys, "{case}");
            let mut version = 1;
            for expected in expected {
                let item = items
                    .iter()
                    .find(|i| {
                        i["label"] == expected["label"]
                            && i["textEdit"]["newText"] == expected["insert"]
                    })
                    .expect("item");
                assert_eq!(
                    item["kind"],
                    if expected["kind"] == "Function" { 3 } else { 6 },
                    "{case}"
                );
                assert_eq!(item["detail"], expected["detail"]);
                assert_eq!(
                    item["labelDetails"]["description"],
                    if expected["named"] == true {
                        json!("named argument")
                    } else {
                        json!(null)
                    }
                );
                assert_eq!(
                    item["textEdit"],
                    json!({"range":{"start":{"line":range.start.line,"character":range.start.character},"end":{"line":range.end.line,"character":range.end.character}},"newText":expected["insert"]})
                );
                let mut insertion = expected["insert"]
                    .as_str()
                    .expect("fixture string")
                    .replace("$0", "");
                if insertion.ends_with(" = ") {
                    insertion.push_str(case["value"].as_str().expect("value"));
                }
                let edited = apply_edits(
                    &source.text,
                    &[Edit {
                        start: (range.start.line, range.start.character),
                        end: (range.end.line, range.end.character),
                        text: &insertion,
                    }],
                )
                .expect("apply");
                assert_eq!(
                    edited,
                    format!(
                        "{}{}{}",
                        &source.text[..range.start.byte],
                        insertion,
                        &source.text[range.end.byte..]
                    )
                );
                let parsed = vela_syntax::parse::parse_source(&format!(
                    "{edited}{}",
                    case["recoverySuffix"].as_str().expect("suffix")
                ));
                assert!(
                    parsed.diagnostics().is_empty(),
                    "{case}: {:?}",
                    parsed.diagnostics()
                );
                version += 1;
                let _ = notify::<n::DidChangeTextDocument>(
                    &mut server,
                    json!({"textDocument":{"uri":uri(file),"version":version},"contentChanges":[{"text":edited}]}),
                );
                let again = response_value(request::<r::Completion>(
                    &mut server,
                    id,
                    json!({"textDocument":{"uri":uri(file)},"position":{"line":range.start.line,"character":range.start.character+insertion.encode_utf16().count()}}),
                ));
                id += 1;
                assert!(again["error"].is_null(), "{case}: {again}");
                assert!(again["result"]["items"].is_array());
                version += 1;
                let _ = notify::<n::DidChangeTextDocument>(
                    &mut server,
                    json!({"textDocument":{"uri":uri(file),"version":version},"contentChanges":[{"text":source.text}]}),
                );
                let restored =
                    response_value(request::<r::Completion>(&mut server, id, params.clone()));
                id += 1;
                assert_eq!(restored["result"], response["result"], "{case}");
            }
        }
        std::fs::remove_dir_all(temp).expect("cleanup");
    }
}
