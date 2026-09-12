use crate::matrix_fixture::{Edit, FixtureWorkspace, apply_edits, load, schema_artifact};
use crate::tests::{TestServer, notify, request, response_value};
use lsp_types::{notification as n, request as r};
use serde_json::json;

#[test]
fn callable_return_matrix_projects_nested_owner_members_and_resolved_docs() {
    assert_member_matrix("completion-callable-returns");
}

#[test]
fn callable_return_schema_lifecycle_projects_refreshed_nested_facts_and_docs() {
    let spec = load("completion-callable-returns");
    let fixture = FixtureWorkspace::new(&spec).expect("fixture");
    let temp = crate::tests::support::unique_temp_root("callable-return-lifecycle");
    let root = temp.join("中文 % workspace");
    fixture.materialize(&root).expect("workspace");
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
    let file = "scripts/function.vela";
    let source = fixture.document(file).expect("source");
    let point = source.markers["cursor"].start;
    let call_line = source.text.lines().nth(2).expect("call line");
    let call_column = call_line.find("accept(rows)").expect("call") + 7;
    let mut id = 2;
    for step in spec.oracle["lifecycle"].as_array().expect("states") {
        let text = if step["mode"] == "invalid" {
            "{".to_owned()
        } else {
            json!({"formatVersion":1,"facts":step["schema"]}).to_string()
        };
        std::fs::write(root.join("schema.json"), text).expect("schema");
        let _ = notify::<n::DidChangeWatchedFiles>(
            &mut server,
            json!({"changes":[{"uri":uri("schema.json"),"type":2}]}),
        );
        let response = response_value(request::<r::Completion>(
            &mut server,
            id,
            json!({"textDocument":{"uri":uri(file)},"position":{"line":point.line,"character":point.character}}),
        ));
        id += 1;
        let mut actual = Vec::new();
        for item in response["result"]["items"].as_array().expect("items") {
            let resolved = response_value(request::<r::ResolveCompletionItem>(
                &mut server,
                id,
                item.clone(),
            ));
            id += 1;
            assert_eq!(resolved["result"]["data"], item["data"]);
            actual.push(json!({"label":item["label"],"detail":item["detail"],"docs":resolved["result"]["documentation"]["value"]}));
        }
        assert_eq!(json!(actual), step["items"], "{step}");
        let signature = response_value(request::<r::SignatureHelpRequest>(
            &mut server,
            id,
            json!({"textDocument":{"uri":uri(file)},"position":{"line":2,"character":call_column}}),
        ));
        id += 1;
        assert_eq!(
            signature["result"]["signatures"][0]["label"],
            step["signature"]
        );
    }
    std::fs::remove_dir_all(temp).expect("cleanup");
}

#[test]
fn member_matrix_projects_exact_sets_resolve_payloads_and_applied_utf16_edits() {
    assert_member_matrix("completion-members");
}

#[test]
fn enum_matrix_projects_exact_variant_and_constructor_completion_edits() {
    assert_member_matrix("completion-enums");
}

