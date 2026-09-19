use super::{TestServer, notify, request, response_value};
use crate::matrix_fixture::{Edit, apply_edits, load, parse_markers};
use lsp_types::{notification as n, request as r};
use serde_json::json;

#[test]
fn structured_completion_contexts_project_exact_candidates_and_independent_edits() {
    let spec = load("completion-analysis-contexts");
    let uri = "file:///workspace/scripts/main.vela";
    for crlf in [false, true] {
        for case in spec.oracle["queries"].as_array().expect("queries") {
            let text = spec.files[case["file"].as_str().expect("file")]
                .replace('\n', if crlf { "\r\n" } else { "\n" });
            let source = parse_markers(&text).expect("source");
            let mut live = TestServer::new();
            let initialized = response_value(request::<r::Initialize>(
                &mut live,
                1,
                json!({"processId":null,"rootUri":"file:///workspace/scripts","capabilities":{"textDocument":{"completion":{"completionItem":{"snippetSupport":true,"labelDetailsSupport":true}}}}}),
            ));
            assert!(initialized["error"].is_null());
            let _ = notify::<n::DidOpenTextDocument>(
                &mut live,
                json!({"textDocument":{"uri":uri,"languageId":"vela","version":1,"text":source.text}}),
            );
            let point = source.markers["cursor"].start;
            let params = json!({"textDocument":{"uri":uri},"position":{"line":point.line,"character":point.character}});
            let first = response_value(request::<r::Completion>(&mut live, 2, params.clone()));
            let repeated = response_value(request::<r::Completion>(&mut live, 3, params));
            assert!(first["error"].is_null(), "{first}");
            assert_eq!(first["result"], repeated["result"]);
            assert_eq!(first["result"]["isIncomplete"], false);
            assert!(first["result"].get("analysis").is_none());
            let items = first["result"]["items"].as_array().expect("items");
            let expected = case["items"].as_array().expect("expected");
            assert_eq!(
                items.iter().map(|i| &i["label"]).collect::<Vec<_>>(),
                expected.iter().map(|i| &i["label"]).collect::<Vec<_>>(),
                "{case}"
            );
            for (item, expected) in items.iter().zip(expected) {
                assert_eq!(item["kind"], expected["kind"], "{case}");
                assert_eq!(item["detail"], expected["detail"], "{case}");
                assert_eq!(item["insertText"], expected["insert"], "{case}");
                let range = source.markers["replace"];
                assert_eq!(
                    item["textEdit"],
                    json!({"range":{"start":{"line":range.start.line,"character":range.start.character},"end":{"line":range.end.line,"character":range.end.character}},"newText":expected["insert"]})
                );
                let resolved = response_value(request::<r::ResolveCompletionItem>(
                    &mut live,
                    4,
                    item.clone(),
                ));
                assert!(resolved["error"].is_null());
                assert_eq!(&resolved["result"], item);
                let mut expanded = item["textEdit"]["newText"]
                    .as_str()
                    .expect("edit")
                    .to_owned();
                for fill in expected["fill"].as_array().expect("fills") {
                    expanded = expanded.replace(
                        fill[0].as_str().expect("from"),
                        fill[1].as_str().expect("to"),
                    );
                }
                // Plain keyword/argument insertions intentionally leave a value for the user.
                if !expanded.contains('$') && expected["fill"].as_array().expect("fills").is_empty()
                {
                    let completed = expected["expanded"].as_str().expect("completed");
                    assert!(completed.starts_with(&expanded));
                    expanded = completed.to_owned();
                }
                assert_eq!(expanded, expected["expanded"]);
                let applied = apply_edits(
                    &source.text,
                    &[Edit {
                        start: (range.start.line, range.start.character),
                        end: (range.end.line, range.end.character),
                        text: &expanded,
                    }],
                )
                .expect("edit");
                assert_eq!(
                    applied,
                    format!(
                        "{}{}{}",
                        &source.text[..range.start.byte],
                        expected["expanded"].as_str().expect("expanded"),
                        &source.text[range.end.byte..]
                    )
                );
                let parsed = vela_syntax::parse::parse_source(&applied);
                assert!(
                    parsed.diagnostics().is_empty(),
                    "{case}: {:?}",
                    parsed.diagnostics()
                );
            }
        }
    }
}
