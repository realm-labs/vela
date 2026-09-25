use crate::matrix_fixture::{Document, FixtureWorkspace, Marker, load, parse_markers};
use crate::tests::{TestServer, request, response_value, sync_diagnostics};
use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};
use std::fs;

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

fn action_result(
    server: &mut TestServer,
    id: i32,
    uri: &str,
    document: &Document,
    oracle: &Value,
    version: Option<i32>,
) -> Value {
    let fix = document.markers["fix"];
    let response = response_value(request::<r::CodeActionRequest>(
        server,
        id,
        json!({
            "textDocument": {"uri": uri},
            "range": range(fix),
            "context": {"diagnostics": []}
        }),
    ));
    let actions = response["result"].as_array().expect("code actions");
    let titles = actions
        .iter()
        .map(|action| &action["title"])
        .collect::<Vec<_>>();
    let expected_titles = oracle["actionTitles"]
        .as_array()
        .expect("action titles")
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(titles, expected_titles, "exact candidate set");
    for (action, replacement) in actions.iter().zip(
        oracle["actionReplacements"]
            .as_array()
            .expect("replacements"),
    ) {
        assert_eq!(action["kind"], "quickfix");
        assert_eq!(
            action["edit"]["changes"]
                .as_object()
                .expect("changes")
                .len(),
            1
        );
        assert_eq!(
            action["edit"]["changes"][uri],
            json!([{"range":range(fix),"newText":replacement}])
        );
        let [change] = action["edit"]["documentChanges"]
            .as_array()
            .expect("document changes")
            .as_slice()
        else {
            panic!("one versioned document");
        };
        assert_eq!(change["textDocument"], json!({"uri":uri,"version":version}));
        assert_eq!(change["edits"], action["edit"]["changes"][uri]);
    }
    response["result"].clone()
}

#[test]
fn encoded_uri_dirty_repeat_and_close_restore_diagnostics_and_actions() {
    for crlf in [false, true] {
        let mut spec = load("diagnostic-action-method-typo");
        if crlf {
            for source in spec.files.values_mut() {
                *source = source.replace('\n', "\r\n");
            }
        }
        let fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let file = "scripts/game/main.vela";
        let disk = &fixture.disk[file];
        let marked = |name: &str| {
            let source = spec.oracle[name].as_str().expect("marked source");
            parse_markers(&if crlf {
                source.replace('\n', "\r\n")
            } else {
                source.to_owned()
            })
            .expect("source markers")
        };
        let dirty = marked("dirty");
        let applied = marked("appliedDirty");
        let expected = spec.oracle["diagnostics"].as_array().expect("diagnostics");
        let parent = crate::tests::support::unique_temp_root("diagnostic-action-encoded");
        let root = parent.join("中文 % action");
        fixture.materialize(&root).expect("isolated fixture");
        let uri = lsp_types::Url::from_file_path(root.join(file))
            .expect("encoded file URI")
            .to_string();
        assert!(uri.contains("%E4%B8%AD") && uri.contains("%20") && uri.contains("%25"));
        let root_uri = lsp_types::Url::from_file_path(&root).expect("encoded root URI");
        let mut server = TestServer::new();
        let _ = response_value(request::<r::Initialize>(
            &mut server,
            1,
            json!({"processId":null,"rootUri":root_uri,"capabilities":{}}),
        ));
        let opened = sync_diagnostics::<n::DidOpenTextDocument>(
            &mut server,
            json!({"textDocument":{"uri":uri,"languageId":"vela","version":1,"text":disk.text}}),
        );
        assert_eq!(opened["params"]["uri"], uri);
        check_diagnostics(&opened, disk, expected);
        let baseline = action_result(&mut server, 2, &uri, disk, &spec.oracle, Some(1));
        assert_eq!(
            action_result(&mut server, 3, &uri, disk, &spec.oracle, Some(1)),
            baseline,
            "same source and version give identical actions"
        );
        let repeated = sync_diagnostics::<n::DidChangeTextDocument>(
            &mut server,
            json!({"textDocument":{"uri":uri,"version":2},"contentChanges":[{"text":disk.text}]}),
        );
        assert_eq!(
            repeated["params"]["diagnostics"],
            opened["params"]["diagnostics"]
        );
        check_diagnostics(&repeated, disk, expected);
        action_result(&mut server, 9, &uri, disk, &spec.oracle, Some(2));

        let changed = sync_diagnostics::<n::DidChangeTextDocument>(
            &mut server,
            json!({"textDocument":{"uri":uri,"version":3},"contentChanges":[{"text":dirty.text}]}),
        );
        assert_eq!(changed["params"]["uri"], uri);
        check_diagnostics(&changed, &dirty, expected);
        assert_ne!(
            disk.markers["fix"].start.character,
            dirty.markers["fix"].start.character
        );
        let actions = action_result(&mut server, 4, &uri, &dirty, &spec.oracle, Some(3));
        assert_eq!(
            action_result(&mut server, 5, &uri, &dirty, &spec.oracle, Some(3)),
            actions,
            "dirty source and version give identical actions"
        );
        let replacement = actions[0]["edit"]["changes"][&uri][0]["newText"]
            .as_str()
            .expect("replacement");
        let mut applied_text = dirty.text.clone();
        let fix = dirty.markers["fix"];
        applied_text.replace_range(fix.start.byte..fix.end.byte, replacement);
        assert_eq!(applied_text, applied.text, "whole applied dirty source");
        let repaired = sync_diagnostics::<n::DidChangeTextDocument>(
            &mut server,
            json!({"textDocument":{"uri":uri,"version":4},"contentChanges":[{"text":applied_text}]}),
        );
        assert_eq!(repaired["params"]["uri"], uri);
        check_diagnostics(&repaired, &applied, &expected[1..]);
        let no_fix = response_value(request::<r::CodeActionRequest>(
            &mut server,
            6,
            json!({"textDocument":{"uri":uri},"range":range(applied.markers["fix"]),"context":{"diagnostics":[]}}),
        ));
        assert_eq!(no_fix["result"], json!([]));

        let closed = sync_diagnostics::<n::DidCloseTextDocument>(
            &mut server,
            json!({"textDocument":{"uri":uri}}),
        );
        assert_eq!(closed["params"]["uri"], uri);
        check_diagnostics(&closed, disk, expected);
        action_result(&mut server, 7, &uri, disk, &spec.oracle, None);
        let reopened = sync_diagnostics::<n::DidOpenTextDocument>(
            &mut server,
            json!({"textDocument":{"uri":uri,"languageId":"vela","version":5,"text":disk.text}}),
        );
        check_diagnostics(&reopened, disk, expected);
        action_result(&mut server, 8, &uri, disk, &spec.oracle, Some(5));
        fs::remove_dir_all(parent).expect("remove isolated fixture");
    }
}
