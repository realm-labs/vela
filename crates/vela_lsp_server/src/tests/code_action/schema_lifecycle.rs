use std::{fs, path::Path};

use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};

use crate::matrix_fixture::{Document, FixtureWorkspace, Marker, load, parse_markers};
use crate::tests::{
    TestServer, notification_values, notify, request, response_value, sync_diagnostics,
};

fn uri(root: &Path, file: &str) -> String {
    lsp_types::Url::from_file_path(root.join(file))
        .expect("file URI")
        .to_string()
}

fn range(marker: Marker) -> Value {
    json!({
        "start": {"line": marker.start.line, "character": marker.start.character},
        "end": {"line": marker.end.line, "character": marker.end.character}
    })
}

fn start(root: &Path, document: &Document) -> (TestServer, Value) {
    let mut server = TestServer::new();
    let _ = response_value(request::<r::Initialize>(
        &mut server,
        1,
        json!({"processId": null, "rootUri": uri(root, ""), "capabilities": {}}),
    ));
    let opened = sync_diagnostics::<n::DidOpenTextDocument>(
        &mut server,
        json!({"textDocument": {"uri": uri(root, "scripts/main.vela"),
            "languageId": "vela", "version": 1, "text": document.text}}),
    );
    (server, opened)
}

fn projection(publication: &Value) -> Value {
    let diagnostics = publication["params"]["diagnostics"]
        .as_array()
        .expect("diagnostic publication");
    json!(
        diagnostics
            .iter()
            .map(|item| json!({
                "code": item["code"],
                "message": item["message"],
                "severity": item["severity"],
                "range": item["range"],
                "candidates": item["data"]["candidates"].as_array().expect("candidates")
                    .iter().map(|candidate| candidate["replacement"].clone()).collect::<Vec<_>>()
            }))
            .collect::<Vec<_>>()
    )
}

fn expected_diagnostics(
    document: &Document,
    schema: &Path,
    state: &Value,
    fixed: Option<&str>,
) -> Value {
    let mut expected = vec![json!({
        "code": "analysis::unknown_method",
        "message": "unknown method `frist` for `Array(i64)`",
        "severity": 1,
        "range": range(document.markers["control"]),
        "candidates": ["first", "find", "last"]
    })];
    if let Some(field) = state["field"].as_str() {
        for (marker, typo) in [("old", "levle"), ("new", "rnak")] {
            if fixed == Some(marker) {
                continue;
            }
            expected.push(json!({
                "code": "analysis::unknown_field",
                "message": format!("unknown field `{typo}` for `Player`"),
                "severity": 1,
                "range": range(document.markers[marker]),
                "candidates": [field]
            }));
        }
    } else {
        let warning = state["warning"].as_str().expect("schema warning");
        expected.push(json!({
            "code": "schema::unavailable",
            "message": format!("host schema `{}` {warning}", schema.display().to_string().replace('\\', "/")),
            "severity": 2,
            "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}},
            "candidates": []
        }));
    }
    json!(expected)
}

fn actions(
    server: &mut TestServer,
    root: &Path,
    document: &Document,
    publication: &Value,
    marker: &str,
    id: i32,
    replacements: &[&str],
) -> Value {
    let file = uri(root, "scripts/main.vela");
    let target = range(document.markers[marker]);
    let response = response_value(request::<r::CodeActionRequest>(
        server,
        id,
        json!({"textDocument": {"uri": file}, "range": target,
            "context": {"diagnostics": publication["params"]["diagnostics"]}}),
    ));
    let actual = response["result"].as_array().expect("action array");
    assert_eq!(actual.len(), replacements.len(), "{marker} action count");
    for (action, replacement) in actual.iter().zip(replacements) {
        assert_eq!(action["title"], format!("Replace with `{replacement}`"));
        assert_eq!(action["kind"], "quickfix");
        assert_eq!(
            action["edit"]["changes"]
                .as_object()
                .expect("changes")
                .len(),
            1
        );
        let edit = json!({"range": target, "newText": replacement});
        assert_eq!(action["edit"]["changes"][&file], json!([edit]));
        assert_eq!(
            action["edit"]["documentChanges"],
            json!([{
                "textDocument": {"uri": file, "version": 1}, "edits": [edit]
            }])
        );
    }
    response["result"].clone()
}

fn expected_replacements<'a>(state: &'a Value, marker: &str) -> Vec<&'a str> {
    if marker == "control" {
        vec!["first", "find", "last"]
    } else {
        state["field"].as_str().into_iter().collect()
    }
}

