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

fn expected_action(
    uri: &str,
    version: Option<i32>,
    item: &Value,
    document: &Document,
    crlf: bool,
) -> Value {
    let insertion = item["insert"].as_str().expect("insertion marker");
    let replacement = item["replacement"].as_str().expect("replacement");
    let replacement = if crlf {
        replacement.replace('\n', "\r\n")
    } else {
        replacement.to_owned()
    };
    let edit = json!({"range":marker_range(document, insertion),"newText":replacement});
    json!([{"title":item["title"],"kind":"quickfix",
        "edit":{"changes":{uri.to_owned():[edit.clone()]},
            "documentChanges":[{"textDocument":{"uri":uri,"version":version},"edits":[edit]}]}}])
}

fn expected_diagnostics(uri: &str, document: &Document, positive: &[Value]) -> Value {
    let owners = ["Option", "Result", "Option"];
    let missing = ["None", "Ok", "Some"];
    Value::Array(
        positive
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let span = marker_range(document, item["marker"].as_str().expect("marker"));
                json!({"range":span,"severity":2,"code":"analysis::non_exhaustive_match",
            "message":format!("match on `{}` does not cover all known variants",owners[index]),
            "source":"vela","data":{"candidates":[],"repairHints":[],"labels":[{
                "uri":uri,"range":span,"message":format!("missing variants: {}",missing[index])}]}})
            })
            .collect(),
    )
}

