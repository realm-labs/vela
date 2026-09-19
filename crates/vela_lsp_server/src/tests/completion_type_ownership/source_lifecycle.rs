use crate::matrix_fixture::FixtureWorkspace;
use crate::tests::{TestServer, notification_values, notify, request, response_value};
use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};
use std::path::Path;

pub(super) fn apply_phase(
    server: &mut TestServer,
    root: &Path,
    fixture: &FixtureWorkspace,
    phase: &Value,
    diagnostic_file: &str,
) {
    let mut changes = Vec::new();
    for file in phase["files"].as_object().expect("source files").keys() {
        crate::matrix_fixture::safe_file(file).expect("fixture-relative path");
        let path = root.join(file);
        let exists = path.exists();
        let kind = if let Some(document) = fixture.disk.get(file) {
            std::fs::write(&path, &document.text).expect("write source");
            if exists { 2 } else { 1 }
        } else {
            assert!(exists, "delete an existing dependency");
            std::fs::remove_file(&path).expect("delete source");
            3
        };
        changes.push(json!({"uri":lsp_types::Url::from_file_path(path).expect("URI"),"type":kind}));
    }
    let messages = notification_values(notify::<n::DidChangeWatchedFiles>(
        server,
        json!({"changes":changes}),
    ));
    let diagnostic_uri = lsp_types::Url::from_file_path(root.join(diagnostic_file)).expect("URI");
    let publications = messages
        .iter()
        .filter(|message| {
            message["method"] == "textDocument/publishDiagnostics"
                && message["params"]["uri"] == diagnostic_uri.as_str()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        publications.len(),
        1,
        "control publication: {phase} {messages:?}"
    );
    let actual = publications[0]["params"]["diagnostics"]
        .as_array()
        .expect("diagnostics");
    let expected = phase["diagnostics"]
        .as_array()
        .expect("expected diagnostics");
    assert_eq!(
        actual.len(),
        expected.len(),
        "control diagnostics: {phase} {actual:?}"
    );
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(actual["code"], expected["code"]);
        assert_eq!(actual["severity"], 1);
        assert!(
            actual["message"]
                .as_str()
                .expect("message")
                .contains(expected["message"].as_str().expect("expected message")),
            "{actual}"
        );
    }
}

pub(super) fn fresh_server(root: &Path, capabilities: &Value) -> TestServer {
    let mut server = TestServer::new();
    let result = response_value(request::<r::Initialize>(
        &mut server,
        1,
        json!({"processId":null,"rootUri":lsp_types::Url::from_file_path(root).expect("root URI"),"capabilities":capabilities}),
    ));
    assert!(result["error"].is_null(), "{result}");
    server
}

pub(super) fn assert_definition(
    server: &mut TestServer,
    root: &Path,
    fixture: &FixtureWorkspace,
    query: &Value,
    id: i32,
) {
    let point = fixture
        .document(query["file"].as_str().expect("query file"))
        .expect("source")
        .markers["callee"]
        .start;
    let result = response_value(request::<r::GotoDefinition>(
        server,
        id,
        json!({"textDocument":{"uri":lsp_types::Url::from_file_path(root.join(query["file"].as_str().expect("file"))).expect("URI")},"position":{"line":point.line,"character":point.character+1}}),
    ));
    assert!(result["error"].is_null(), "{result}");
    let expected = query["target"].as_str().map(|target| {
        let marker = fixture.document(target).expect("target").markers[query["marker"].as_str().expect("marker")];
        json!({"uri":lsp_types::Url::from_file_path(root.join(target)).expect("URI"),"range":{"start":{"line":marker.start.line,"character":marker.start.character},"end":{"line":marker.end.line,"character":marker.end.character}}})
    });
    assert_eq!(
        result["result"],
        json!(expected),
        "existing-call definition: {query}"
    );
}
