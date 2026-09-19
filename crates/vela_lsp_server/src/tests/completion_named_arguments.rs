use crate::matrix_fixture::{Edit, FixtureWorkspace, apply_edits, load};
use crate::tests::{TestServer, notify, request, response_value};
use lsp_types::{notification as n, request as r};
use serde_json::json;

#[test]
fn callable_hint_matrix_projects_nested_types_and_registered_parameter_prefixes() {
    verify_fixture("completion-callable-hints");
}

#[test]
fn named_argument_matrix_projects_exact_parameter_sets_and_parseable_utf16_edits() {
    verify_fixture("completion-named-arguments");
}

#[test]
fn stdlib_named_argument_matrix_projects_registered_names_edits_and_negative_boundaries() {
    verify_fixture("completion-stdlib-arguments");
}

#[test]
fn service_named_argument_matrix_projects_contracts_and_utf16_edits() {
    verify_fixture("completion-service-arguments");
}

fn verify_fixture(fixture_name: &str) {
    for crlf in [false, true] {
        let mut spec = load(fixture_name);
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
        for (file, source) in &fixture.disk {
            if file.ends_with(".vela") {
                let messages = notify::<n::DidOpenTextDocument>(
                    &mut server,
                    json!({"textDocument":{
                        "uri":uri(file),"languageId":"vela","version":1,"text":source.text
                    }}),
                );
                if spec.oracle["queries"]
                    .as_array()
                    .expect("queries")
                    .iter()
                    .any(|query| query["file"] == *file && query["diagnosticError"] == true)
                {
                    let notifications = crate::tests::notification_values(messages);
                    assert!(
                        notifications
                            .iter()
                            .any(|notification| notification["method"]
                                == "textDocument/publishDiagnostics"
                                && notification["params"]["uri"] == uri(file)
                                && notification["params"]["diagnostics"]
                                    .as_array()
                                    .is_some_and(|diagnostics| diagnostics
                                        .iter()
                                        .any(|d| d["severity"] == 1))),
                        "invalid import diagnostics: {notifications:?}"
                    );
                }
            }
        }
        let mut id = 2;
        for query in spec.oracle["queries"].as_array().expect("queries") {
            let file = query["file"].as_str().expect("file");
            let source = fixture.document(file).expect("source");
            if query["parseErrors"] == true {
                assert!(
                    !vela_syntax::parse::parse_source(&source.text)
                        .diagnostics()
                        .is_empty(),
                    "invalid non-builtin type arguments: {query}"
                );
            }
            let point = source.markers["cursor"].start;
            let range = source.markers["replace"];
            let response = response_value(request::<r::Completion>(
                &mut server,
                id,
                json!({"textDocument":{"uri":uri(file)},"position":{"line":point.line,"character":point.character}}),
            ));
            id += 1;
            let repeated = response_value(request::<r::Completion>(
                &mut server,
                id,
                json!({"textDocument":{"uri":uri(file)},"position":{"line":point.line,"character":point.character}}),
            ));
            id += 1;
            assert_eq!(repeated["result"], response["result"]);
            let items = response["result"]["items"].as_array().expect("items");
            let mut names = items
                .iter()
                .map(|item| item["label"].as_str().expect("name"))
                .collect::<Vec<_>>();
            names.sort_unstable();
            let mut expected = query["parameters"]
                .as_array()
                .expect("params")
                .iter()
                .map(|item| item["name"].as_str().expect("name"))
                .collect::<Vec<_>>();
            expected.extend(crate::matrix_fixture::expected_expression_labels(
                &spec.oracle,
                query,
            ));
            expected.sort_unstable();
            assert_eq!(names, expected, "{query}");
            if let Some(label) = query["signature"].as_str() {
                let help = response_value(request::<r::SignatureHelpRequest>(
                    &mut server,
                    id,
                    json!({"textDocument":{"uri":uri(file)},"position":{"line":point.line,"character":point.character}}),
                ));
                id += 1;
                assert_eq!(
                    help["result"]["signatures"]
                        .as_array()
                        .expect("signatures")
                        .len(),
                    1,
                    "{query}"
                );
                assert_eq!(help["result"]["signatures"][0]["label"], label, "{query}");
                if let Some(active) = query["activeParameter"].as_u64() {
                    assert_eq!(help["result"]["activeParameter"], active, "{query}");
                }
            }
            for expected in query["parameters"].as_array().expect("params") {
                let item = items
                    .iter()
                    .find(|item| {
                        item["label"] == expected["name"]
                            && item["labelDetails"]["description"] == "named argument"
                    })
                    .expect("parameter");
                assert_eq!(item["kind"], 6);
                assert!(
                    item.get("documentation").is_none(),
                    "lightweight parameter: {query}"
                );
                assert!(
                    item["data"].get("resolve").is_none(),
                    "no parameter lazy payload: {query}"
                );
                let before = server.snapshot();
                for _ in 0..2 {
                    let resolved = response_value(request::<r::ResolveCompletionItem>(
                        &mut server,
                        id,
                        item.clone(),
                    ));
                    id += 1;
                    assert!(resolved["error"].is_null(), "{resolved}");
                    assert_eq!(
                        resolved["result"], *item,
                        "parameter resolve preserves every field: {query}"
                    );
                }
                let after = server.snapshot();
                assert_eq!(
                    after.databases().parse_db().parse_count(),
                    before.databases().parse_db().parse_count()
                );
                assert_eq!(
                    after.databases().hir_db().rebuild_count(),
                    before.databases().hir_db().rebuild_count()
                );
                assert_eq!(item["detail"], expected["detail"]);
                assert_eq!(
                    item["textEdit"],
                    json!({"range":{"start":{"line":range.start.line,"character":range.start.character},"end":{"line":range.end.line,"character":range.end.character}},"newText":expected["insert"]})
                );
                if item["label"] == query["apply"] {
                    let insertion = format!(
                        "{}{}",
                        item["textEdit"]["newText"].as_str().expect("text"),
                        query["value"].as_str().expect("value")
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
                            "{}{}{}{}",
                            &source.text[..range.start.byte],
                            expected["insert"].as_str().expect("insert"),
                            query["value"].as_str().expect("value"),
                            &source.text[range.end.byte..]
                        )
                    );
                    assert!(
                        vela_syntax::parse::parse_source(&edited)
                            .diagnostics()
                            .is_empty(),
                        "applied call must parse: {edited}"
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
                    assert!(
                        again["result"]["items"]
                            .as_array()
                            .expect("requery")
                            .iter()
                            .any(|candidate| candidate["label"] == item["label"]
                                && candidate["textEdit"]["newText"] == item["label"]),
                        "current label replacement: {query}"
                    );
                }
            }
        }
        std::fs::remove_dir_all(temp).expect("cleanup workspace");
    }
}
