use crate::matrix_fixture::{Edit, apply_edits, load, parse_markers};
use crate::tests::{TestServer, notify, request, response_value};
use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};

const URI: &str = "file:///workspace/scripts/main.vela";

#[test]
fn authoring_surface_projects_complete_templates_compact_details_and_applied_source() {
    let spec = load("completion-authoring-surface");
    for crlf in [false, true] {
        for query in spec.oracle["queries"].as_array().expect("queries") {
            let file = query["file"].as_str().expect("file");
            let source = parse_markers(&eol(&spec.files[file], crlf)).expect("source");
            let mut live = start(&source.text);
            let point = source.markers["cursor"].start;
            let params = json!({"textDocument":{"uri":URI},"position":{"line":point.line,"character":point.character}});
            let result = completion(&mut live, params.clone());
            assert_eq!(result, completion(&mut live, params));
            assert_eq!(result["isIncomplete"], false);
            let items = result["items"].as_array().expect("items");
            let expected_items = query["items"].as_array().expect("expected");
            let allowed = query["allowedBuiltins"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let mut expected_labels = expected_items
                .iter()
                .map(|i| &i["label"])
                .chain(allowed.iter())
                .collect::<Vec<_>>();
            if !allowed.is_empty() {
                expected_labels.sort_by_key(|i| i.as_str().expect("label"));
            }
            if let Some(inventory) = query["inventory"].as_array() {
                expected_labels = inventory.iter().collect();
                let owner = query["owner"].as_str().expect("owner");
                let range = source.markers["replace"];
                for item in items {
                    assert_eq!(item["kind"], 2);
                    assert_eq!(
                        item["data"]["resolve"]["symbol"],
                        json!({"kind":"builtin","name":format!("{owner}.{}",item["label"].as_str().expect("label"))})
                    );
                    assert_eq!(
                        item["textEdit"],
                        json!({"range":{"start":{"line":range.start.line,"character":range.start.character},"end":{"line":range.end.line,"character":range.end.character}},"newText":format!("{}($0)",item["label"].as_str().expect("label"))})
                    );
                    assert_eq!(item["insertTextFormat"], 2);
                    assert!(item.get("documentation").is_none());
                    let resolved = response_value(request::<r::ResolveCompletionItem>(
                        &mut live,
                        3,
                        item.clone(),
                    ));
                    assert!(resolved["error"].is_null());
                    assert_eq!(&resolved["result"], item);
                }
            }
            assert_eq!(
                items.iter().map(|i| &i["label"]).collect::<Vec<_>>(),
                expected_labels,
                "{query}"
            );
            for label in &allowed {
                let item = items
                    .iter()
                    .find(|i| &i["label"] == label)
                    .expect("builtin item");
                assert_eq!(item["kind"], 3);
                assert_eq!(
                    item["data"]["resolve"]["symbol"],
                    json!({"kind":"builtin","name":label})
                );
                assert!(item.get("documentation").is_none());
            }
            for expected in expected_items {
                let item = items
                    .iter()
                    .find(|i| i["label"] == expected["label"])
                    .expect("expected candidate");
                assert_eq!(item["kind"], expected["lspKind"]);
                assert_eq!(item["detail"], expected["detail"]);
                assert_eq!(item["filterText"], expected["label"]);
                assert_eq!(item["labelDetails"], json!({"detail":expected["detail"]}));
                assert!(item.get("documentation").is_none());
                if let Some(symbol) = expected["symbol"].as_str() {
                    assert_eq!(
                        item["data"]["resolve"],
                        json!({"kind":"documentation","symbol":{"kind":"builtin","name":symbol}})
                    );
                } else {
                    assert_eq!(item["data"], json!({"source":"vela"}));
                }
                assert_eq!(item["insertText"], expected["insert"]);
                assert_eq!(
                    item["insertTextFormat"],
                    if expected["format"] == 2 {
                        json!(2)
                    } else {
                        Value::Null
                    }
                );
                let range = source.markers["replace"];
                assert_eq!(
                    item["textEdit"],
                    json!({"range":{"start":{"line":range.start.line,"character":range.start.character},"end":{"line":range.end.line,"character":range.end.character}},"newText":expected["insert"]})
                );
                let resolved = response_value(request::<r::ResolveCompletionItem>(
                    &mut live,
                    3,
                    item.clone(),
                ));
                assert!(resolved["error"].is_null());
                assert_eq!(&resolved["result"], item);
                let mut expanded = item["textEdit"]["newText"]
                    .as_str()
                    .expect("insert")
                    .to_owned();
                for fill in expected["fills"].as_array().expect("fills") {
                    let from = fill[0].as_str().expect("placeholder");
                    assert!(expanded.contains(from));
                    expanded = expanded.replace(from, fill[1].as_str().expect("fill"));
                }
                assert_eq!(expanded, expected["expanded"]);
                assert!(!expanded.contains('$'));
                let applied = apply_edits(
                    &source.text,
                    &[Edit {
                        start: (range.start.line, range.start.character),
                        end: (range.end.line, range.end.character),
                        text: &expanded,
                    }],
                )
                .expect("edit");
                let expected_text = eol(expected["applied"].as_str().expect("applied"), crlf)
                    .replacen(&eol(&expanded, crlf), &expanded, 1);
                assert_eq!(applied, expected_text, "{query}");
                let parsed = vela_syntax::parse::parse_source(&applied);
                assert!(
                    parsed.diagnostics().is_empty(),
                    "{query}: {:?}",
                    parsed.diagnostics()
                );
                let mut applied_server = start(&source.text);
                let _ = notify::<n::DidChangeTextDocument>(
                    &mut applied_server,
                    json!({"textDocument":{"uri":URI,"version":2},"contentChanges":[{"text":applied}]}),
                );
                let again = completion(
                    &mut applied_server,
                    json!({"textDocument":{"uri":URI},"position":{"line":range.start.line,"character":range.start.character+if expected["kind"]=="Parameter" {expanded.encode_utf16().count()} else {0}}}),
                );
                let labels = again["items"]
                    .as_array()
                    .expect("applied items")
                    .iter()
                    .filter(|i| {
                        if query["inventory"].is_array() {
                            i["kind"] == 2
                        } else if expected["kind"] == "Parameter" {
                            i["kind"] == 6
                        } else {
                            i["kind"] == 14 || i["kind"] == 15
                        }
                    })
                    .map(|i| &i["label"])
                    .collect::<Vec<_>>();
                assert_eq!(json!(labels), expected["requery"], "{query}: {applied}");
            }
        }
    }
}
fn eol(text: &str, crlf: bool) -> String {
    text.replace('\n', if crlf { "\r\n" } else { "\n" })
}
fn completion(live: &mut TestServer, params: Value) -> Value {
    let response = response_value(request::<r::Completion>(live, 2, params));
    assert!(response["error"].is_null(), "{response}");
    response["result"].clone()
}
fn start(text: &str) -> TestServer {
    let mut live = TestServer::new();
    let initialized = response_value(request::<r::Initialize>(
        &mut live,
        1,
        json!({"processId":null,"rootUri":"file:///workspace/scripts","capabilities":{"textDocument":{"completion":{"completionItem":{"snippetSupport":true,"labelDetailsSupport":true}}}}}),
    ));
    assert!(initialized["error"].is_null());
    let _ = notify::<n::DidOpenTextDocument>(
        &mut live,
        json!({"textDocument":{"uri":URI,"languageId":"vela","version":1,"text":text}}),
    );
    live
}
