use crate::matrix_fixture::{Edit, FixtureWorkspace, apply_edits, load};
use crate::tests::{TestServer, notify, request, response_value};
use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};

#[test]
fn type_ownership_matrix_projects_owned_edits_definitions_and_restored_candidates() {
    assert_type_ownership("completion-type-ownership");
}

#[test]
fn package_type_ownership_projects_exact_dependency_targets_and_edits() {
    assert_type_ownership("completion-package-type-ownership");
}

#[test]
fn package_callable_ownership_projects_sets_signatures_and_applied_targets() {
    assert_type_ownership("completion-package-callables");
}

#[test]
fn package_member_ownership_projects_sets_signatures_returns_and_targets() {
    assert_type_ownership("completion-package-members");
}

fn assert_type_ownership(fixture_id: &str) {
    for crlf in [false, true] {
        let mut spec = load(fixture_id);
        if crlf {
            for text in spec.files.values_mut() {
                *text = text.replace('\n', "\r\n");
            }
        }
        let fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let temp = crate::tests::support::unique_temp_root("import-aliases");
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
            json!({"processId":null,"rootUri":uri(""),"capabilities":{"textDocument":{"completion":{"completionItem":{"snippetSupport":true,"labelDetailsSupport":true,"resolveSupport":{"properties":["documentation"]}}}}}}),
        );
        let mut id = 2;
        for case in spec.oracle["queries"].as_array().expect("queries") {
            let file = string(case, "file");
            let source = fixture.document(file).expect("document");
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
            assert!(response["error"].is_null(), "{case} {response}");
            let items = response["result"]["items"].as_array().expect("items");
            let expected = case["items"].as_array().expect("items");
            let mut actual = items
                .iter()
                .map(|i| (string(i, "label"), string(&i["textEdit"], "newText")))
                .collect::<Vec<_>>();
            let mut keys = expected
                .iter()
                .map(|i| (string(i, "label"), string(i, "insert")))
                .collect::<Vec<_>>();
            actual.sort();
            keys.sort();
            assert_eq!(actual, keys, "{case}");
            let mut version = 1;
            for expected in expected {
                let item = items
                    .iter()
                    .find(|i| {
                        i["label"] == expected["label"]
                            && i["textEdit"]["newText"] == expected["insert"]
                    })
                    .expect("item");
                let kind = match string(expected, "kind") {
                    "Function" => 3,
                    "Method" => 2,
                    "Field" => 5,
                    "Const" => 21,
                    "Type" => 22,
                    "Trait" => 8,
                    "Binding" => 6,
                    "Module" => 9,
                    other => panic!("{other}"),
                };
                assert_eq!(item["kind"], kind);
                assert_eq!(item["detail"], expected["detail"]);
                let symbol_name = expected["protocolSymbol"]
                    .as_str()
                    .unwrap_or(string(expected, "symbol"));
                if expected["origin"] != "local" {
                    assert_eq!(
                        item["data"]["resolve"],
                        json!({"kind":"documentation","symbol":{"kind":expected["origin"],"name":symbol_name}}),
                        "{case}"
                    );
                    let resolved = response_value(request::<r::ResolveCompletionItem>(
                        &mut server,
                        id,
                        item.clone(),
                    ));
                    id += 1;
                    assert_eq!(resolved["result"]["textEdit"], item["textEdit"]);
                    assert_eq!(resolved["result"]["data"], item["data"]);
                    let mut expected_resolved = item.clone();
                    if let Some(docs) = expected["docs"].as_str() {
                        expected_resolved["documentation"] =
                            json!({"kind":"markdown","value":docs});
                    }
                    assert_eq!(resolved["result"], expected_resolved, "{case}");
                }
                assert_eq!(
                    item["textEdit"],
                    json!({"range":{"start":{"line":range.start.line,"character":range.start.character},"end":{"line":range.end.line,"character":range.end.character}},"newText":expected["insert"]})
                );
                let insertion = format!(
                    "{}{}",
                    string(expected, "insert")
                        .replace("$0", expected["value"].as_str().unwrap_or("1")),
                    string(case, "applySuffix")
                );
                let edited = apply_edits(
                    &source.text,
                    &[Edit {
                        start: (range.start.line, range.start.character),
                        end: (range.end.line, range.end.character),
                        text: &insertion,
                    }],
                )
                .expect("edit");
                assert_eq!(
                    edited,
                    format!(
                        "{}{}{}",
                        &source.text[..range.start.byte],
                        insertion,
                        &source.text[range.end.byte..]
                    )
                );
                assert!(
                    vela_syntax::parse::parse_source(&edited)
                        .diagnostics()
                        .is_empty()
                );
                version += 1;
                let _ = notify::<n::DidChangeTextDocument>(
                    &mut server,
                    json!({"textDocument":{"uri":uri(file),"version":version},"contentChanges":[{"text":edited}]}),
                );
                if let Some(signature) = expected["signature"].as_str() {
                    let open = insertion.find('(').expect("call");
                    let help = response_value(request::<r::SignatureHelpRequest>(
                        &mut server,
                        id,
                        json!({"textDocument":{"uri":uri(file)},"position":{"line":range.start.line,"character":range.start.character+open+1}}),
                    ));
                    id += 1;
                    assert!(help["error"].is_null());
                    assert_eq!(
                        help["result"]["signatures"]
                            .as_array()
                            .expect("signatures")
                            .len(),
                        1,
                        "{case}"
                    );
                    assert_eq!(
                        help["result"]["signatures"][0]["label"], signature,
                        "{case} {expected}"
                    );
                }
                if let Some(argument) = expected.get("argument") {
                    if let Some(label) = expected["inlay"].as_str() {
                        let character =
                            range.start.character + insertion.find('(').expect("call") + 1;
                        let point = json!({"line":range.start.line,"character":character});
                        let hints = response_value(request::<r::InlayHintRequest>(
                            &mut server,
                            id,
                            json!({"textDocument":{"uri":uri(file)},"range":{"start":point,"end":point}}),
                        ));
                        id += 1;
                        assert!(hints["error"].is_null());
                        let hints = hints["result"].as_array().expect("hints");
                        assert_eq!(hints.len(), 1, "{case}");
                        assert_eq!(hints[0]["label"], label, "{case}");
                        assert_eq!(hints[0]["kind"], 2);
                        assert_eq!(hints[0]["position"], point);
                    }
                    let text = source.text[..range.start.byte].to_owned()
                        + &string(expected, "insert").replace("$0", "zz_")
                        + &source.text[range.end.byte..];
                    let start = range.start.character
                        + string(expected, "insert").find('(').expect("call")
                        + 1;
                    version += 1;
                    let _ = notify::<n::DidChangeTextDocument>(
                        &mut server,
                        json!({"textDocument":{"uri":uri(file),"version":version},"contentChanges":[{"text":text}]}),
                    );
                    let response = response_value(request::<r::Completion>(
                        &mut server,
                        id,
                        json!({"textDocument":{"uri":uri(file)},"position":{"line":range.start.line,"character":start+3}}),
                    ));
                    id += 1;
                    assert!(response["error"].is_null());
                    let parameters = response["result"]["items"].as_array().expect("parameters");
                    assert_eq!(
                        parameters
                            .iter()
                            .map(|i| string(i, "label"))
                            .collect::<Vec<_>>(),
                        [string(argument, "label")],
                        "{case}"
                    );
                    assert_eq!(parameters[0]["detail"], argument["detail"]);
                    assert_eq!(parameters[0]["kind"], 6);
                    assert_eq!(
                        parameters[0]["textEdit"],
                        json!({"range":{"start":{"line":range.start.line,"character":start},"end":{"line":range.start.line,"character":start+3}},"newText":format!("{} = ",string(argument,"label"))})
                    );
                    version += 1;
                    let _ = notify::<n::DidChangeTextDocument>(
                        &mut server,
                        json!({"textDocument":{"uri":uri(file),"version":version},"contentChanges":[{"text":edited}]}),
                    );
                }
                if let Some(target) = expected["target"].as_str() {
                    let target = if target == "self" { file } else { target };
                    let marker = fixture.document(target).expect("target").markers
                        [string(expected, "marker")];
                    let start = insertion.rfind("::").map_or(0, |i| i + 2);
                    let definition = response_value(request::<r::GotoDefinition>(
                        &mut server,
                        id,
                        json!({"textDocument":{"uri":uri(file)},"position":{"line":range.start.line,"character":range.start.character+start+1}}),
                    ));
                    id += 1;
                    assert_eq!(
                        definition["result"],
                        json!({"uri":uri(target),"range":{"start":{"line":marker.start.line,"character":marker.start.character},"end":{"line":marker.end.line,"character":marker.end.character}}}),
                        "{case} {expected}"
                    );
                }
                if case["member"] == true {
                    let member = source.markers["member"].start;
                    let response = response_value(request::<r::Completion>(
                        &mut server,
                        id,
                        json!({"textDocument":{"uri":uri(file)},"position":{"line":member.line,"character":member.character+insertion.encode_utf16().count()-(range.end.character-range.start.character)}}),
                    ));
                    id += 1;
                    let members = response["result"]["items"].as_array().expect("members");
                    assert_eq!(
                        members
                            .iter()
                            .map(|i| string(i, "label"))
                            .collect::<Vec<_>>(),
                        ["tag"],
                        "{case}"
                    );
                    assert_eq!(
                        members[0]["data"]["resolve"]["symbol"],
                        json!({"kind":expected["origin"],"name":format!("{}.tag",string(expected,"symbol"))})
                    );
                    if let Some(detail) = expected["fieldDetail"].as_str() {
                        assert_eq!(members[0]["detail"], detail, "{case} {expected}");
                    }
                }
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
        for (index, step) in spec.oracle["lifecycle"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default()
            .iter()
            .enumerate()
        {
            let text = string(step, "source").replace('\n', if crlf { "\r\n" } else { "\n" });
            let source = crate::matrix_fixture::parse_markers(&text).expect("source");
            let point = source.markers["cursor"].start;
            let file = "scripts/source_alias.vela";
            let _ = notify::<n::DidChangeTextDocument>(
                &mut server,
                json!({"textDocument":{"uri":uri(file),"version":100+index},"contentChanges":[{"text":source.text}]}),
            );
            let response = response_value(request::<r::Completion>(
                &mut server,
                id,
                json!({"textDocument":{"uri":uri(file)},"position":{"line":point.line,"character":point.character}}),
            ));
            id += 1;
            let items = response["result"]["items"].as_array().expect("items");
            let expected = step["items"].as_array().expect("items");
            assert_eq!(items.len(), expected.len(), "{step}");
            for (item, expected) in items.iter().zip(expected) {
                assert_eq!(item["label"], expected["label"]);
                assert_eq!(item["detail"], expected["detail"]);
                assert_eq!(
                    item["data"]["resolve"]["symbol"],
                    json!({"kind":"source","name":expected["symbol"]})
                );
                assert_eq!(item["textEdit"]["newText"], expected["insert"]);
            }
        }
        std::fs::remove_dir_all(temp).expect("cleanup");
    }
}
fn string<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().expect("fixture string")
}