#[test]
fn control_flow_actions_use_complete_utf16_edits_and_clear_on_apply() {
    let spec = load("diagnostic-action-control-flow");
    let positive = spec.oracle["positive"]
        .as_array()
        .expect("positive actions");
    for crlf in [false, true] {
        let mut fixture = FixtureWorkspace::new(&spec).expect("fixture");
        if crlf {
            for (name, document) in &mut fixture.disk {
                *document =
                    parse_markers(&spec.files[name].replace('\n', "\r\n")).expect("CRLF fixture");
            }
        }
        let parent = crate::tests::support::unique_temp_root("code-action-control-flow");
        let root = parent.join("中文 % control flow");
        fixture.materialize(&root).expect("workspace");
        let valid_uri = uri(&root, "scripts/valid.vela");
        let invalid_uri = uri(&root, "scripts/invalid.vela");
        let inline_uri = uri(&root, "scripts/inline.vela");
        let incomplete_uri = uri(&root, "scripts/incomplete.vela");
        assert!(invalid_uri.contains('%'));
        let valid = &fixture.disk["scripts/valid.vela"];
        let invalid = &fixture.disk["scripts/invalid.vela"];
        let inline = &fixture.disk["scripts/inline.vela"];
        let incomplete = &fixture.disk["scripts/incomplete.vela"];
        let first_marker = invalid.markers["option"];
        let line_start = invalid.text[..first_marker.start.byte]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        assert_ne!(
            first_marker.start.character,
            first_marker.start.byte - line_start
        );

        let mut server = initialized(&root);
        let valid_open = open(&mut server, &valid_uri, &valid.text, 1);
        let invalid_open = open(&mut server, &invalid_uri, &invalid.text, 1);
        let inline_open = open(&mut server, &inline_uri, &inline.text, 1);
        let incomplete_open = open(&mut server, &incomplete_uri, &incomplete.text, 1);
        assert_eq!(valid_open["params"]["diagnostics"], json!([]));
        assert_eq!(
            actions(&mut server, &valid_uri, whole(valid), &valid_open),
            json!([])
        );
        assert_eq!(
            invalid_open["params"]["diagnostics"],
            expected_diagnostics(&invalid_uri, invalid, positive)
        );
        for (name, uri, document, publication) in [
            ("inline", &inline_uri, inline, &inline_open),
            ("incomplete", &incomplete_uri, incomplete, &incomplete_open),
        ] {
            assert!(
                publication["params"]["diagnostics"]
                    .as_array()
                    .expect("diagnostics")
                    .iter()
                    .any(|diagnostic| diagnostic["code"] == "analysis::non_exhaustive_match")
            );
            assert_eq!(
                actions(&mut server, uri, marker_range(document, name), publication),
                json!([]),
                "no unsafe edit at {name}, CRLF={crlf}"
            );
        }
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
                "no action at {name}, CRLF={crlf}"
            );
        }

        let mut edits = Vec::new();
        for (index, item) in positive.iter().enumerate() {
            let name = item["marker"].as_str().expect("marker");
            let result = actions(
                &mut server,
                &invalid_uri,
                marker_range(invalid, name),
                &invalid_open,
            );
            assert_eq!(
                result,
                expected_action(&invalid_uri, Some(1), item, invalid, crlf),
                "exact versioned action at {name}, CRLF={crlf}"
            );
            let insertion = item["insert"].as_str().expect("insert marker");
            let offset = invalid.markers[insertion].start.byte;
            let replacement = result[0]["edit"]["changes"][&invalid_uri][0]["newText"]
                .as_str()
                .expect("replacement")
                .to_owned();
            edits.push((offset, replacement.clone()));
            let mut one_fixed = invalid.text.clone();
            one_fixed.insert_str(offset, &replacement);
            let expected = spec.oracle["applied"][name]
                .as_str()
                .expect("whole-source oracle");
            assert_eq!(
                one_fixed,
                if crlf {
                    expected.replace('\n', "\r\n")
                } else {
                    expected.to_owned()
                },
                "whole applied source at {name}"
            );
            let mut fresh = initialized(&root);
            let after = open(&mut fresh, &invalid_uri, &one_fixed, 2);
            let remaining = after["params"]["diagnostics"]
                .as_array()
                .expect("diagnostics");
            assert_eq!(remaining.len(), 2);
            let expected_messages = ["Option", "Result", "Option"]
                .iter()
                .enumerate()
                .filter(|(other, _)| *other != index)
                .map(|(_, owner)| format!("match on `{owner}` does not cover all known variants"))
                .collect::<Vec<_>>();
            assert_eq!(
                remaining
                    .iter()
                    .map(|item| item["message"].as_str().expect("message"))
                    .collect::<Vec<_>>(),
                expected_messages
            );
            let one_document = parse_markers(&one_fixed).expect("applied source");
            assert_eq!(
                actions(&mut fresh, &invalid_uri, whole(&one_document), &after)
                    .as_array()
                    .expect("remaining actions")
                    .len(),
                2
            );
        }

        let mut all_fixed = invalid.text.clone();
        edits.sort_by_key(|edit| edit.0);
        for (offset, replacement) in edits.into_iter().rev() {
            all_fixed.insert_str(offset, &replacement);
        }
        let expected = spec.oracle["applied"]["all"]
            .as_str()
            .expect("all-fixes oracle");
        assert_eq!(
            all_fixed,
            if crlf {
                expected.replace('\n', "\r\n")
            } else {
                expected.to_owned()
            }
        );
        let changed = sync_diagnostics::<n::DidChangeTextDocument>(
            &mut server,
            json!({"textDocument":{"uri":invalid_uri,"version":2},
                "contentChanges":[{"text":all_fixed}]}),
        );
        assert_eq!(changed["params"]["diagnostics"], json!([]));
        let repaired = parse_markers(&all_fixed).expect("repaired source");
        assert_eq!(
            actions(&mut server, &invalid_uri, whole(&repaired), &changed),
            json!([])
        );
        let mut fresh = initialized(&root);
        let fresh_open = open(&mut fresh, &invalid_uri, &all_fixed, 2);
        assert_eq!(
            fresh_open["params"]["diagnostics"],
            changed["params"]["diagnostics"]
        );
        assert_eq!(
            actions(&mut fresh, &invalid_uri, whole(&repaired), &fresh_open),
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
        for item in positive {
            let name = item["marker"].as_str().expect("marker");
            assert_eq!(
                actions(
                    &mut server,
                    &invalid_uri,
                    marker_range(invalid, name),
                    &closed
                ),
                expected_action(&invalid_uri, None, item, invalid, crlf)
            );
        }
        fs::remove_dir_all(parent).expect("cleanup");
    }
}

