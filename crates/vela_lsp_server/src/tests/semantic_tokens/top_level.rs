use std::fs;

use lsp_types::{notification as n, request as r};
use serde_json::json;

use super::support::{apply_delta, delta, full, rows, start};
use crate::matrix_fixture::{FixtureWorkspace, load, parse_markers, semantic_tokens as oracle};
use crate::tests::{notify, request, response_value, sync_diagnostics};

#[test]
fn top_level_declarations_and_duplicates_have_exact_streams_deltas_and_ranges() {
    let spec = load("semantic-token-top-level");
    for crlf in [false, true] {
        let source = |text: &str| {
            parse_markers(&if crlf {
                text.replace('\n', "\r\n")
            } else {
                text.to_owned()
            })
            .expect("document")
        };
        let mut fixture = FixtureWorkspace::new(&spec).expect("fixture");
        fixture.disk.insert(
            "scripts/main.vela".into(),
            source(&spec.files["scripts/main.vela"]),
        );
        let positive = &fixture.disk["scripts/main.vela"];
        let negative = source(
            spec.oracle["negative"]["source"]
                .as_str()
                .expect("negative source"),
        );
        let parent = crate::tests::support::unique_temp_root("semantic-declarations");
        let root = parent.join("中文 % declarations");
        fixture.materialize(&root).expect("workspace");
        let uri = lsp_types::Url::from_file_path(root.join("scripts/main.vela"))
            .expect("URI")
            .to_string();
        assert!(uri.contains('%'));
        let (mut server, legend) = start(&root);
        let _ = sync_diagnostics::<n::DidOpenTextDocument>(
            &mut server,
            json!({"textDocument":{"uri":uri,"languageId":"vela","version":1,"text":positive.text}}),
        );
        let original = full(&mut server, &uri);
        let mut previous = original.clone();
        for (index, (document, expected)) in [
            (positive, &spec.oracle["positive"]),
            (&negative, &spec.oracle["negative"]["tokens"]),
            (positive, &spec.oracle["positive"]),
        ]
        .into_iter()
        .enumerate()
        {
            if index == 1 {
                let _ = sync_diagnostics::<n::DidChangeTextDocument>(
                    &mut server,
                    json!({"textDocument":{"uri":uri,"version":2},"contentChanges":[{"text":document.text}]}),
                );
            } else if index == 2 {
                let _ = notify::<n::DidCloseTextDocument>(
                    &mut server,
                    json!({"textDocument":{"uri":uri}}),
                );
            }
            let expected = oracle::expected(document, expected, true);
            let current = full(&mut server, &uri);
            oracle::assert_stream(&rows(document, &current["data"], &legend), &expected);
            let changed = delta(&mut server, &uri, &previous["resultId"]);
            assert_eq!(apply_delta(&previous["data"], &changed), current["data"]);
            assert_eq!(changed["resultId"], current["resultId"]);
            assert_eq!(
                delta(&mut server, &uri, &current["resultId"])["edits"],
                json!([])
            );
            assert_eq!(full(&mut server, &uri), current);
            let (mut fresh, fresh_legend) = start(&root);
            let _ = sync_diagnostics::<n::DidOpenTextDocument>(
                &mut fresh,
                json!({"textDocument":{"uri":uri,"languageId":"vela","version":1,"text":document.text}}),
            );
            assert_eq!(fresh_legend, legend);
            assert_eq!(full(&mut fresh, &uri), current);
            let ranges = (0..document.text.lines().count())
                .map(|line| {
                    (
                        json!({"line":line,"character":0}),
                        json!({"line":line+1,"character":0}),
                        expected
                            .iter()
                            .filter(|row| row.line == line)
                            .cloned()
                            .collect::<Vec<_>>(),
                    )
                })
                .chain(expected.iter().flat_map(|token| {
                    let start = json!({"line":token.line,"character":token.column});
                    let end = json!({"line":token.line,"character":token.column+token.length});
                    [
                        (start, end.clone(), vec![token.clone()]),
                        (end.clone(), end, vec![]),
                    ]
                }));
            for (start, end, wanted) in ranges {
                let response = response_value(request::<r::SemanticTokensRangeRequest>(
                    &mut server,
                    4,
                    json!({"textDocument":{"uri":uri},"range":{"start":start,"end":end}}),
                ));
                assert!(response.get("error").is_none(), "{response}");
                oracle::assert_stream(
                    &rows(document, &response["result"]["data"], &legend),
                    &wanted,
                );
            }
            previous = current;
        }
        assert_eq!(previous, original, "close restores disk stream and ID");
        fs::remove_dir_all(parent).expect("cleanup own fixture");
    }
}
