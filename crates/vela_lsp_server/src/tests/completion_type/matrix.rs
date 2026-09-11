use lsp_types::{notification as n, request as r};
use serde_json::json;

use crate::matrix_fixture::{Edit, FixtureWorkspace, apply_edits, load, schema_artifact};
use crate::tests::{TestServer, notify, request, response_value};

#[test]
fn type_completion_matrix_projects_utf16_edits_ownership_docs_and_applied_source() {
    assert_type_matrix("completion-type-positions");
}

#[test]
fn builtin_type_completion_matrix_projects_public_spellings_edits_and_negative_boundaries() {
    assert_type_matrix("completion-builtin-types");
}

fn assert_type_matrix(fixture_id: &str) {
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
            assert_eq!(response["id"], id);
            id += 1;
            assert_eq!(response["result"]["isIncomplete"], false);
            let items = response["result"]["items"].as_array().expect("items");
            if let Some(expected) = query["typeInventory"].as_array() {
                let mut actual = items
                    .iter()
                    .filter(|item| item["kind"] == 22)
                    .map(|item| item["label"].as_str().expect("label"))
                    .collect::<Vec<_>>();
                actual.sort_unstable();
                assert_eq!(json!(actual), json!(expected));
            }
            if query["empty"] == true {
                assert!(items.is_empty(), "{query}: {response}");
                continue;
            }
            for excluded in query["exclude"].as_array().expect("exclusions") {
                assert!(
                    items.iter().all(|item| item["label"] != *excluded),
                    "{query}: {response}"
                );
            }
            let range = source.markers["replace"];
            for expected in query["items"].as_array().expect("expected items") {
                let found = items
                    .iter()
                    .filter(|item| item["label"] == expected["label"])
                    .collect::<Vec<_>>();
                assert_eq!(found.len(), 1, "{query}: {response}");
                let item = found[0];
                assert_eq!(item["kind"], expected["lspKind"]);
                assert_eq!(item["detail"], expected["detail"]);
                assert_eq!(item["filterText"], expected["lookup"]);
                assert_eq!(item["labelDetails"]["description"], expected["owner"]);
                assert_eq!(item["labelDetails"]["detail"], expected["detail"]);
                assert!(item.get("documentation").is_none());
                assert_eq!(
                    item["data"]["resolve"],
                    json!({"kind":"documentation","symbol":{
                        "kind":expected["symbolKind"],"name":expected["symbol"]
                    }})
                );
                assert_eq!(
                    item["textEdit"],
                    json!({"range":{
                    "start":{"line":range.start.line,"character":range.start.character},
                    "end":{"line":range.end.line,"character":range.end.character}
                },"newText":expected["label"]})
                );
                assert_eq!(item["insertText"], expected["label"]);
                let resolved = response_value(request::<r::ResolveCompletionItem>(
                    &mut server,
                    id,
                    item.clone(),
                ));
                assert_eq!(resolved["id"], id);
                id += 1;
                let mut expected_resolved = item.clone();
                if !expected["docs"].is_null() {
                    expected_resolved["documentation"] =
                        json!({"kind":"markdown","value":expected["docs"]});
                }
                assert_eq!(resolved["result"], expected_resolved);
                if item["label"] == query["apply"] {
                    let edited = apply_edits(
                        &source.text,
                        &[Edit {
                            start: (range.start.line, range.start.character),
                            end: (range.end.line, range.end.character),
                            text: item["textEdit"]["newText"].as_str().expect("replacement"),
                        }],
                    )
                    .expect("valid edit");
                    let inserted = query["apply"].as_str().expect("apply");
                    assert_eq!(
                        edited,
                        format!(
                            "{}{}{}",
                            &source.text[..range.start.byte],
                            inserted,
                            &source.text[range.end.byte..]
                        )
                    );
                    let _ = notify::<n::DidChangeTextDocument>(
                        &mut server,
                        json!({
                            "textDocument":{"uri":uri(file),"version":2},"contentChanges":[{"text":edited}]
                        }),
                    );
                    let again = response_value(request::<r::Completion>(
                        &mut server,
                        id,
                        json!({
                            "textDocument":{"uri":uri(file)},"position":{"line":range.start.line,"character":range.start.character + query["requeryOffset"].as_u64().map_or(inserted.len(), |offset| usize::try_from(offset).expect("offset"))}
                        }),
                    ));
                    id += 1;
                    let again_items = again["result"]["items"].as_array().expect("requery items");
                    if let Some(expected) = query["requeryTypeInventory"].as_array() {
                        let mut labels = again_items
                            .iter()
                            .filter(|candidate| candidate["kind"] == 22)
                            .map(|candidate| candidate["label"].as_str().expect("label"))
                            .collect::<Vec<_>>();
                        labels.sort_unstable();
                        assert_eq!(json!(labels), json!(expected));
                        assert_eq!(
                            again_items
                                .iter()
                                .find(|candidate| candidate["label"] == query["apply"])
                                .expect("applied unit")["data"],
                            item["data"]
                        );
                        continue;
                    }
                    assert_eq!(again_items.len(), 1, "{query}: {again}");
                    assert_eq!(again_items[0]["label"], query["apply"]);
                    assert_eq!(again_items[0]["data"], item["data"]);
                }
            }
        }
        std::fs::remove_dir_all(temp).expect("cleanup isolated workspace");
    }
}
