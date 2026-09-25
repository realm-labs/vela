use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};

use crate::matrix_fixture::{Document, Marker, load, parse_markers};
use crate::tests::{TestServer, request, response_value, sync_diagnostics};

const URI: &str = "file:///workspace/scripts/game/main.vela";

fn range(marker: Marker) -> Value {
    json!({"start":{"line":marker.start.line,"character":marker.start.character},
        "end":{"line":marker.end.line,"character":marker.end.character}})
}

fn start(document: &Document, version: i32) -> (TestServer, Value) {
    let mut server = TestServer::new();
    let _ = response_value(request::<r::Initialize>(
        &mut server,
        1,
        json!({"processId":null,"rootUri":"file:///workspace/scripts","capabilities":{}}),
    ));
    let publication = sync_diagnostics::<n::DidOpenTextDocument>(
        &mut server,
        json!({"textDocument":{"uri":URI,"languageId":"vela","version":version,
            "text":document.text}}),
    );
    (server, publication)
}

fn projection(publication: &Value) -> Value {
    json!(
        publication["params"]["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .iter()
            .map(|item| json!({
                "code":item["code"], "message":item["message"],
                "severity":item["severity"], "range":item["range"],
                "candidates":item["data"]["candidates"].as_array().expect("candidates")
                    .iter().map(|candidate| candidate["replacement"].clone()).collect::<Vec<_>>()
            }))
            .collect::<Vec<_>>()
    )
}

fn expected(document: &Document, phase: &str, fixed: bool) -> Value {
    let mut diagnostics = Vec::new();
    match phase {
        "type" => diagnostics.push(json!({
            "code":"syntax::type_argument_arity",
            "message":"`Array` expects 1 type argument",
            "severity":1,"range":range(document.markers["bad"]),"candidates":[]
        })),
        "declaration" => diagnostics.push(json!({
            "code":"E_PARSE","message":"expected `)`",
            "severity":1,"range":range(document.markers["bad"]),"candidates":[]
        })),
        "baseline" | "member" | "call" | "shifted" | "restored" => {}
        other => panic!("unknown recovery phase {other}"),
    }
    diagnostics.push(json!({
        "code":"hir::unresolved_name","message":"unresolved name `missing`",
        "severity":1,"range":range(document.markers["missing"]),"candidates":[]
    }));
    if !fixed {
        diagnostics.push(json!({
            "code":"analysis::unknown_method",
            "message":"unknown method `frist` for `Array(i64)`",
            "severity":1,"range":range(document.markers["typo"]),
            "candidates":["first","find","last"]
        }));
    }
    json!(diagnostics)
}

fn actions(
    server: &mut TestServer,
    document: &Document,
    publication: &Value,
    marker: &str,
    version: i32,
    typo_exists: bool,
) -> Value {
    let response = response_value(request::<r::CodeActionRequest>(
        server,
        2,
        json!({"textDocument":{"uri":URI},"range":range(document.markers[marker]),
            "context":{"diagnostics":publication["params"]["diagnostics"]}}),
    ));
    let result = response["result"].as_array().expect("code actions");
    if marker == "typo" && typo_exists {
        assert_eq!(result.len(), 3, "three exact typo repairs");
        for (action, replacement) in result.iter().zip(["first", "find", "last"]) {
            assert_eq!(action["title"], format!("Replace with `{replacement}`"));
            assert_eq!(action["kind"], "quickfix");
            assert_eq!(action.as_object().expect("action object").len(), 3);
            let edit = json!({"range":range(document.markers[marker]),"newText":replacement});
            assert_eq!(
                action["edit"]["changes"]
                    .as_object()
                    .expect("changes")
                    .len(),
                1
            );
            assert_eq!(action["edit"]["changes"][URI], json!([edit]));
            assert_eq!(
                action["edit"]["documentChanges"],
                json!([{
                    "textDocument":{"uri":URI,"version":version},"edits":[edit]
                }])
            );
        }
    } else {
        assert!(result.is_empty(), "no guessed fix at {marker}");
    }
    response["result"].clone()
}

fn check(
    document: &Document,
    phase: &str,
    version: i32,
    publication: &Value,
    live: &mut TestServer,
    applied: Option<&Document>,
) {
    assert_eq!(publication["params"]["uri"], URI, "{phase}");
    assert_eq!(
        projection(publication),
        expected(document, phase, false),
        "complete diagnostics {phase}"
    );
    let (mut fresh_server, fresh) = start(document, version);
    assert_eq!(
        publication["params"]["diagnostics"], fresh["params"]["diagnostics"],
        "fresh publication {phase}"
    );
    let mut selected = None;
    for marker in ["typo", "missing", "bad"] {
        if document.markers.contains_key(marker) {
            let actual = actions(live, document, publication, marker, version, true);
            assert_eq!(
                actual,
                actions(&mut fresh_server, document, &fresh, marker, version, true),
                "fresh action set {phase} {marker}"
            );
            if marker == "typo" {
                selected = Some(actual[0].clone());
            }
        }
    }

    if let Some(applied_document) = applied {
        let selected = selected.expect("selected typo fix");
        let edit = &selected["edit"]["changes"][URI][0];
        let marker = document.markers["typo"];
        let mut applied_text = document.text.clone();
        applied_text.replace_range(
            marker.start.byte..marker.end.byte,
            edit["newText"].as_str().expect("replacement"),
        );
        assert_eq!(applied_text, applied_document.text, "whole applied {phase}");
        let changed = sync_diagnostics::<n::DidChangeTextDocument>(
            &mut fresh_server,
            json!({"textDocument":{"uri":URI,"version":version+1},
                "contentChanges":[{"text":applied_text}]}),
        );
        assert_eq!(
            projection(&changed),
            expected(applied_document, phase, true),
            "only selected diagnostic clears {phase}"
        );
        for marker in ["typo", "missing", "bad"] {
            if applied_document.markers.contains_key(marker) {
                assert_eq!(
                    actions(
                        &mut fresh_server,
                        applied_document,
                        &changed,
                        marker,
                        version + 1,
                        false
                    ),
                    json!([]),
                    "no stale or guessed action after repair {phase} {marker}"
                );
            }
        }
    }
}

#[test]
fn incomplete_neighbors_preserve_diagnostics_and_safe_actions() {
    for crlf in [false, true] {
        let spec = load("diagnostic-recovery-partitions");
        let marked = |source: &str| {
            parse_markers(&if crlf {
                source.replace('\n', "\r\n")
            } else {
                source.to_owned()
            })
            .expect("marked source")
        };
        let baseline = marked(&spec.files["scripts/game/main.vela"]);
        let (mut live, opened) = start(&baseline, 1);
        check(&baseline, "baseline", 1, &opened, &mut live, None);

        for (index, state) in spec.oracle["states"]
            .as_array()
            .expect("states")
            .iter()
            .enumerate()
        {
            let phase = state["id"].as_str().expect("phase");
            let document = marked(state["text"].as_str().expect("phase text"));
            let version = index as i32 + 2;
            let publication = sync_diagnostics::<n::DidChangeTextDocument>(
                &mut live,
                json!({"textDocument":{"uri":URI,"version":version},
                    "contentChanges":[{"text":document.text}]}),
            );
            let applied_source = format!(
                "{}{}{}",
                state["appliedPrefix"].as_str().unwrap_or(""),
                spec.oracle["appliedHead"].as_str().expect("applied head"),
                state["appliedTail"].as_str().expect("applied tail")
            );
            let applied = marked(&applied_source);
            check(
                &document,
                phase,
                version,
                &publication,
                &mut live,
                Some(&applied),
            );
            if phase == "shifted" {
                let stale = response_value(request::<r::CodeActionRequest>(
                    &mut live,
                    9,
                    json!({"textDocument":{"uri":URI},
                        "range":range(baseline.markers["typo"]),
                        "context":{"diagnostics":publication["params"]["diagnostics"]}}),
                ));
                assert_eq!(
                    stale["result"],
                    json!([]),
                    "old range must not repair shifted text"
                );
            }
        }

        let restored_version = spec.oracle["states"].as_array().expect("states").len() as i32 + 2;
        let publication = sync_diagnostics::<n::DidChangeTextDocument>(
            &mut live,
            json!({"textDocument":{"uri":URI,"version":restored_version},
                "contentChanges":[{"text":baseline.text}]}),
        );
        check(
            &baseline,
            "restored",
            restored_version,
            &publication,
            &mut live,
            None,
        );
    }
}