#[test]
fn schema_match_action_projects_tuple_and_record_patterns() {
    let spec = load("diagnostic-action-schema-match");
    for crlf in [false, true] {
        let mut fixture = FixtureWorkspace::new(&spec).expect("fixture");
        if crlf {
            for (name, document) in &mut fixture.disk {
                if name.ends_with(".vela") {
                    *document = parse_markers(&spec.files[name].replace('\n', "\r\n"))
                        .expect("CRLF fixture");
                }
            }
        }
        let parent = crate::tests::support::unique_temp_root("code-action-schema-match");
        let root = parent.join("中文 % schema match");
        fixture.materialize(&root).expect("workspace");
        let valid_uri = uri(&root, "scripts/valid.vela");
        let invalid_uri = uri(&root, "scripts/invalid.vela");
        assert!(invalid_uri.contains('%'));
        let valid = &fixture.disk["scripts/valid.vela"];
        let invalid = &fixture.disk["scripts/invalid.vela"];
        let marker = invalid.markers["match"];
        let line_start = invalid.text[..marker.start.byte]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        assert_ne!(marker.start.character, marker.start.byte - line_start);
        let mut server = initialized(&root);
        let valid_open = open(&mut server, &valid_uri, &valid.text, 1);
        let invalid_open = open(&mut server, &invalid_uri, &invalid.text, 1);
        assert_eq!(valid_open["params"]["diagnostics"], json!([]));
        assert_eq!(
            actions(&mut server, &valid_uri, whole(valid), &valid_open),
            json!([])
        );
        let match_span = marker_range(invalid, "match");
        assert_eq!(
            invalid_open["params"]["diagnostics"],
            json!([{
                "range":match_span,"severity":2,"code":"analysis::non_exhaustive_match",
                "message":"match on `State` does not cover all known variants","source":"vela",
                "data":{"candidates":[],"repairHints":[],"labels":[{
                    "uri":invalid_uri,"range":match_span,"message":"missing variants: Named, Pair"}]}
            }])
        );
        let title = spec.oracle["title"].as_str().expect("title");
        let replacement = spec.oracle["replacement"].as_str().expect("replacement");
        let replacement = if crlf {
            replacement.replace('\n', "\r\n")
        } else {
            replacement.to_owned()
        };
        let insertion = marker_range(invalid, "insert");
        let expected_action = |version: Option<i32>| {
            let edit = json!({"range":insertion,"newText":replacement});
            json!([{"title":title,"kind":"quickfix",
                "edit":{"changes":{invalid_uri.to_owned():[edit.clone()]},
                    "documentChanges":[{"textDocument":{"uri":invalid_uri,"version":version},
                        "edits":[edit]}]}}])
        };
        assert_eq!(
            actions(&mut server, &invalid_uri, match_span, &invalid_open),
            expected_action(Some(1))
        );
        let mut applied = invalid.text.clone();
        applied.insert_str(invalid.markers["insert"].start.byte, &replacement);
        let expected = spec.oracle["applied"]
            .as_str()
            .expect("whole-source oracle");
        assert_eq!(
            applied,
            if crlf {
                expected.replace('\n', "\r\n")
            } else {
                expected.to_owned()
            }
        );
        assert!(
            vela_syntax::parse::parse_source(&applied)
                .diagnostics()
                .is_empty()
        );
        let changed = sync_diagnostics::<n::DidChangeTextDocument>(
            &mut server,
            json!({"textDocument":{"uri":invalid_uri,"version":2},
                "contentChanges":[{"text":applied}]}),
        );
        assert_eq!(changed["params"]["diagnostics"], json!([]));
        let repaired = parse_markers(&applied).expect("repaired source");
        assert_eq!(
            actions(&mut server, &invalid_uri, whole(&repaired), &changed),
            json!([])
        );
        let mut fresh = initialized(&root);
        let fresh_open = open(&mut fresh, &invalid_uri, &applied, 2);
        assert_eq!(
            fresh_open["params"]["diagnostics"],
            changed["params"]["diagnostics"]
        );
        assert_eq!(
            actions(&mut fresh, &invalid_uri, whole(&repaired), &fresh_open),
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
                marker_range(invalid, "match"),
                &closed
            ),
            expected_action(None)
        );
        fs::remove_dir_all(parent).expect("cleanup");
    }
}
