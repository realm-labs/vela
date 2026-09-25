use std::{fs, path::Path};

use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};

use crate::matrix_fixture::{Document, FixtureWorkspace, load, parse_markers};
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

fn whole(document: &Document) -> Value {
    json!({"start":{"line":0,"character":0},
        "end":{"line":document.text.lines().count(),"character":0}})
}

fn expected_action(uri: &str, version: Option<i32>, span: Value) -> Value {
    let edit = json!({"range":span,"newText":", extra: ()"});
    json!([{"title":"Add missing field `extra`","kind":"quickfix",
        "edit":{"changes":{uri.to_owned():[edit.clone()]},
            "documentChanges":[{"textDocument":{"uri":uri,"version":version},"edits":[edit]}]}}])
}

#[test]
fn member_and_constructor_actions_keep_utf16_edits_and_overlay_state() {
    let spec = load("diagnostic-action-members-constructors");
    for crlf in [false, true] {
        let mut fixture = FixtureWorkspace::new(&spec).expect("fixture");
        if crlf {
            for (name, document) in &mut fixture.disk {
                *document =
                    parse_markers(&spec.files[name].replace('\n', "\r\n")).expect("CRLF fixture");
            }
        }
        let parent = crate::tests::support::unique_temp_root("code-action-members");
        let root = parent.join("中文 % members");
        fixture.materialize(&root).expect("workspace");
        let valid_uri = uri(&root, "scripts/valid.vela");
        let invalid_uri = uri(&root, "scripts/invalid.vela");
        let incomplete_uri = uri(&root, "scripts/incomplete.vela");
        assert!(invalid_uri.contains('%'));
        let valid = &fixture.disk["scripts/valid.vela"];
        let invalid = &fixture.disk["scripts/invalid.vela"];
        let incomplete = &fixture.disk["scripts/incomplete.vela"];
        let insertion_marker = invalid.markers["missing-insert"];
        let line_start = invalid.text[..insertion_marker.start.byte]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        assert_ne!(
            insertion_marker.start.character,
            insertion_marker.start.byte - line_start,
            "the edit must cross a same-line Unicode UTF-16 boundary"
        );
        let mut server = initialized(&root);
        let valid_open = open(&mut server, &valid_uri, &valid.text, 1);
        let invalid_open = open(&mut server, &invalid_uri, &invalid.text, 1);
        let incomplete_open = open(&mut server, &incomplete_uri, &incomplete.text, 1);
        assert!(
            !incomplete_open["params"]["diagnostics"]
                .as_array()
                .expect("syntax diagnostics")
                .is_empty()
        );
        assert_eq!(
            actions(
                &mut server,
                &incomplete_uri,
                marker_range(incomplete, "incomplete"),
                &incomplete_open
            ),
            json!([])
        );
        assert_eq!(valid_open["params"]["diagnostics"], json!([]));
        assert_eq!(
            actions(&mut server, &valid_uri, whole(valid), &valid_open),
            json!([])
        );
        assert_eq!(
            invalid_open["params"]["diagnostics"],
            json!([{"range":marker_range(invalid,"missing"),"severity":1,
                "code":"analysis::missing_constructor_field",
                "message":"missing constructor field `extra` for `Box`",
                "source":"vela",
                "data":{"candidates":[],"repairHints":[],
                    "labels":[{"uri":invalid_uri,"range":marker_range(invalid,"missing"),
                        "message":"required field is not provided and has no default"}]}}])
        );
        for name in spec.oracle["negative"]
            .as_array()
            .expect("negative markers")
        {
            let name = name.as_str().expect("marker");
            assert_eq!(
                actions(
                    &mut server,
                    &invalid_uri,
                    marker_range(invalid, name),
                    &invalid_open
                ),
                json!([]),
                "no guessed action at {name}, CRLF={crlf}"
            );
        }
        let insertion = marker_range(invalid, "missing-insert");
        assert_eq!(
            actions(
                &mut server,
                &invalid_uri,
                marker_range(invalid, "missing"),
                &invalid_open
            ),
            expected_action(&invalid_uri, Some(1), insertion.clone())
        );
        let mut applied = invalid.text.clone();
        applied.insert_str(invalid.markers["missing-insert"].start.byte, ", extra: ()");
        let expected = if crlf {
            spec.oracle["applied"]
                .as_str()
                .expect("applied")
                .replace('\n', "\r\n")
        } else {
            spec.oracle["applied"].as_str().expect("applied").to_owned()
        };
        assert_eq!(applied, expected, "whole applied source, CRLF={crlf}");
        let applied_document = parse_markers(&applied).expect("applied source");
        let changed = sync_diagnostics::<n::DidChangeTextDocument>(
            &mut server,
            json!({"textDocument":{"uri":invalid_uri,"version":2},
                "contentChanges":[{"text":applied}]}),
        );
        assert_eq!(changed["params"]["diagnostics"], json!([]));
        assert_eq!(
            actions(
                &mut server,
                &invalid_uri,
                whole(&applied_document),
                &changed
            ),
            json!([])
        );
        let mut fresh = initialized(&root);
        let fresh_open = open(&mut fresh, &invalid_uri, &applied, 2);
        assert_eq!(
            fresh_open["params"]["diagnostics"],
            changed["params"]["diagnostics"]
        );
        assert_eq!(
            actions(
                &mut fresh,
                &invalid_uri,
                whole(&applied_document),
                &fresh_open
            ),
            json!([])
        );
        let closed = sync_diagnostics::<n::DidCloseTextDocument>(
            &mut server,
            json!({"textDocument":{"uri":invalid_uri}}),
        );
        assert_eq!(
            closed["params"]["diagnostics"],
            invalid_open["params"]["diagnostics"]
        );
        assert_eq!(
            actions(
                &mut server,
                &invalid_uri,
                marker_range(invalid, "missing"),
                &closed
            ),
            expected_action(&invalid_uri, None, insertion)
        );
        fs::remove_dir_all(parent).expect("cleanup");
    }
}
