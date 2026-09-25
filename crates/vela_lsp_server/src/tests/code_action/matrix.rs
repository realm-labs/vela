use crate::matrix_fixture::{Document, FixtureWorkspace, Marker, load, parse_markers};
use crate::tests::{TestServer, request, response_value, sync_diagnostics};
use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};

fn range(marker: Marker) -> Value {
    json!({
        "start": { "line": marker.start.line, "character": marker.start.character },
        "end": { "line": marker.end.line, "character": marker.end.character }
    })
}

fn check_diagnostics(actual: &Value, document: &Document, expected: &[Value]) {
    let diagnostics = actual["params"]["diagnostics"]
        .as_array()
        .expect("published diagnostics");
    assert_eq!(diagnostics.len(), expected.len(), "exact diagnostic count");
    for (diagnostic, expected) in diagnostics.iter().zip(expected) {
        let marker = expected["marker"].as_str().expect("marker");
        assert_eq!(diagnostic["code"], expected["code"]);
        assert_eq!(diagnostic["message"], expected["message"]);
        assert_eq!(diagnostic["range"], range(document.markers[marker]));
        assert_eq!(diagnostic["severity"], 1);
    }
}

#[test]
fn method_typo_fix_clears_only_its_published_diagnostic() {
    for crlf in [false, true] {
        let mut spec = load("diagnostic-action-method-typo");
        if crlf {
            for source in spec.files.values_mut() {
                *source = source.replace('\n', "\r\n");
            }
        }
        let fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let source = &fixture.disk["scripts/game/main.vela"];
        let applied_source = spec.oracle["applied"].as_str().expect("applied source");
        let applied = parse_markers(&if crlf {
            applied_source.replace('\n', "\r\n")
        } else {
            applied_source.to_owned()
        })
        .expect("applied markers");
        let expected = spec.oracle["diagnostics"].as_array().expect("diagnostics");
        let uri = "file:///workspace/scripts/game/main.vela";
        let mut server = TestServer::new();
        let _ = response_value(request::<r::Initialize>(
            &mut server,
            1,
            json!({"processId": null, "rootUri": "file:///workspace/scripts", "capabilities": {}}),
        ));
        let published = sync_diagnostics::<n::DidOpenTextDocument>(
            &mut server,
            json!({"textDocument": {"uri": uri, "languageId": "vela", "version": 1, "text": source.text}}),
        );
        check_diagnostics(&published, source, expected);

        let fix = source.markers["fix"];
        let response = response_value(request::<r::CodeActionRequest>(
            &mut server,
            2,
            json!({
                "textDocument": {"uri": uri},
                "range": range(fix),
                "context": {"diagnostics": published["params"]["diagnostics"]}
            }),
        ));
        let actions = response["result"].as_array().expect("code actions");
        let titles = actions
            .iter()
            .map(|action| &action["title"])
            .collect::<Vec<_>>();
        let expected_titles = spec.oracle["actionTitles"]
            .as_array()
            .expect("action titles")
            .iter()
            .collect::<Vec<_>>();
        assert_eq!(titles, expected_titles, "exact LSP fix set {crlf}");
        let replacements = spec.oracle["actionReplacements"]
            .as_array()
            .expect("action replacements");
        assert_eq!(actions.len(), replacements.len());
        for (action, replacement) in actions.iter().zip(replacements) {
            assert_eq!(action["kind"], "quickfix");
            let edits = action["edit"]["changes"][uri]
                .as_array()
                .expect("workspace edits");
            let [edit] = edits.as_slice() else {
                panic!("one text edit");
            };
            assert_eq!(edit["range"], range(fix));
            assert_eq!(edit["newText"], *replacement);
            let changes = action["edit"]["documentChanges"]
                .as_array()
                .expect("versioned changes");
            let [change] = changes.as_slice() else {
                panic!("one versioned document");
            };
            assert_eq!(change["textDocument"], json!({"uri": uri, "version": 1}));
            assert_eq!(change["edits"], json!([edit]));
        }
        let action = actions
            .iter()
            .find(|action| action["title"] == spec.oracle["action"]["title"])
            .expect("selected fix");
        assert_eq!(action["kind"], "quickfix");
        let edits = action["edit"]["changes"][uri]
            .as_array()
            .expect("workspace edits");
        let [edit] = edits.as_slice() else {
            panic!("one text edit");
        };
        assert_eq!(edit["range"], range(fix));
        assert_eq!(edit["newText"], "first");
        let mut actual_text = source.text.clone();
        actual_text.replace_range(
            fix.start.byte..fix.end.byte,
            edit["newText"].as_str().expect("replacement"),
        );
        assert_eq!(actual_text, applied.text, "whole applied source {crlf}");
        assert!(
            vela_syntax::parse::parse_source(&actual_text)
                .diagnostics()
                .is_empty()
        );

        let after = sync_diagnostics::<n::DidChangeTextDocument>(
            &mut server,
            json!({
                "textDocument": {"uri": uri, "version": 2},
                "contentChanges": [{"text": actual_text}]
            }),
        );
        check_diagnostics(&after, &applied, &expected[1..]);
        let fixed = applied.markers["fix"];
        let fixed_response = response_value(request::<r::CodeActionRequest>(
            &mut server,
            3,
            json!({
                "textDocument": {"uri": uri},
                "range": range(fixed),
                "context": {"diagnostics": after["params"]["diagnostics"]}
            }),
        ));
        assert_eq!(fixed_response["result"], json!([]));
    }
}
