use std::{fs, path::Path};

use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};

use crate::matrix_fixture::{Document, FixtureWorkspace, Spec, load, parse_markers};
use crate::tests::{TestServer, request, response_value, sync_diagnostics};

fn uri(root: &Path, file: &str) -> String {
    lsp_types::Url::from_file_path(root.join(file))
        .expect("valid top-level fixture")
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

fn marker_range(document: &Document, name: &str) -> Value {
    let marker = document.markers[name];
    json!({"start":{"line":marker.start.line,"character":marker.start.character},
        "end":{"line":marker.end.line,"character":marker.end.character}})
}

fn expected_duplicates(spec: &Spec, document: &Document, uri: &str) -> Value {
    json!(
        spec.oracle["duplicates"]
            .as_array()
            .expect("valid top-level fixture")
            .iter()
            .map(|item| {
                let first = marker_range(
                    document,
                    item["first"].as_str().expect("valid top-level fixture"),
                );
                let second = marker_range(
                    document,
                    item["second"].as_str().expect("valid top-level fixture"),
                );
                let kind = item["kind"].as_str().expect("valid top-level fixture");
                json!({"code":item["code"],"message":item["message"],"severity":1,
            "source":"vela","range":second,
            "data":{"labels":[
                {"uri":uri,"range":first,"message":format!("previous {kind} is here")},
                {"uri":uri,"range":second,"message":format!("duplicate {kind} is here")}
            ],"candidates":[],"repairHints":[]}})
            })
            .collect::<Vec<_>>()
    )
}

fn utf16_column(document: &Document, line: usize, byte_column: usize) -> usize {
    let text = document
        .text
        .split('\n')
        .nth(line)
        .expect("valid top-level fixture");
    text[..byte_column].encode_utf16().count()
}

fn expected_state(spec: &Spec, document: &Document) -> Value {
    json!(
        spec.oracle["stateErrors"]
            .as_array()
            .expect("valid top-level fixture")
            .iter()
            .map(|item| {
                let line = item["line"].as_u64().expect("valid top-level fixture") as usize;
                let start = utf16_column(
                    document,
                    line,
                    item["startByte"].as_u64().expect("valid top-level fixture") as usize,
                );
                let end = utf16_column(
                    document,
                    line,
                    item["endByte"].as_u64().expect("valid top-level fixture") as usize,
                );
                json!({"code":"E_PARSE","message":item["message"],"severity":1,
            "source":"vela","range":{"start":{"line":line,"character":start},
                "end":{"line":line,"character":end}},
            "data":{"labels":[],"candidates":[],"repairHints":[]}})
            })
            .collect::<Vec<_>>()
    )
}

fn repaired(spec: &Spec, document: &Document) -> String {
    let mut edits = spec.oracle["repairs"]
        .as_object()
        .expect("valid top-level fixture")
        .iter()
        .map(|(marker, value)| {
            let span = document.markers[marker];
            (
                span.start.byte,
                span.end.byte,
                value.as_str().expect("valid top-level fixture"),
            )
        })
        .collect::<Vec<_>>();
    edits.sort_by_key(|edit| edit.0);
    let mut text = document.text.clone();
    for (start, end, value) in edits.into_iter().rev() {
        text.replace_range(start..end, value);
    }
    text
}

#[test]
fn top_level_declaration_diagnostics_publish_utf16_and_restore_disk() {
    let spec = load("diagnostic-top-level-declarations");
    for crlf in [false, true] {
        let mut fixture = FixtureWorkspace::new(&spec).expect("valid top-level fixture");
        if crlf {
            for (file, document) in &mut fixture.disk {
                *document = parse_markers(&spec.files[file].replace('\n', "\r\n"))
                    .expect("valid top-level fixture");
            }
        }
        let parent = crate::tests::support::unique_temp_root("diagnostic-top-level");
        let root = parent.join("中文 % declarations");
        fixture.materialize(&root).expect("valid top-level fixture");
        let valid = uri(&root, "scripts/valid.vela");
        let invalid = uri(&root, "scripts/invalid.vela");
        let invalid_state = uri(&root, "scripts/invalid_state.vela");
        assert!(valid.contains('%') && invalid.contains('%'));
        let invalid_document = &fixture.disk["scripts/invalid.vela"];
        let state_document = &fixture.disk["scripts/invalid_state.vela"];
        let mut server = initialized(&root);
        assert_eq!(
            open(
                &mut server,
                &valid,
                &fixture.disk["scripts/valid.vela"].text,
                1
            )["params"]["diagnostics"],
            json!([])
        );
        let original_invalid = expected_duplicates(&spec, invalid_document, &invalid);
        let original_state = expected_state(&spec, state_document);
        assert_eq!(
            open(&mut server, &invalid, &invalid_document.text, 1)["params"]["diagnostics"],
            original_invalid,
            "CRLF={crlf}"
        );
        assert_eq!(
            open(&mut server, &invalid_state, &state_document.text, 1)["params"]["diagnostics"],
            original_state,
            "CRLF={crlf}"
        );
        let mut fresh = initialized(&root);
        assert_eq!(
            open(&mut fresh, &invalid, &invalid_document.text, 1)["params"]["diagnostics"],
            original_invalid
        );

        let repaired_invalid = repaired(&spec, invalid_document);
        let repaired_state = spec.oracle["repairedState"]
            .as_str()
            .expect("valid top-level fixture")
            .replace('\n', if crlf { "\r\n" } else { "\n" });
        for (target, text) in [
            (&invalid, &repaired_invalid),
            (&invalid_state, &repaired_state),
        ] {
            let changed = sync_diagnostics::<n::DidChangeTextDocument>(
                &mut server,
                json!({"textDocument":{"uri":target,"version":2},
                    "contentChanges":[{"text":text}]}),
            );
            assert_eq!(changed["params"]["diagnostics"], json!([]));
            let mut fresh = initialized(&root);
            assert_eq!(
                open(&mut fresh, target, text, 2)["params"]["diagnostics"],
                json!([])
            );
        }
        for (target, expected) in [
            (&invalid, original_invalid),
            (&invalid_state, original_state),
        ] {
            let closed = sync_diagnostics::<n::DidCloseTextDocument>(
                &mut server,
                json!({"textDocument":{"uri":target}}),
            );
            assert_eq!(closed["params"]["diagnostics"], expected);
        }
        fs::remove_dir_all(parent).expect("valid top-level fixture");
    }
}
