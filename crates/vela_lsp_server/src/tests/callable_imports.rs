use crate::matrix_fixture::{Edit, FixtureWorkspace, apply_edits, load};
use crate::tests::{TestServer, notify, request, response_value};
use lsp_types::{notification as n, request as r};
use serde_json::json;

#[test]
fn callable_import_matrix_projects_owned_signatures_parameters_and_utf16_edits() {
    verify_fixture("callable-imports");
}

#[test]
fn task_callable_matrix_projects_reserved_ownership_and_positional_operands() {
    verify_fixture("completion-task-calls");
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
        let temp = crate::tests::support::unique_temp_root("callable-imports");
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
            json!({"processId":null,"rootUri":uri(""),"capabilities":{}}),
        );
        for (file, source) in &fixture.disk {
            if file.ends_with(".vela") {
                let _ = notify::<n::DidOpenTextDocument>(
                    &mut server,
                    json!({"textDocument":{"uri":uri(file),"languageId":"vela","version":1,"text":source.text}}),
                );
            }
        }
        let mut id = 2;
        for case in spec.oracle["queries"].as_array().expect("queries") {
            let file = case["file"].as_str().expect("file");
            let source = fixture.document(file).expect("source");
            let point = source.markers["cursor"].start;
            let params = json!({"textDocument":{"uri":uri(file)},"position":{"line":point.line,"character":point.character}});
            let help = response_value(request::<r::SignatureHelpRequest>(
                &mut server,
                id,
                params.clone(),
            ));
            id += 1;
            assert!(help["error"].is_null());
            let repeat = response_value(request::<r::SignatureHelpRequest>(
                &mut server,
                id,
                params.clone(),
            ));
            id += 1;
            assert_eq!(help["result"], repeat["result"]);
            if case["signature"].is_null() {
                assert!(help["result"].is_null(), "{case}: {help}");
            } else {
                assert_eq!(
                    help["result"]["signatures"]
                        .as_array()
                        .expect("signatures")
                        .len(),
                    1,
                    "{case}"
                );
                assert_eq!(
                    help["result"]["signatures"][0]["label"], case["signature"],
                    "{case}"
                );
                assert_eq!(
                    help["result"]["activeParameter"],
                    case["activeParameter"].as_u64().unwrap_or(0)
                );
            }
            let completion =
                response_value(request::<r::Completion>(&mut server, id, params.clone()));
            id += 1;
            let repeat = response_value(request::<r::Completion>(&mut server, id, params));
            id += 1;
            assert!(completion["error"].is_null());
            assert_eq!(completion["result"], repeat["result"]);
            let items = completion["result"]["items"].as_array().expect("items");
            let expected = case["parameters"].as_array().expect("parameters");
            let mut labels = items
                .iter()
                .map(|i| i["label"].as_str().expect("label"))
                .collect::<Vec<_>>();
            labels.sort_unstable();
            let mut names = expected
                .iter()
                .map(|i| i["name"].as_str().expect("name"))
                .collect::<Vec<_>>();
            names.extend(crate::matrix_fixture::expected_expression_labels(
                &spec.oracle,
                case,
            ));
            names.sort_unstable();
            assert_eq!(labels, names, "{case}");
            let range = source.markers["replace"];
            let mut version = 1;
            for expected in expected {
                let item = items
                    .iter()
                    .find(|i| {
                        i["label"] == expected["name"]
                            && i["labelDetails"]["description"] == "named argument"
                    })
                    .expect("item");
                assert_eq!(item["kind"], 6);
                assert_eq!(item["detail"], expected["detail"]);
                assert_eq!(
                    item["textEdit"],
                    json!({"range":{"start":{"line":range.start.line,"character":range.start.character},"end":{"line":range.end.line,"character":range.end.character}},"newText":expected["insert"]})
                );
                let edit: lsp_types::TextEdit =
                    serde_json::from_value(item["textEdit"].clone()).expect("edit");
                let inserted = format!("{}1", edit.new_text);
                let edited = apply_edits(
                    &source.text,
                    &[Edit {
                        start: (
                            edit.range.start.line as usize,
                            edit.range.start.character as usize,
                        ),
                        end: (
                            edit.range.end.line as usize,
                            edit.range.end.character as usize,
                        ),
                        text: &inserted,
                    }],
                )
                .expect("apply");
                assert_eq!(
                    edited,
                    format!(
                        "{}{}1{}",
                        &source.text[..range.start.byte],
                        expected["insert"].as_str().expect("insert"),
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
                let again = response_value(request::<r::SignatureHelpRequest>(
                    &mut server,
                    id,
                    json!({"textDocument":{"uri":uri(file)},"position":{"line":range.start.line,"character":range.start.character + inserted.encode_utf16().count()}}),
                ));
                id += 1;
                assert_eq!(again["result"]["signatures"], help["result"]["signatures"]);
                version += 1;
                let _ = notify::<n::DidChangeTextDocument>(
                    &mut server,
                    json!({"textDocument":{"uri":uri(file),"version":version},"contentChanges":[{"text":source.text}]}),
                );
            }
        }
        std::fs::remove_dir_all(temp).expect("cleanup");
    }
}