fn assert_member_matrix(fixture_id: &str) {
    for crlf in [false, true] {
        let mut spec = load(fixture_id);
        if crlf {
            for source in spec.files.values_mut() {
                *source = source.replace('\n', "\r\n");
            }
        }
        let fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let temp = crate::tests::support::unique_temp_root("completion-type-matrix");
        let root = temp.join("中文 % workspace");
        fixture.materialize(&root).expect("workspace");
        let uri = |file: &str| {
            lsp_types::Url::from_file_path(root.join(file))
                .expect("URI")
                .to_string()
        };
        let mut server = TestServer::new();
        let initialized = response_value(request::<r::Initialize>(
            &mut server,
            1,
            json!({
                "processId":null,"rootUri":uri(""),"capabilities":{"textDocument":{"completion":{"completionItem":{
                    "resolveSupport":{"properties":["documentation"]},"labelDetailsSupport":true
                }}}}
            }),
        ));
        assert_eq!(
            initialized["result"]["capabilities"]["completionProvider"]["resolveProvider"],
            true
        );
        let schema = schema_artifact(&spec.oracle["schema"], &fixture, |_| {
            panic!("metadata-only schema")
        });
        std::fs::write(root.join("schema.json"), schema.to_string()).expect("schema");
        let _ = notify::<n::DidChangeWatchedFiles>(
            &mut server,
            json!({"changes":[{"uri":uri("schema.json"),"type":1}]}),
        );
        for (file, source) in &fixture.disk {
            if file.ends_with(".vela") {
                let _ = notify::<n::DidOpenTextDocument>(
                    &mut server,
                    json!({"textDocument":{
                        "uri":uri(file),"languageId":"vela","version":1,"text":source.text
                    }}),
                );
            }
        }
        let mut id = 2;
        for query in spec.oracle["queries"].as_array().expect("queries") {
            let file = query["file"].as_str().expect("file");
            let source = fixture.document(file).expect("source");
            let point = source.markers["cursor"].start;
            let response = response_value(request::<r::Completion>(
                &mut server,
                id,
                json!({
                    "textDocument":{"uri":uri(file)},"position":{"line":point.line,"character":point.character}
                }),
            ));
            id += 1;
            assert_eq!(response["result"]["isIncomplete"], false);
            let items = response["result"]["items"].as_array().expect("items");
            let repeated = response_value(request::<r::Completion>(
                &mut server,
                id,
                json!({
                    "textDocument":{"uri":uri(file)},"position":{"line":point.line,"character":point.character}
                }),
            ));
            id += 1;
            assert_eq!(repeated["result"], response["result"]);
            let mut actual = items
                .iter()
                .map(|item| item["label"].as_str().expect("label"))
                .collect::<Vec<_>>();
            let mut expected = query["items"]
                .as_array()
                .expect("items")
                .iter()
                .map(|item| item["label"].as_str().expect("label"))
                .collect::<Vec<_>>();
            actual.sort_unstable();
            expected.sort_unstable();
            assert_eq!(actual, expected, "{}", query["id"]);
            let range = source.markers["replace"];
            for expected in query["items"].as_array().expect("items") {
                let item = items
                    .iter()
                    .find(|item| item["label"] == expected["label"])
                    .expect("item");
                assert_eq!(item["kind"], expected["lspKind"]);
                assert_eq!(item["detail"], expected["detail"]);
                assert_eq!(item["filterText"], expected["label"]);
                assert_eq!(
                    item["data"]["resolve"],
                    json!({"kind":"documentation","symbol":{"kind":expected["symbolKind"],"name":expected["symbol"]}})
                );
                assert_eq!(
                    item["textEdit"],
                    json!({"range":{"start":{"line":range.start.line,"character":range.start.character},"end":{"line":range.end.line,"character":range.end.character}},"newText":expected["insert"]})
                );
                assert_eq!(
                    item["insertTextFormat"],
                    if expected["kind"] == "Method" {
                        json!(2)
                    } else {
                        serde_json::Value::Null
                    }
                );
                assert!(item.get("documentation").is_none());
                let resolved = response_value(request::<r::ResolveCompletionItem>(
                    &mut server,
                    id,
                    item.clone(),
                ));
                id += 1;
                let mut resolved_expected = item.clone();
                if !expected["docs"].is_null() {
                    resolved_expected["documentation"] =
                        json!({"kind":"markdown","value":expected["docs"]});
                }
                assert_eq!(resolved["result"], resolved_expected);
                if item["label"] == query["apply"] {
                    let insertion = item["textEdit"]["newText"]
                        .as_str()
                        .expect("text")
                        .replace("$0", "");
                    let edited = apply_edits(
                        &source.text,
                        &[Edit {
                            start: (range.start.line, range.start.character),
                            end: (range.end.line, range.end.character),
                            text: &insertion,
                        }],
                    )
                    .expect("valid edit");
                    assert_eq!(
                        edited,
                        format!(
                            "{}{}{}",
                            &source.text[..range.start.byte],
                            expected["insert"]
                                .as_str()
                                .expect("insert")
                                .replace("$0", ""),
                            &source.text[range.end.byte..]
                        )
                    );
                    let _ = notify::<n::DidChangeTextDocument>(
                        &mut server,
                        json!({"textDocument":{"uri":uri(file),"version":2},"contentChanges":[{"text":edited}]}),
                    );
                    let again = response_value(request::<r::Completion>(
                        &mut server,
                        id,
                        json!({"textDocument":{"uri":uri(file)},"position":{"line":range.start.line,"character":range.start.character+item["label"].as_str().expect("label").len()}}),
                    ));
                    id += 1;
                    let candidates = again["result"]["items"].as_array().expect("requery");
                    if query["context"] == "RecordField" {
                        assert!(candidates.is_empty(), "used field must disappear: {query}");
                    } else {
                        let reapplied = candidates
                            .iter()
                            .find(|candidate| candidate["label"] == item["label"])
                            .expect("applied member");
                        assert_eq!(reapplied["data"], item["data"]);
                        assert_eq!(reapplied["detail"], item["detail"]);
                    }
                }
            }
        }
        std::fs::remove_dir_all(temp).expect("cleanup workspace");
    }
}
