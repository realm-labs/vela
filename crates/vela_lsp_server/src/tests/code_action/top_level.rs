use std::{fs, path::Path};

use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};

use crate::matrix_fixture::{Document, FixtureWorkspace, Spec, load};
use crate::tests::{TestServer, request, response_value, sync_diagnostics};

fn uri(root: &Path, file: &str) -> String {
    lsp_types::Url::from_file_path(root.join(file))
        .expect("URI")
        .to_string()
}

fn initialized(root: &Path) -> TestServer {
    let mut server = TestServer::new();
    let _ = response_value(request::<r::Initialize>(
        &mut server,
        1,
        json!({"processId":null,"rootUri":uri(root,""),"capabilities":{}}),
    ));
    server
}

fn open(server: &mut TestServer, uri: &str, text: &str, version: i32) -> Value {
    sync_diagnostics::<n::DidOpenTextDocument>(
        server,
        json!({"textDocument":{"uri":uri,"languageId":"vela","version":version,"text":text}}),
    )
}

fn actions(server: &mut TestServer, uri: &str, span: Value, publication: &Value) -> Value {
    response_value(request::<r::CodeActionRequest>(
        server,
        2,
        json!({"textDocument":{"uri":uri},"range":span,
            "context":{"diagnostics":publication["params"]["diagnostics"]}}),
    ))["result"]
        .clone()
}

fn marker_range(document: &Document, name: &str) -> Value {
    let marker = document.markers[name];
    json!({"start":{"line":marker.start.line,"character":marker.start.character},
        "end":{"line":marker.end.line,"character":marker.end.character}})
}

fn utf16_column(document: &Document, line: usize, byte: usize) -> usize {
    document.text.split('\n').nth(line).expect("line")[..byte]
        .encode_utf16()
        .count()
}

fn state_range(document: &Document, item: &Value) -> Value {
    let line = item["line"].as_u64().expect("line") as usize;
    let start = utf16_column(
        document,
        line,
        item["startByte"].as_u64().expect("start") as usize,
    );
    let end = utf16_column(
        document,
        line,
        item["endByte"].as_u64().expect("end") as usize,
    );
    json!({"start":{"line":line,"character":start},"end":{"line":line,"character":end}})
}

fn repaired_duplicates(spec: &Spec, document: &Document) -> String {
    let mut edits = spec.oracle["repairs"]
        .as_object()
        .expect("repairs")
        .iter()
        .map(|(name, value)| {
            let marker = document.markers[name];
            (
                marker.start.byte,
                marker.end.byte,
                value.as_str().expect("replacement"),
            )
        })
        .collect::<Vec<_>>();
    edits.sort_by_key(|edit| edit.0);
    let mut text = document.text.clone();
    for (start, end, replacement) in edits.into_iter().rev() {
        text.replace_range(start..end, replacement);
    }
    text
}