#[test]
fn schema_lifecycle_clears_stale_diagnostics_and_actions() {
    for crlf in [false, true] {
        let mut spec = load("diagnostic-action-schema-lifecycle");
        if crlf {
            for source in spec.files.values_mut() {
                *source = source.replace('\n', "\r\n");
            }
        }
        let fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let parent = crate::tests::support::unique_temp_root("diagnostic-action-schema");
        let root = parent.join("中文 % schema");
        fixture.materialize(&root).expect("workspace");
        let schema = root.join("schema.json");
        fs::write(&schema, spec.oracle["levelSchema"].to_string()).expect("initial schema");
        let document = &fixture.disk["scripts/main.vela"];
        let (mut server, initial) = start(&root, document);
        let states = spec.oracle["states"].as_array().expect("schema states");
        for (index, state) in states.iter().enumerate() {
            let publication = if index == 0 {
                initial.clone()
            } else {
                let existed = schema.exists();
                let kind = match state["operation"].as_str().expect("operation") {
                    "delete" => {
                        fs::remove_file(&schema).expect("delete schema");
                        3
                    }
                    "level" => {
                        fs::write(&schema, spec.oracle["levelSchema"].to_string())
                            .expect("restore schema");
                        if existed { 2 } else { 1 }
                    }
                    "rank" => {
                        fs::write(&schema, spec.oracle["rankSchema"].to_string())
                            .expect("replace schema");
                        if existed { 2 } else { 1 }
                    }
                    "invalid" => {
                        fs::write(&schema, "{ broken").expect("invalidate schema");
                        2
                    }
                    operation => panic!("unsupported operation {operation}"),
                };
                let notifications = notification_values(notify::<n::DidChangeWatchedFiles>(
                    &mut server,
                    json!({"changes": [{"uri": uri(&root, "schema.json"), "type": kind}]}),
                ));
                notifications
                    .into_iter()
                    .find(|value| {
                        value["method"] == "textDocument/publishDiagnostics"
                            && value["params"]["uri"] == uri(&root, "scripts/main.vela")
                    })
                    .expect("source publication")
            };
            assert_eq!(
                projection(&publication),
                expected_diagnostics(document, &schema, state, None),
                "{} CRLF={crlf}",
                state["id"]
            );
            let mut observed = Vec::new();
            for (offset, marker) in ["old", "new", "control"].iter().enumerate() {
                observed.push(actions(
                    &mut server,
                    &root,
                    document,
                    &publication,
                    marker,
                    10 + (index as i32) * 3 + offset as i32,
                    &expected_replacements(state, marker),
                ));
            }

            let (mut fresh, fresh_publication) = start(&root, document);
            assert_eq!(
                publication["params"]["diagnostics"], fresh_publication["params"]["diagnostics"],
                "fresh diagnostics {} CRLF={crlf}",
                state["id"]
            );
            for (offset, marker) in ["old", "new", "control"].iter().enumerate() {
                assert_eq!(
                    observed[offset],
                    actions(
                        &mut fresh,
                        &root,
                        document,
                        &fresh_publication,
                        marker,
                        2 + offset as i32,
                        &expected_replacements(state, marker)
                    ),
                    "fresh actions {} {marker} CRLF={crlf}",
                    state["id"]
                );
            }

            if matches!(state["id"].as_str(), Some("level" | "rank")) {
                let (marker, oracle_key) = if state["id"] == "level" {
                    ("old", "appliedLevel")
                } else {
                    ("new", "appliedRank")
                };
                let edit = &observed[if marker == "old" { 0 } else { 1 }][0]["edit"]["changes"]
                    [&uri(&root, "scripts/main.vela")][0];
                let target = document.markers[marker];
                let mut applied_text = document.text.clone();
                applied_text.replace_range(
                    target.start.byte..target.end.byte,
                    edit["newText"].as_str().expect("replacement"),
                );
                let marked = spec.oracle[oracle_key].as_str().expect("applied oracle");
                let applied_document = parse_markers(&if crlf {
                    marked.replace('\n', "\r\n")
                } else {
                    marked.to_owned()
                })
                .expect("applied markers");
                assert_eq!(
                    applied_text, applied_document.text,
                    "whole applied source {oracle_key} {crlf}"
                );
                let changed = sync_diagnostics::<n::DidChangeTextDocument>(
                    &mut fresh,
                    json!({"textDocument": {"uri": uri(&root, "scripts/main.vela"), "version": 2},
                        "contentChanges": [{"text": applied_text}]}),
                );
                assert_eq!(
                    projection(&changed),
                    expected_diagnostics(&applied_document, &schema, state, Some(marker)),
                    "applied diagnostics {oracle_key} {crlf}"
                );
                let no_stale = response_value(request::<r::CodeActionRequest>(
                    &mut fresh,
                    8,
                    json!({"textDocument": {"uri": uri(&root, "scripts/main.vela")},
                        "range": range(applied_document.markers[marker]),
                        "context": {"diagnostics": changed["params"]["diagnostics"]}}),
                ));
                assert_eq!(
                    no_stale["result"],
                    json!([]),
                    "no stale applied action {oracle_key} {crlf}"
                );
            }
        }
        fs::remove_dir_all(parent).expect("cleanup");
    }
}
