use std::{fs, path::Path};

use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};

use crate::matrix_fixture::{Document, FixtureWorkspace, Marker, load, parse_markers};
use crate::tests::{
    TestServer, notification_values, notify, request, response_value, sync_diagnostics,
};

const MAIN: &str = "scripts/game/main.vela";
const REWARD: &str = "scripts/game/reward.vela";

fn uri(root: &Path, file: &str) -> String {
    lsp_types::Url::from_file_path(root.join(file))
        .expect("file URI")
        .to_string()
}

fn range(marker: Marker) -> Value {
    json!({"start":{"line":marker.start.line,"character":marker.start.character},
        "end":{"line":marker.end.line,"character":marker.end.character}})
}

fn start(root: &Path, document: &Document, version: i32) -> (TestServer, Value) {
    let mut server = TestServer::new();
    let _ = response_value(request::<r::Initialize>(
        &mut server,
        1,
        json!({"processId":null,"rootUri":uri(root,""),"capabilities":{}}),
    ));
    let publication = sync_diagnostics::<n::DidOpenTextDocument>(
        &mut server,
        json!({"textDocument":{"uri":uri(root,MAIN),
            "languageId":"vela","version":version,"text":document.text}}),
    );
    (server, publication)
}

fn projection(publication: &Value) -> Value {
    json!(
        publication["params"]["diagnostics"]
            .as_array()
            .expect("published diagnostics")
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

fn expected_diagnostics(document: &Document, fixed: Option<&str>) -> Value {
    let mut expected = Vec::new();
    for name in ["grant", "award"] {
        if fixed == Some(name) {
            continue;
        }
        expected.push(json!({
            "code":"hir::unresolved_name", "message":format!("unresolved name `{name}`"),
            "severity":1, "range":range(document.markers[name]), "candidates":[]
        }));
    }
    expected.push(json!({
        "code":"analysis::unknown_method",
        "message":"unknown method `frist` for `Array(i64)`",
        "severity":1, "range":range(document.markers["control"]),
        "candidates":["first", "find", "last"]
    }));
    json!(expected)
}

fn actions(
    server: &mut TestServer,
    root: &Path,
    document: &Document,
    publication: &Value,
    marker: &str,
    version: i32,
    available: Option<&str>,
) -> Value {
    let file = uri(root, MAIN);
    let response = response_value(request::<r::CodeActionRequest>(
        server,
        2,
        json!({"textDocument":{"uri":file}, "range":range(document.markers[marker]),
            "context":{"diagnostics":publication["params"]["diagnostics"]}}),
    ));
    let actual = response["result"].as_array().expect("code action array");
    let expected: Vec<(&str, String, Value)> = if marker == "control" {
        ["first", "find", "last"]
            .into_iter()
            .map(|replacement| {
                (
                    replacement,
                    format!("Replace with `{replacement}`"),
                    range(document.markers[marker]),
                )
            })
            .collect()
    } else if available == Some(marker) {
        vec![(
            marker,
            format!("Import `game::reward::{marker}`"),
            json!({"start":{"line":0,"character":0},"end":{"line":0,"character":0}}),
        )]
    } else {
        Vec::new()
    };
    assert_eq!(actual.len(), expected.len(), "{marker} action count");
    for (action, (replacement, title, edit_range)) in actual.iter().zip(expected) {
        assert_eq!(action["title"], title);
        assert_eq!(action["kind"], "quickfix");
        assert_eq!(
            action["edit"]["changes"]
                .as_object()
                .expect("changes")
                .len(),
            1
        );
        let new_text = if marker == "control" {
            replacement.to_owned()
        } else {
            format!("use game::reward::{replacement}\n")
        };
        let edit = json!({"range":edit_range,"newText":new_text});
        assert_eq!(action["edit"]["changes"][&file], json!([edit]));
        assert_eq!(
            action["edit"]["documentChanges"],
            json!([{"textDocument":{"uri":file,"version":version},"edits":[edit]}])
        );
    }
    response["result"].clone()
}

fn check_phase(
    state: &Value,
    root: &Path,
    document: &Document,
    version: i32,
    live: &mut TestServer,
    publication: &Value,
    applied: Option<&Document>,
) {
    let id = state["id"].as_str().expect("phase id");
    let available = state["available"].as_str();
    assert_eq!(
        publication["params"]["uri"],
        uri(root, MAIN),
        "importer publication {id}"
    );
    assert_eq!(
        projection(publication),
        expected_diagnostics(document, None),
        "diagnostics {id}"
    );
    let mut observed = Vec::new();
    for marker in ["grant", "award", "control"] {
        observed.push(actions(
            live,
            root,
            document,
            publication,
            marker,
            version,
            available,
        ));
    }

    let (mut fresh, fresh_publication) = start(root, document, version);
    assert_eq!(
        publication["params"]["diagnostics"], fresh_publication["params"]["diagnostics"],
        "fresh diagnostics {id}"
    );
    for (offset, marker) in ["grant", "award", "control"].iter().enumerate() {
        assert_eq!(
            observed[offset],
            actions(
                &mut fresh,
                root,
                document,
                &fresh_publication,
                marker,
                version,
                available,
            ),
            "fresh actions {id} {marker}"
        );
    }

    if let (Some(marker), Some(applied_document)) = (available, applied) {
        let action_index = if marker == "grant" { 0 } else { 1 };
        let edit = &observed[action_index][0]["edit"]["changes"][&uri(root, MAIN)][0];
        let new_text = edit["newText"].as_str().expect("import text");
        let applied_text = format!("{new_text}{}", document.text);
        assert_eq!(
            applied_text, applied_document.text,
            "whole imported source {id}"
        );
        let changed = sync_diagnostics::<n::DidChangeTextDocument>(
            &mut fresh,
            json!({"textDocument":{"uri":uri(root,MAIN),"version":version+1},
                "contentChanges":[{"text":applied_text}]}),
        );
        assert_eq!(
            projection(&changed),
            expected_diagnostics(applied_document, Some(marker)),
            "applied diagnostics {id}"
        );
        assert_eq!(
            actions(
                &mut fresh,
                root,
                applied_document,
                &changed,
                marker,
                version + 1,
                None,
            ),
            json!([]),
            "no stale import action {id}"
        );
        let other = if marker == "grant" { "award" } else { "grant" };
        assert_eq!(
            actions(
                &mut fresh,
                root,
                applied_document,
                &changed,
                other,
                version + 1,
                Some(marker),
            ),
            json!([]),
            "other missing name remains unimportable {id}"
        );
        let _ = actions(
            &mut fresh,
            root,
            applied_document,
            &changed,
            "control",
            version + 1,
            Some(marker),
        );
    }
}

#[test]
fn dependency_lifecycle_updates_diagnostics_and_import_actions() {
    for crlf in [false, true] {
        let mut spec = load("diagnostic-action-dependency-lifecycle");
        if crlf {
            for source in spec.files.values_mut() {
                *source = source.replace('\n', "\r\n");
            }
        }
        let fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let parent = crate::tests::support::unique_temp_root("diagnostic-action-dependency");
        let root = parent.join("中文 % dependency");
        fixture.materialize(&root).expect("workspace");
        let disk = &fixture.disk[MAIN];
        let line_endings = |key: &str| {
            let marked = spec.oracle[key].as_str().expect("oracle source");
            parse_markers(&if crlf {
                if key.starts_with("applied") {
                    let (import, body) = marked.split_once('\n').expect("import line");
                    format!("{import}\n{}", body.replace('\n', "\r\n"))
                } else {
                    marked.replace('\n', "\r\n")
                }
            } else {
                marked.to_owned()
            })
            .expect("oracle markers")
        };
        let dirty = line_endings("dirty");
        let applied_grant = line_endings("appliedGrant");
        let applied_award = line_endings("appliedAward");
        let states = spec.oracle["states"].as_array().expect("states");
        let reward = root.join(REWARD);

        let (mut live, opened) = start(&root, disk, 1);
        check_phase(
            states.first().expect("disk state"),
            &root,
            disk,
            1,
            &mut live,
            &opened,
            None,
        );
        let changed = sync_diagnostics::<n::DidChangeTextDocument>(
            &mut live,
            json!({"textDocument":{"uri":uri(&root,MAIN),"version":2},
                "contentChanges":[{"text":dirty.text}]}),
        );
        check_phase(
            &states[1],
            &root,
            &dirty,
            2,
            &mut live,
            &changed,
            Some(&applied_grant),
        );
        assert_eq!(
            fs::read_to_string(root.join(MAIN)).expect("disk importer"),
            disk.text
        );

        for (index, kind) in [2, 3, 1].into_iter().enumerate() {
            if kind == 3 {
                fs::remove_file(&reward).expect("delete dependency");
            } else {
                let key = if kind == 2 {
                    "changedDependency"
                } else {
                    "recreatedDependency"
                };
                let source = spec.oracle[key].as_str().expect("dependency source");
                fs::write(
                    &reward,
                    if crlf {
                        source.replace('\n', "\r\n")
                    } else {
                        source.to_owned()
                    },
                )
                .expect("write dependency");
            }
            let messages = notification_values(notify::<n::DidChangeWatchedFiles>(
                &mut live,
                json!({"changes":[{"uri":uri(&root,REWARD),"type":kind}]}),
            ));
            let publication = messages
                .iter()
                .find(|message| {
                    message["method"] == "textDocument/publishDiagnostics"
                        && message["params"]["uri"] == uri(&root, MAIN)
                })
                .expect("dirty importer publication");
            let state = &states[index + 2];
            let applied = match state["available"].as_str() {
                Some("grant") => Some(&applied_grant),
                Some("award") => Some(&applied_award),
                None => None,
                Some(other) => panic!("unsupported import {other}"),
            };
            check_phase(state, &root, &dirty, 2, &mut live, publication, applied);
            assert_eq!(
                fs::read_to_string(root.join(MAIN)).expect("disk importer"),
                disk.text
            );
        }
        fs::remove_dir_all(parent).expect("cleanup");
    }
}
