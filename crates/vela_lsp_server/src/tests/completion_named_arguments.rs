use crate::matrix_fixture::{Edit, FixtureWorkspace, apply_edits, load};
use crate::tests::{TestServer, notify, request, response_value};
use lsp_types::{notification as n, request as r};
use serde_json::json;

#[test]
fn named_argument_matrix_projects_exact_parameter_sets_and_parseable_utf16_edits() {
    for crlf in [false, true] {
        let mut spec = load("completion-named-arguments");
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
                    .find(|item| item["label"] == expected["name"])
                    .expect("parameter");
                assert_eq!(item["kind"], 6);
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
                            .all(|candidate| candidate["label"] != item["label"]),
                        "occupied parameter: {query}"
                    );
                }
            }
        }
        std::fs::remove_dir_all(temp).expect("cleanup workspace");
    }
}
