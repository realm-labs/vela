use crate::matrix_fixture::{Edit, FixtureWorkspace, apply_edits, load};
use crate::tests::{TestServer, notify, request, response_value};
use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};

#[test]
fn enum_alias_matrix_projects_owned_edits_definitions_and_restored_candidates() {
    assert_enum_alias("completion-enum-aliases");
}

#[test]
fn enum_alias_overlay_transitions_replace_field_owner_metadata() {
    let spec = load("completion-enum-aliases");
    let fixture = FixtureWorkspace::new(&spec).expect("enum fixture invariant");
    let temp = crate::tests::support::unique_temp_root("enum-alias-lifecycle");
    let root = temp.join("中文 % workspace");
    fixture.materialize(&root).expect("enum fixture invariant");
    let uri = |file: &str| {
        lsp_types::Url::from_file_path(root.join(file))
            .expect("enum fixture invariant")
            .to_string()
    };
    let mut server = TestServer::new();
    let _ = request::<r::Initialize>(
        &mut server,
        1,
        json!({"processId":null,"rootUri":uri(""),"capabilities":{}}),
    );
    let lifecycle = &spec.oracle["lifecycle"];
    let file = string(lifecycle, "file");
    let original = fixture.document(file).expect("enum fixture invariant");
    let cursor = original.markers["cursor"].start;
    let suffix = original
        .text
        .split_once('\n')
        .expect("enum fixture invariant")
        .1;
    let _ = notify::<n::DidOpenTextDocument>(
        &mut server,
        json!({"textDocument":{"uri":uri(file),"languageId":"vela","version":1,"text":original.text}}),
    );
    for (index, state) in lifecycle["states"]
        .as_array()
        .expect("enum fixture invariant")
        .iter()
        .enumerate()
    {
        let text = format!("{}\n{suffix}", string(state, "imports"));
        let _ = notify::<n::DidChangeTextDocument>(
            &mut server,
            json!({"textDocument":{"uri":uri(file),"version":index+2},"contentChanges":[{"text":text}]}),
        );
        let response = response_value(request::<r::Completion>(
            &mut server,
            i32::try_from(index).expect("enum fixture invariant") + 2,
            json!({"textDocument":{"uri":uri(file)},"position":{"line":cursor.line,"character":cursor.character}}),
        ));
        let items = response["result"]["items"]
            .as_array()
            .expect("enum fixture invariant");
        if state["detail"].is_null() {
            assert!(items.is_empty(), "{state}");
        } else {
            assert_eq!(items.len(), 1, "{state}");
            assert_eq!(items[0]["label"], "tag");
            assert_eq!(items[0]["detail"], state["detail"]);
            assert_eq!(
                items[0]["data"]["resolve"]["symbol"],
                json!({"kind":state["origin"],"name":state["symbol"]})
            );
        }
    }
    std::fs::remove_dir_all(temp).expect("enum fixture invariant");
}

fn assert_enum_alias(fixture_id: &str) {
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
                    "Const" => 21,
                    "Type" => 22,
                    "Variant" => 20,
                    "Field" => 5,
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
                    string(expected, "insert").replace("$0", "1"),
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
                let start = insertion.rfind("::").map_or(0, |i| i + 2);
                let definition = response_value(request::<r::GotoDefinition>(
                    &mut server,
                    id,
                    json!({"textDocument":{"uri":uri(file)},"position":{"line":range.start.line,"character":range.start.character+start+1}}),
                ));
                id += 1;
                if let Some(target) = expected["target"].as_str() {
                    let target = if target == "self" { file } else { target };
                    let marker = fixture.document(target).expect("target").markers
                        [string(expected, "marker")];
                    assert_eq!(
                        definition["result"],
                        json!({"uri":uri(target),"range":{"start":{"line":marker.start.line,"character":marker.start.character},"end":{"line":marker.end.line,"character":marker.end.character}}}),
                        "{case} {expected}"
                    );
                } else {
                    assert!(definition["result"].is_null(), "{case} {definition}");
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
                        expected["members"]
                            .as_array()
                            .expect("members")
                            .iter()
                            .map(|i| string(i, "label"))
                            .collect::<Vec<_>>(),
                        "{case}"
                    );
                    for (item, wanted) in members
                        .iter()
                        .zip(expected["members"].as_array().expect("members"))
                    {
                        assert_eq!(item["detail"], wanted["detail"], "{case}");
                        assert_eq!(
                            item["data"]["resolve"]["symbol"],
                            json!({"kind":expected["origin"],"name":format!("{}.{}",string(expected,"receiver"),string(wanted,"label"))})
                        );
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
        std::fs::remove_dir_all(temp).expect("cleanup");
    }
}
fn string<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().expect("fixture string")
}
