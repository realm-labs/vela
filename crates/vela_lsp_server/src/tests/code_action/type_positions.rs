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

fn actions(server: &mut TestServer, uri: &str, range: Value, publication: &Value) -> Value {
    response_value(request::<r::CodeActionRequest>(
        server,
        2,
        json!({"textDocument":{"uri":uri},"range":range,
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

fn expected_action(
    uri: &str,
    version: Option<i32>,
    title: &str,
    range: Value,
    replacement: &str,
) -> Value {
    let edit = json!({"range":range,"newText":replacement});
    json!([{"title":title,"kind":"quickfix",
        "edit":{"changes":{uri.to_owned():[edit.clone()]},
            "documentChanges":[{"textDocument":{"uri":uri,"version":version},"edits":[edit]}]}}])
}

fn repaired(spec: &Spec, document: &Document) -> String {
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
    for pair in edits.windows(2) {
        assert!(pair[0].1 <= pair[1].0, "independent repairs");
    }
    let mut text = document.text.clone();
    for (start, end, replacement) in edits.into_iter().rev() {
        text.replace_range(start..end, replacement);
    }
    text
}

#[test]
fn type_position_actions_project_safe_edit_and_reject_guesses() {
    let spec = load("diagnostic-type-positions");
    for crlf in [false, true] {
        let mut fixture = FixtureWorkspace::new(&spec).expect("fixture");
        if crlf {
            for (file, document) in &mut fixture.disk {
                *document =
                    crate::matrix_fixture::parse_markers(&spec.files[file].replace('\n', "\r\n"))
                        .expect("CRLF fixture");
            }
        }
        let parent = crate::tests::support::unique_temp_root("code-action-type-positions");
        let root = parent.join("中文 % types");
        fixture.materialize(&root).expect("workspace");
        let valid = uri(&root, "scripts/valid.vela");
        let invalid = uri(&root, "scripts/invalid.vela");
        assert!(valid.contains('%') && invalid.contains('%'));
        let valid_document = &fixture.disk["scripts/valid.vela"];
        let invalid_document = &fixture.disk["scripts/invalid.vela"];
        let mut server = initialized(&root);
        let valid_open = open(&mut server, &valid, &valid_document.text, 1);
        let invalid_open = open(&mut server, &invalid, &invalid_document.text, 1);
        assert_eq!(valid_open["params"]["diagnostics"], json!([]));
        assert_eq!(
            actions(&mut server, &valid, whole(valid_document), &valid_open),
            json!([])
        );
        let diagnostics = invalid_open["params"]["diagnostics"]
            .as_array()
            .expect("diagnostics");
        assert_eq!(diagnostics.len(), 12);
        let action_spec = &spec.oracle["codeAction"];
        let action_marker = action_spec["requestMarker"]
            .as_str()
            .expect("action marker");
        let edit_range = marker_range(
            invalid_document,
            action_spec["editMarker"].as_str().expect("edit marker"),
        );
        let title = action_spec["title"].as_str().expect("title");
        let replacement = action_spec["replacement"].as_str().expect("replacement");
        for item in spec.oracle["diagnostics"]
            .as_array()
            .expect("oracle diagnostics")
        {
            let name = item["marker"].as_str().expect("marker");
            let span = marker_range(invalid_document, name);
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic["range"] == span
                        && diagnostic["code"] == item["code"]),
                "UTF-16 diagnostic at {name}"
            );
            let actual = actions(&mut server, &invalid, span, &invalid_open);
            if name == action_marker {
                assert_eq!(
                    actual,
                    expected_action(&invalid, Some(1), title, edit_range.clone(), replacement),
                    "supported type fix, CRLF={crlf}"
                );
            } else {
                assert_eq!(actual, json!([]), "no guessed fix at {name}, CRLF={crlf}");
            }
        }
        let mut one_fixed = invalid_document.text.clone();
        let edit_marker =
            invalid_document.markers[action_spec["editMarker"].as_str().expect("edit marker")];
        one_fixed.replace_range(edit_marker.start.byte..edit_marker.end.byte, replacement);
        assert_eq!(
            one_fixed,
            invalid_document.text.replace("Cell<i64>", "Cell")
        );
        let one_changed = sync_diagnostics::<n::DidChangeTextDocument>(
            &mut server,
            json!({"textDocument":{"uri":invalid,"version":2},
                "contentChanges":[{"text":one_fixed}]}),
        );
        let one_diagnostics = one_changed["params"]["diagnostics"]
            .as_array()
            .expect("one-fixed diagnostics");
        let expected_remaining = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic["code"] != "syntax::generic_type_hint")
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(expected_remaining.len(), 11);
        assert_eq!(one_diagnostics, &expected_remaining);
        let one_document =
            crate::matrix_fixture::parse_markers(&one_fixed).expect("one-fixed source");
        assert_eq!(
            actions(&mut server, &invalid, whole(&one_document), &one_changed),
            json!([])
        );
        let mut one_fresh = initialized(&root);
        let one_fresh_open = open(&mut one_fresh, &invalid, &one_fixed, 2);
        assert_eq!(
            one_fresh_open["params"]["diagnostics"],
            one_changed["params"]["diagnostics"]
        );
        assert_eq!(
            actions(
                &mut one_fresh,
                &invalid,
                whole(&one_document),
                &one_fresh_open
            ),
            json!([])
        );
        let repaired = repaired(&spec, invalid_document);
        assert!(
            vela_syntax::parse::parse_source(&repaired)
                .diagnostics()
                .is_empty()
        );
        let changed = sync_diagnostics::<n::DidChangeTextDocument>(
            &mut server,
            json!({"textDocument":{"uri":invalid,"version":3},
                "contentChanges":[{"text":repaired}]}),
        );
        assert_eq!(changed["params"]["diagnostics"], json!([]));
        let repaired_document = crate::matrix_fixture::parse_markers(&repaired).expect("repaired");
        assert_eq!(
            actions(&mut server, &invalid, whole(&repaired_document), &changed),
            json!([])
        );
        let mut fresh = initialized(&root);
        let fresh_open = open(&mut fresh, &invalid, &repaired, 3);
        assert_eq!(
            fresh_open["params"]["diagnostics"],
            changed["params"]["diagnostics"]
        );
        assert_eq!(
            actions(&mut fresh, &invalid, whole(&repaired_document), &fresh_open),
            json!([])
        );

        let closed = sync_diagnostics::<n::DidCloseTextDocument>(
            &mut server,
            json!({"textDocument":{"uri":invalid}}),
        );
        assert_eq!(
            closed["params"]["diagnostics"],
            invalid_open["params"]["diagnostics"]
        );
        for item in spec.oracle["diagnostics"]
            .as_array()
            .expect("oracle diagnostics")
        {
            let span = marker_range(invalid_document, item["marker"].as_str().expect("marker"));
            let actual = actions(&mut server, &invalid, span, &closed);
            if item["marker"] == action_spec["requestMarker"] {
                assert_eq!(
                    actual,
                    expected_action(&invalid, None, title, edit_range.clone(), replacement)
                );
            } else {
                assert_eq!(actual, json!([]));
            }
        }
        let incomplete_document = crate::matrix_fixture::parse_markers(
            "/* 中😀 */ fn broken(value: Cell[[open:start]]<[[open:end]]i64) {}",
        )
        .expect("incomplete source");
        let incomplete = uri(&root, "scripts/incomplete.vela");
        let incomplete_open = open(&mut server, &incomplete, &incomplete_document.text, 1);
        assert!(
            incomplete_open["params"]["diagnostics"]
                .as_array()
                .expect("incomplete diagnostics")
                .iter()
                .any(
                    |diagnostic| diagnostic["code"] == "syntax::generic_type_hint"
                        && diagnostic["data"]["repairHints"] == json!([])
                )
        );
        assert_eq!(
            actions(
                &mut server,
                &incomplete,
                marker_range(&incomplete_document, "open"),
                &incomplete_open,
            ),
            json!([]),
            "unclosed type arguments have no removal edit"
        );
        let incomplete_close = sync_diagnostics::<n::DidCloseTextDocument>(
            &mut server,
            json!({"textDocument":{"uri":incomplete}}),
        );
        assert_eq!(incomplete_close["params"]["diagnostics"], json!([]));
        fs::remove_dir_all(parent).expect("cleanup");
    }
}
