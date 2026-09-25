use std::{fs, path::Path};

use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};

use crate::matrix_fixture::{Document, FixtureWorkspace, Spec, load, parse_markers};
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

fn expected(spec: &Spec, document: &Document, uri: &str) -> Value {
    json!(
        spec.oracle["diagnostics"]
            .as_array()
            .expect("oracle diagnostics")
            .iter()
            .map(|item| {
                let marker = document.markers[item["marker"].as_str().expect("marker")];
                let span = json!({
                    "start":{"line":marker.start.line,"character":marker.start.character},
                    "end":{"line":marker.end.line,"character":marker.end.character}
                });
                json!({"code":item["code"],"message":item["message"],
                "severity":1,"source":"vela","range":span,
                "data":{"labels":[{"uri":uri,"range":span,"message":item["label"]}],
                    "candidates":[],"repairHints":[]}})
            })
            .collect::<Vec<_>>()
    )
}

fn repaired(spec: &Spec, document: &Document) -> String {
    let mut edits = spec.oracle["repairs"]
        .as_object()
        .expect("repairs")
        .iter()
        .map(|(marker, replacement)| {
            let span = document.markers[marker];
            (
                span.start.byte,
                span.end.byte,
                replacement.as_str().expect("replacement"),
            )
        })
        .collect::<Vec<_>>();
    edits.sort_by_key(|edit| edit.0);
    for pair in edits.windows(2) {
        assert!(pair[0].1 <= pair[1].0, "independent repairs do not overlap");
    }
    let mut text = document.text.clone();
    for (start, end, replacement) in edits.into_iter().rev() {
        text.replace_range(start..end, replacement);
    }
    text
}

fn open(server: &mut TestServer, uri: &str, text: &str, version: i32) -> Value {
    sync_diagnostics::<n::DidOpenTextDocument>(
        server,
        json!({"textDocument":{"uri":uri,"languageId":"vela","version":version,"text":text}}),
    )
}

#[test]
fn type_hint_diagnostics_project_utf16_and_clear_after_all_repairs() {
    let spec = load("diagnostic-type-positions");
    for crlf in [false, true] {
        let mut fixture = FixtureWorkspace::new(&spec).expect("fixture");
        if crlf {
            for (file, document) in &mut fixture.disk {
                *document =
                    parse_markers(&spec.files[file].replace('\n', "\r\n")).expect("CRLF source");
            }
        }
        let parent = crate::tests::support::unique_temp_root("diagnostic-type-positions");
        let root = parent.join("中文 % types");
        fixture.materialize(&root).expect("workspace");
        let valid = uri(&root, "scripts/valid.vela");
        let invalid = uri(&root, "scripts/invalid.vela");
        assert!(valid.contains('%') && invalid.contains('%'));
        let invalid_document = &fixture.disk["scripts/invalid.vela"];
        let mut server = initialized(&root);
        let valid_open = open(
            &mut server,
            &valid,
            &fixture.disk["scripts/valid.vela"].text,
            1,
        );
        assert_eq!(valid_open["params"]["diagnostics"], json!([]));
        let invalid_open = open(&mut server, &invalid, &invalid_document.text, 1);
        assert_eq!(
            invalid_open["params"]["diagnostics"],
            expected(&spec, invalid_document, &invalid)
        );
        let mut fresh = initialized(&root);
        let fresh_invalid = open(&mut fresh, &invalid, &invalid_document.text, 1);
        assert_eq!(
            invalid_open["params"]["diagnostics"],
            fresh_invalid["params"]["diagnostics"]
        );

        let repaired = repaired(&spec, invalid_document);
        assert!(
            vela_syntax::parse::parse_source(&repaired)
                .diagnostics()
                .is_empty()
        );
        let changed = sync_diagnostics::<n::DidChangeTextDocument>(
            &mut server,
            json!({"textDocument":{"uri":invalid,"version":2},
                "contentChanges":[{"text":repaired}]}),
        );
        assert_eq!(changed["params"]["diagnostics"], json!([]));
        let mut fresh = initialized(&root);
        let fresh_repaired = open(&mut fresh, &invalid, &repaired, 2);
        assert_eq!(
            changed["params"]["diagnostics"],
            fresh_repaired["params"]["diagnostics"]
        );

        let close = sync_diagnostics::<n::DidCloseTextDocument>(
            &mut server,
            json!({"textDocument":{"uri":invalid}}),
        );
        assert_eq!(
            close["params"]["diagnostics"],
            expected(&spec, invalid_document, &invalid),
            "closing dirty type fixes restores disk diagnostics"
        );
        let reopen = open(&mut server, &invalid, &repaired, 3);
        assert_eq!(reopen["params"]["diagnostics"], json!([]));
        fs::remove_dir_all(parent).expect("cleanup");
    }
}
