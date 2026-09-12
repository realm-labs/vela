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

#[test]
fn task_eligibility_matrix_projects_static_targets_and_matching_continuations() {
    verify_fixture("completion-task-eligibility");
}

#[test]
fn sync_callback_matrix_projects_owned_function_values_and_excludes_async_targets() {
    verify_fixture("completion-sync-callbacks");
}

#[test]
fn task_value_matrix_projects_argument_choices_and_exact_admission_diagnostics() {
    verify_fixture("completion-task-values");
}

#[test]
fn task_effect_matrix_projects_choices_and_transitive_denials() {
    verify_fixture("completion-task-effects");
    verify_fixture("completion-task-spawn-ceiling");
    verify_fixture("completion-task-unknown-ceiling");
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
                if expected["symbol"].is_string() {
                    assert_eq!(
                        item["data"]["resolve"],
                        json!({"kind":"documentation","symbol":{"kind":expected["origin"].as_str().unwrap_or("source"),"name":expected["symbol"]}})
                    );
                    let resolved = response_value(request::<r::ResolveCompletionItem>(
                        &mut server,
                        id,
                        item.clone(),
                    ));
                    id += 1;
                    assert!(resolved["error"].is_null());
                    assert_eq!(resolved["result"], *item);
                }
                assert_eq!(
                    item["labelDetails"]["description"],
                    if expected["description"].is_string() {
                        expected["description"].clone()
                    } else if expected["named"] == true {
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
                let changes = notify::<n::DidChangeTextDocument>(
                    &mut server,
                    json!({"textDocument":{"uri":uri(file),"version":version},"contentChanges":[{"text":edited}]}),
                );
                if let Some(applied_file) = expected["appliedFile"].as_str() {
                    let applied = fixture.document(applied_file).expect("applied oracle");
                    assert_eq!(edited, applied.text, "{case}");
                    let notifications = crate::tests::notification_values(changes.clone());
                    let publication = notifications
                        .iter()
                        .find(|n| {
                            n["method"] == "textDocument/publishDiagnostics"
                                && n["params"]["uri"] == uri(file)
                        })
                        .expect("applied diagnostics");
                    let actual = publication["params"]["diagnostics"]
                        .as_array()
                        .expect("diagnostics")
                        .iter()
                        .filter(|d| {
                            d["code"].as_str().is_some_and(|code| {
                                code.starts_with("analysis::task_")
                                    || code.starts_with("hir::task_")
                                    || code == "analysis::async_call_requires_await"
                            })
                        })
                        .collect::<Vec<_>>();
                    let oracle = expected["diagnostics"].as_array().expect("diagnostics");
                    assert_eq!(actual.len(), oracle.len(), "{case}: {actual:?}");
                    for (actual, expected) in actual.iter().zip(oracle) {
                        assert_eq!(actual["code"], expected["code"]);
                        assert_eq!(actual["message"], expected["message"]);
                        assert_eq!(actual["severity"], 1);
                        let range = applied.markers[expected["marker"].as_str().expect("marker")];
                        assert_eq!(
                            actual["range"],
                            json!({"start":{"line":range.start.line,"character":range.start.character},"end":{"line":range.end.line,"character":range.end.character}})
                        );
                        assert_eq!(
                            actual["data"]["labels"],
                            json!([{"uri":uri(file),"range":actual["range"],"message":expected["label"]}])
                        );
                    }
                }
                if case["checkTask"] == true {
                    if expected.get("target").is_some() {
                        let column = range.start.character
                            + insertion
                                .rfind("::")
                                .map_or(0, |i| insertion[..i + 2].encode_utf16().count())
                            + 1;
                        let definition = response_value(request::<r::GotoDefinition>(
                            &mut server,
                            id,
                            json!({"textDocument":{"uri":uri(file)},"position":{"line":range.start.line,"character":column}}),
                        ));
                        id += 1;
                        assert!(definition["error"].is_null());
                        if let Some(target) = expected["target"].as_str() {
                            let target_source = fixture.document(target).expect("target");
                            let target_range = target_source.markers
                                [expected["targetMarker"].as_str().expect("target marker")];
                            assert_eq!(
                                definition["result"],
                                json!({"uri":uri(target),"range":{"start":{"line":target_range.start.line,"character":target_range.start.character},"end":{"line":target_range.end.line,"character":target_range.end.character}}}),
                                "{case} {expected}"
                            );
                        } else {
                            assert!(
                                definition["result"].is_null(),
                                "schema callback has no source definition"
                            );
                        }
                    }
                    let notifications = crate::tests::notification_values(changes);
                    let diagnostics = notifications
                        .iter()
                        .find(|n| {
                            n["method"] == "textDocument/publishDiagnostics"
                                && n["params"]["uri"] == uri(file)
                        })
                        .expect("applied diagnostics");
                    let codes = diagnostics["params"]["diagnostics"]
                        .as_array()
                        .expect("diagnostics")
                        .iter()
                        .filter_map(|d| d["code"].as_str())
                        .filter(|code| {
                            code.starts_with("hir::task_")
                                || code.starts_with("analysis::task_")
                                || *code == "analysis::async_call_requires_await"
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(
                        codes,
                        expected["taskDiagnostic"]
                            .as_str()
                            .into_iter()
                            .collect::<Vec<_>>(),
                        "{case} {expected}"
                    );
                    if let Some(code) = expected["taskDiagnostic"].as_str() {
                        let diagnostic = diagnostics["params"]["diagnostics"]
                            .as_array()
                            .expect("diagnostics")
                            .iter()
                            .find(|d| d["code"] == code)
                            .expect("task diagnostic");
                        let marked = source.markers["diagnostic"];
                        let shift = insertion.encode_utf16().count() as isize
                            - (range.end.character - range.start.character) as isize;
                        let start = marked
                            .start
                            .character
                            .checked_add_signed(shift)
                            .expect("start");
                        let end = marked.end.character.checked_add_signed(shift).expect("end");
                        assert_eq!(
                            diagnostic["range"],
                            json!({"start":{"line":marked.start.line,"character":start},"end":{"line":marked.end.line,"character":end}})
                        );
                        assert_eq!(diagnostic["severity"], 1);
                    }
                }
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