#[test]
fn top_level_actions_publish_no_placeholder_or_duplicate_edits() {
    for crlf in [false, true] {
        let mut spec = load("diagnostic-top-level-declarations");
        if crlf {
            for text in spec.files.values_mut() {
                *text = text.replace('\n', "\r\n");
            }
        }
        let fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let parent = crate::tests::support::unique_temp_root("code-action-top-level");
        let root = parent.join("中文 % declarations");
        fixture.materialize(&root).expect("workspace");
        let mut server = initialized(&root);
        let valid = uri(&root, "scripts/valid.vela");
        let invalid = uri(&root, "scripts/invalid.vela");
        let state = uri(&root, "scripts/invalid_state.vela");
        let legacy = uri(&root, "scripts/legacy.vela");
        assert!(legacy.contains('%'));
        let valid_document = &fixture.disk["scripts/valid.vela"];
        let invalid_document = &fixture.disk["scripts/invalid.vela"];
        let state_document = &fixture.disk["scripts/invalid_state.vela"];
        let legacy_document = &fixture.disk["scripts/legacy.vela"];
        let valid_open = open(&mut server, &valid, &valid_document.text, 1);
        let invalid_open = open(&mut server, &invalid, &invalid_document.text, 1);
        let state_open = open(&mut server, &state, &state_document.text, 1);
        let legacy_open = open(&mut server, &legacy, &legacy_document.text, 1);
        assert_eq!(valid_open["params"]["diagnostics"], json!([]));
        assert_eq!(
            invalid_open["params"]["diagnostics"]
                .as_array()
                .expect("invalid diagnostics")
                .len(),
            7
        );
        assert_eq!(
            state_open["params"]["diagnostics"]
                .as_array()
                .expect("state diagnostics")
                .len(),
            3
        );
        assert_eq!(
            legacy_open["params"]["diagnostics"][0]["code"],
            "syntax::legacy_global_decl"
        );
        assert_eq!(
            legacy_open["params"]["diagnostics"][0]["data"]["candidates"],
            json!([{"replacement":"state name: Type = expression;"},
                {"replacement":"extern state name: Type;"}])
        );

        let entire_valid = json!({"start":{"line":0,"character":0},
            "end":{"line":valid_document.text.lines().count(),"character":0}});
        assert_eq!(
            actions(&mut server, &valid, entire_valid, &valid_open),
            json!([])
        );
        for item in spec.oracle["duplicates"].as_array().expect("duplicates") {
            let marker = item["second"].as_str().expect("duplicate marker");
            assert_eq!(
                actions(
                    &mut server,
                    &invalid,
                    marker_range(invalid_document, marker),
                    &invalid_open
                ),
                json!([]),
                "duplicate {marker}, CRLF={crlf}"
            );
        }
        for item in spec.oracle["stateErrors"].as_array().expect("state errors") {
            assert_eq!(
                actions(
                    &mut server,
                    &state,
                    state_range(state_document, item),
                    &state_open
                ),
                json!([]),
                "state error {}, CRLF={crlf}",
                item["message"]
            );
        }
        assert_eq!(
            actions(
                &mut server,
                &legacy,
                marker_range(legacy_document, "legacy"),
                &legacy_open
            ),
            json!([]),
            "migration placeholders are not executable edits"
        );

        let fixed_invalid = repaired_duplicates(&spec, invalid_document);
        let line_ending = if crlf { "\r\n" } else { "\n" };
        for (target, text) in [
            (&invalid, fixed_invalid),
            (
                &state,
                spec.oracle["repairedState"]
                    .as_str()
                    .expect("state repair")
                    .replace('\n', line_ending),
            ),
            (
                &legacy,
                spec.oracle["repairedLegacy"]
                    .as_str()
                    .expect("legacy repair")
                    .replace('\n', line_ending),
            ),
        ] {
            let changed = sync_diagnostics::<n::DidChangeTextDocument>(
                &mut server,
                json!({"textDocument":{"uri":target,"version":2},"contentChanges":[{"text":text}]}),
            );
            assert_eq!(changed["params"]["diagnostics"], json!([]));
            let full = json!({"start":{"line":0,"character":0},
                "end":{"line":text.lines().count(),"character":0}});
            assert_eq!(
                actions(&mut server, target, full.clone(), &changed),
                json!([])
            );
            let mut fresh = initialized(&root);
            let fresh_open = open(&mut fresh, target, &text, 2);
            assert_eq!(
                fresh_open["params"]["diagnostics"],
                changed["params"]["diagnostics"]
            );
            assert_eq!(actions(&mut fresh, target, full, &fresh_open), json!([]));
        }
        for (target, prior) in [
            (&invalid, &invalid_open),
            (&state, &state_open),
            (&legacy, &legacy_open),
        ] {
            let closed = sync_diagnostics::<n::DidCloseTextDocument>(
                &mut server,
                json!({"textDocument":{"uri":target}}),
            );
            assert_eq!(
                closed["params"]["diagnostics"],
                prior["params"]["diagnostics"]
            );
        }
        fs::remove_dir_all(parent).expect("cleanup");
    }
}
