use std::{fs, path::Path};

use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};
use vela_language_service::DocumentId;

use crate::matrix_fixture::{Document, FixtureWorkspace, Marker, Spec, load, schema_artifact};
use crate::tests::{TestServer, notify, request, response_value, sync_diagnostics};

const MAIN: &str = "scripts/game/main.vela";
const IMPORT: &str = "use game::reward::award\n";

fn uri(root: &Path, file: &str) -> String {
    lsp_types::Url::from_file_path(root.join(file))
        .expect("URI")
        .to_string()
}

fn range(marker: Marker) -> Value {
    json!({"start":{"line":marker.start.line,"character":marker.start.character},
        "end":{"line":marker.end.line,"character":marker.end.character}})
}

fn point(line: usize, character: usize) -> Value {
    json!({"line":line,"character":character})
}

fn start(root: &Path, spec: &Spec, fixture: &FixtureWorkspace, text: &str) -> (TestServer, Value) {
    let mut server = TestServer::new();
    let _ = response_value(request::<r::Initialize>(
        &mut server,
        1,
        json!({"processId":null,"rootUri":uri(root,""),"capabilities":{}}),
    ));
    let snapshot = server.snapshot();
    let artifact = schema_artifact(&spec.oracle["schema"], fixture, |file| {
        snapshot.databases().source_db().records()[&DocumentId::from(uri(root, file))]
            .source_id()
            .get()
    });
    fs::create_dir_all(root.join("target")).expect("target");
    fs::write(root.join("target/schema.json"), artifact.to_string()).expect("schema");
    let _ = notify::<n::DidChangeWatchedFiles>(
        &mut server,
        json!({"changes":[{"uri":uri(root,"target/schema.json"),"type":1}]}),
    );
    assert!(
        server
            .snapshot()
            .databases()
            .schema_db()
            .facts()
            .function_fact("host::stamp")
            .is_some()
    );
    let opened = sync_diagnostics::<n::DidOpenTextDocument>(
        &mut server,
        json!({"textDocument":{"uri":uri(root,MAIN),
            "languageId":"vela","version":1,"text":text}}),
    );
    (server, opened)
}

fn actions(server: &mut TestServer, root: &Path, span: Value, publication: &Value) -> Value {
    response_value(request::<r::CodeActionRequest>(
        server,
        2,
        json!({"textDocument":{"uri":uri(root,MAIN)}, "range":span,
            "context":{"diagnostics":publication["params"]["diagnostics"]}}),
    ))["result"]
        .clone()
}

fn expected_action(
    root: &Path,
    version: i32,
    title: &str,
    span: Value,
    replacement: &str,
) -> Value {
    let file = uri(root, MAIN);
    let edit = json!({"range":span,"newText":replacement});
    json!([{
        "title":title,"kind":"quickfix",
        "edit":{"changes":{file.clone():[edit.clone()]},
            "documentChanges":[{"textDocument":{"uri":file,"version":version},"edits":[edit]}]}
    }])
}

fn apply_action(text: &str, action: &Value, file: &str) -> String {
    let edit = &action["edit"]["changes"][file][0];
    let span: crate::protocol::LspRange =
        serde_json::from_value(edit["range"].clone()).expect("edit range");
    let index = crate::line_index::LineIndex::new(text);
    let start = index.offset(span.start).expect("edit start");
    let end = index.offset(span.end).expect("edit end");
    let mut result = text.to_owned();
    result.replace_range(start..end, edit["newText"].as_str().expect("replacement"));
    result
}

fn expect_no_stale_diagnostic(publication: &Value, message: &str) {
    let diagnostics = publication["params"]["diagnostics"]
        .as_array()
        .expect("diagnostics");
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic["message"] != message),
        "{diagnostics:?}"
    );
}

fn diagnostic_messages(publication: &Value) -> Vec<String> {
    publication["params"]["diagnostics"]
        .as_array()
        .expect("diagnostics")
        .iter()
        .map(|diagnostic| diagnostic["message"].as_str().expect("message").to_owned())
        .collect()
}

