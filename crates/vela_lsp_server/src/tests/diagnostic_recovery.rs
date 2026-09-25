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

fn expected(document: &Document, phase: &str) -> Value {
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
        "baseline" | "member" | "call" | "restored" => {}
        other => panic!("unknown recovery phase {other}"),
    }
    diagnostics.push(json!({
        "code":"hir::unresolved_name","message":"unresolved name `missing`",
        "severity":1,"range":range(document.markers["missing"]),"candidates":[]
    }));
    diagnostics.push(json!({
        "code":"analysis::unknown_method",
        "message":"unknown method `frist` for `Array(i64)`",
        "severity":1,"range":range(document.markers["typo"]),
        "candidates":["first","find","last"]
    }));
    json!(diagnostics)
}

fn check(document: &Document, phase: &str, version: i32, publication: &Value) {
    assert_eq!(publication["params"]["uri"], URI, "{phase}");
    assert_eq!(
        projection(publication),
        expected(document, phase),
        "complete diagnostics {phase}"
    );
    let (_, fresh) = start(document, version);
    assert_eq!(
        publication["params"]["diagnostics"], fresh["params"]["diagnostics"],
        "fresh publication {phase}"
    );
}

#[test]
fn incomplete_neighbors_preserve_source_diagnostics() {
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
        check(&baseline, "baseline", 1, &opened);

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
            check(&document, phase, version, &publication);
        }

        let publication = sync_diagnostics::<n::DidChangeTextDocument>(
            &mut live,
            json!({"textDocument":{"uri":URI,"version":6},
                "contentChanges":[{"text":baseline.text}]}),
        );
        check(&baseline, "restored", 6, &publication);
    }
}