fn run(crlf: bool) {
    let mut spec = load("code-action-import-partitions");
    if crlf {
        for source in spec.files.values_mut() {
            *source = source.replace('\n', "\r\n");
        }
    }
    let fixture = FixtureWorkspace::new(&spec).expect("fixture");
    let parent = crate::tests::support::unique_temp_root("code-action-imports");
    let root = parent.join("中文 % actions");
    fixture.materialize(&root).expect("workspace");
    let document: &Document = &fixture.disk[MAIN];
    let (mut server, opened) = start(&root, &spec, &fixture, &document.text);
    let diagnostics = opened["params"]["diagnostics"]
        .as_array()
        .expect("diagnostics");
    assert_eq!(
        diagnostics
            .iter()
            .filter(|d| d["code"] == "hir::unresolved_name")
            .count(),
        8
    );
    assert!(
        diagnostics
            .iter()
            .any(|d| d["message"] == "unused import `spare`")
    );
    let baseline_messages = diagnostic_messages(&opened);
    let add = actions(
        &mut server,
        &root,
        range(document.markers["missing-name"]),
        &opened,
    );
    let insert_range = json!({"start":point(7,0),"end":point(7,0)});
    assert_eq!(
        add,
        expected_action(
            &root,
            1,
            "Import `game::reward::award`",
            insert_range,
            IMPORT
        )
    );
    let remove = actions(
        &mut server,
        &root,
        range(document.markers["unused-import"]),
        &opened,
    );
    let remove_range = json!({"start":point(0,0),"end":point(1,0)});
    assert_eq!(
        remove,
        expected_action(&root, 1, "Remove unused import", remove_range.clone(), "")
    );
    for (marker, name) in [
        ("missing-const", "FLAG"),
        ("missing-state", "score"),
        ("missing-struct", "Row"),
        ("missing-enum", "Mode"),
        ("missing-trait", "Reader"),
    ] {
        let marker = document.markers[marker];
        let (mut case_server, case_opened) = start(&root, &spec, &fixture, &document.text);
        let action = actions(&mut case_server, &root, range(marker), &case_opened);
        assert_eq!(
            action,
            expected_action(
                &root,
                1,
                &format!("Import `game::reward::{name}`"),
                json!({"start":point(7,0),"end":point(7,0)}),
                &format!("use game::reward::{name}\n")
            ),
            "{marker:?} {crlf}"
        );
        let inserted = apply_action(&document.text, &action[0], &uri(&root, MAIN));
        let expected = document.text.replacen(
            "/* 中😀 */ fn main()",
            &format!("use game::reward::{name}\n/* 中😀 */ fn main()"),
            1,
        );
        assert_eq!(inserted, expected, "whole {name} edit {crlf}");
        assert!(
            vela_syntax::parse::parse_source(&inserted)
                .diagnostics()
                .is_empty()
        );
        let changed = sync_diagnostics::<n::DidChangeTextDocument>(
            &mut case_server,
            json!({"textDocument":{"uri":uri(&root,MAIN),"version":2},
                "contentChanges":[{"text":inserted}]}),
        );
        expect_no_stale_diagnostic(&changed, &format!("unresolved name `{name}`"));
        assert_eq!(
            diagnostic_messages(&changed),
            baseline_messages
                .iter()
                .filter(|message| *message != &format!("unresolved name `{name}`"))
                .cloned()
                .collect::<Vec<_>>(),
            "{name} {crlf}"
        );
        let shifted = json!({"start":point(marker.start.line+1,marker.start.character),
            "end":point(marker.end.line+1,marker.end.character)});
        assert_eq!(
            actions(&mut case_server, &root, shifted, &changed),
            json!([])
        );
        let (_fresh_case, fresh_changed) = start(&root, &spec, &fixture, &inserted);
        assert_eq!(
            changed["params"]["diagnostics"], fresh_changed["params"]["diagnostics"],
            "fresh {name} {crlf}"
        );
    }
    for marker in [
        "valid-import",
        "typo-import",
        "private-import",
        "missing-module",
        "schema-import",
        "stdlib-import",
        "alias-use",
        "schema-use",
        "stdlib-use",
        "ambiguous-name",
        "private-name",
        "parameter-declaration",
        "parameter-use",
        "local-declaration",
        "local-use",
        "shadow-declaration",
        "shadow-use",
    ] {
        assert_eq!(
            actions(&mut server, &root, range(document.markers[marker]), &opened),
            json!([]),
            "{marker} {crlf}"
        );
    }
    let (mut fresh, fresh_opened) = start(&root, &spec, &fixture, &document.text);
    assert_eq!(
        opened["params"]["diagnostics"],
        fresh_opened["params"]["diagnostics"]
    );
    assert_eq!(
        add,
        actions(
            &mut fresh,
            &root,
            range(document.markers["missing-name"]),
            &fresh_opened
        )
    );
    assert_eq!(
        remove,
        actions(
            &mut fresh,
            &root,
            range(document.markers["unused-import"]),
            &fresh_opened
        )
    );

    let inserted = apply_action(&document.text, &add[0], &uri(&root, MAIN));
    let expected = document.text.replacen(
        "/* 中😀 */ fn main()",
        &format!("{IMPORT}/* 中😀 */ fn main()"),
        1,
    );
    assert_eq!(inserted, expected, "whole award edit {crlf}");
    assert!(
        vela_syntax::parse::parse_source(&inserted)
            .diagnostics()
            .is_empty()
    );
    let changed = sync_diagnostics::<n::DidChangeTextDocument>(
        &mut server,
        json!({"textDocument":{"uri":uri(&root,MAIN),"version":2},
            "contentChanges":[{"text":inserted}]}),
    );
    expect_no_stale_diagnostic(&changed, "unresolved name `award`");
    assert_eq!(
        diagnostic_messages(&changed),
        baseline_messages
            .iter()
            .filter(|message| *message != "unresolved name `award`")
            .cloned()
            .collect::<Vec<_>>(),
        "award {crlf}"
    );
    assert!(
        changed["params"]["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .iter()
            .any(|d| d["message"] == "unresolved name `clash`")
    );
    let new_award = document.markers["missing-name"];
    let new_range = json!({"start":point(new_award.start.line+1,new_award.start.character),
        "end":point(new_award.end.line+1,new_award.end.character)});
    assert_eq!(
        actions(&mut server, &root, new_range.clone(), &changed),
        json!([])
    );
    let (mut fresh_inserted, fresh_changed) = start(&root, &spec, &fixture, &inserted);
    assert_eq!(
        changed["params"]["diagnostics"],
        fresh_changed["params"]["diagnostics"]
    );
    assert_eq!(
        actions(&mut fresh_inserted, &root, new_range, &fresh_changed),
        json!([])
    );
    let both_used = inserted.replacen("return a;", "let kept = spare; return a;", 1);
    let both_used_changed = sync_diagnostics::<n::DidChangeTextDocument>(
        &mut server,
        json!({"textDocument":{"uri":uri(&root,MAIN),"version":3},
            "contentChanges":[{"text":both_used}]}),
    );
    assert_eq!(
        diagnostic_messages(&both_used_changed),
        baseline_messages
            .iter()
            .filter(|message| *message != "unresolved name `award`"
                && *message != "unused import `spare`")
            .cloned()
            .collect::<Vec<_>>(),
        "both bindings used {crlf}"
    );
    let (_fresh_both, fresh_both) = start(&root, &spec, &fixture, &both_used);
    assert_eq!(
        both_used_changed["params"]["diagnostics"],
        fresh_both["params"]["diagnostics"]
    );

    let removed = apply_action(&document.text, &remove[0], &uri(&root, MAIN));
    let first_line = document.text.find('\n').expect("first line") + 1;
    assert_eq!(
        removed,
        document.text[first_line..],
        "whole removal edit {crlf}"
    );
    let (mut removed_server, _) = start(&root, &spec, &fixture, &document.text);
    let removed_changed = sync_diagnostics::<n::DidChangeTextDocument>(
        &mut removed_server,
        json!({"textDocument":{"uri":uri(&root,MAIN),"version":2},
            "contentChanges":[{"text":removed}]}),
    );
    expect_no_stale_diagnostic(&removed_changed, "unused import `spare`");
    assert_eq!(
        diagnostic_messages(&removed_changed),
        baseline_messages
            .iter()
            .filter(|message| *message != "unused import `spare`")
            .cloned()
            .collect::<Vec<_>>(),
        "remove unused {crlf}"
    );
    assert!(
        removed_changed["params"]["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .iter()
            .any(|d| d["message"] == "unresolved name `award`")
    );
    assert_eq!(
        actions(&mut removed_server, &root, remove_range, &removed_changed),
        json!([])
    );
    let (_fresh_removed, fresh_removed) = start(&root, &spec, &fixture, &removed);
    assert_eq!(
        removed_changed["params"]["diagnostics"],
        fresh_removed["params"]["diagnostics"]
    );
    fs::remove_dir_all(parent).expect("cleanup");
}

#[test]
fn import_actions_preserve_valid_owners_and_apply_exact_utf16_edits() {
    for crlf in [false, true] {
        run(crlf);
    }
}
