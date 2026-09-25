use serde_json::{Value, json};

use crate::matrix_fixture::{Document, FixtureWorkspace, Marker, load, schema_artifact};
use crate::{
    DiagnosticRange, DocumentId, LanguageServiceDatabases, LineIndex, Position, SourceFileSnapshot,
    Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

const MAIN: &str = "scripts/game/main.vela";
const IMPORT: &str = "use game::reward::award\n";

fn uri(file: &str) -> DocumentId {
    DocumentId::from(format!("/workspace/{file}"))
}

fn position(text: &str, point: crate::matrix_fixture::Point) -> Position {
    Position::new(
        point.line,
        point.byte - text[..point.byte].rfind('\n').map_or(0, |n| n + 1),
    )
}

fn marker_range(document: &Document, marker: &str) -> DiagnosticRange {
    let Marker { start, end } = document.markers[marker];
    DiagnosticRange::new(
        position(&document.text, start),
        position(&document.text, end),
    )
}

fn databases(fixture: &FixtureWorkspace, schema: &Value) -> LanguageServiceDatabases {
    let sources = fixture
        .disk
        .iter()
        .filter(|(file, _)| file.ends_with(".vela"))
        .map(|(file, document)| SourceFileSnapshot::new(uri(file), document.text.as_str()))
        .collect::<Vec<_>>();
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let mut db = LanguageServiceDatabases::new();
    db.update(&assemble_project_sources(
        &config,
        &sources,
        &Workspace::new().snapshot(),
    ));
    let artifact = schema_artifact(schema, fixture, |file| {
        db.source_db().records()[&uri(file)].source_id().get()
    });
    db.load_schema_artifact_json("/workspace/target/schema.json", &artifact.to_string());
    assert!(
        db.schema_db()
            .facts()
            .function_fact("host::stamp")
            .is_some()
    );
    db
}

fn actions(db: &LanguageServiceDatabases, document: &Document, marker: &str) -> Value {
    json!(
        db.code_actions(&uri(MAIN), marker_range(document, marker))
            .iter()
            .map(|action| {
                let [file] = action.edit().document_edits() else {
                    panic!("one target")
                };
                let [edit] = file.edits() else {
                    panic!("one edit")
                };
                let range = edit.range();
                json!({
                    "title":action.title(), "kind":action.kind().as_lsp_kind(),
                    "uri":file.document_id().as_str(),
                    "range":{"start":[range.start().line,range.start().character],
                        "end":[range.end().line,range.end().character]},
                    "newText":edit.new_text(),
                })
            })
            .collect::<Vec<_>>()
    )
}

fn apply(document: &Document, action: &Value) -> String {
    let index = LineIndex::new(&document.text);
    let span = &action["range"];
    let start = index.offset(Position::new(
        span["start"][0].as_u64().expect("line") as usize,
        span["start"][1].as_u64().expect("character") as usize,
    ));
    let end = index.offset(Position::new(
        span["end"][0].as_u64().expect("line") as usize,
        span["end"][1].as_u64().expect("character") as usize,
    ));
    let mut source = document.text.clone();
    source.replace_range(start..end, action["newText"].as_str().expect("new text"));
    source
}

#[test]
fn import_actions_apply_exact_edits_and_reject_invalid_owners() {
    for crlf in [false, true] {
        let mut spec = load("code-action-import-partitions");
        if crlf {
            for source in spec.files.values_mut() {
                *source = source.replace('\n', "\r\n");
            }
        }
        let fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let document = &fixture.disk[MAIN];
        let db = databases(&fixture, &spec.oracle["schema"]);
        let main = uri(MAIN);
        let names = db
            .diagnostics_for_document(&main)
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code() == Some("hir::unresolved_name"))
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            [
                "unresolved name `award`",
                "unresolved name `clash`",
                "unresolved name `secret`",
                "unresolved name `FLAG`",
                "unresolved name `score`",
                "unresolved name `Row`",
                "unresolved name `Mode`",
                "unresolved name `Reader`",
            ]
        );
        let baseline_messages = db
            .diagnostics_for_document(&main)
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>();

        let add = actions(&db, document, "missing-name");
        assert_eq!(
            add,
            json!([{
                "title":"Import `game::reward::award`", "kind":"quickfix",
                "uri":main.as_str(), "range":{"start":[7,0],"end":[7,0]},
                "newText":IMPORT,
            }])
        );
        let remove = actions(&db, document, "unused-import");
        assert_eq!(
            remove,
            json!([{
                "title":"Remove unused import", "kind":"quickfix",
                "uri":main.as_str(), "range":{"start":[0,0],"end":[1,0]},
                "newText":"",
            }])
        );
        for (marker, name) in [
            ("missing-const", "FLAG"),
            ("missing-state", "score"),
            ("missing-struct", "Row"),
            ("missing-enum", "Mode"),
            ("missing-trait", "Reader"),
        ] {
            let action = actions(&db, document, marker);
            assert_eq!(
                action,
                json!([{
                    "title":format!("Import `game::reward::{name}`"), "kind":"quickfix",
                "uri":main.as_str(), "range":{"start":[7,0],"end":[7,0]},
                    "newText":format!("use game::reward::{name}\n"),
                }]),
                "{marker} {crlf}"
            );
            let inserted = apply(document, &action[0]);
            let expected = document.text.replacen(
                "/* 中😀 */ fn main()",
                &format!("use game::reward::{name}\n/* 中😀 */ fn main()"),
                1,
            );
            assert_eq!(inserted, expected, "whole {name} edit {crlf}");
            let mut applied_fixture = fixture.clone();
            applied_fixture.disk.get_mut(MAIN).expect("main").text = inserted;
            let applied_db = databases(&applied_fixture, &spec.oracle["schema"]);
            let messages = applied_db
                .diagnostics_for_document(&main)
                .diagnostics()
                .iter()
                .map(|diagnostic| diagnostic.message().to_owned())
                .collect::<Vec<_>>();
            assert_eq!(
                messages,
                baseline_messages
                    .iter()
                    .filter(|message| *message != &format!("unresolved name `{name}`"))
                    .cloned()
                    .collect::<Vec<_>>(),
                "{name} {crlf}"
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
        ] {
            assert_eq!(actions(&db, document, marker), json!([]), "{marker} {crlf}");
        }

        let inserted = apply(document, &add[0]);
        let expected = document.text.replacen(
            "/* 中😀 */ fn main()",
            &format!("{IMPORT}/* 中😀 */ fn main()"),
            1,
        );
        assert_eq!(inserted, expected, "whole import edit {crlf}");
        assert!(
            vela_syntax::parse::parse_source(&inserted)
                .diagnostics()
                .is_empty()
        );
        let mut applied_fixture = fixture.clone();
        applied_fixture.disk.get_mut(MAIN).expect("main").text = inserted;
        let applied_db = databases(&applied_fixture, &spec.oracle["schema"]);
        let messages = applied_db
            .diagnostics_for_document(&main)
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            messages,
            baseline_messages
                .iter()
                .filter(|message| *message != "unresolved name `award`")
                .cloned()
                .collect::<Vec<_>>(),
            "award {crlf}"
        );
        let used_alias =
            applied_fixture.disk[MAIN]
                .text
                .replacen("return a;", "let kept = spare; return a;", 1);
        let mut both_used_fixture = applied_fixture.clone();
        both_used_fixture.disk.get_mut(MAIN).expect("main").text = used_alias;
        let both_used_db = databases(&both_used_fixture, &spec.oracle["schema"]);
        let both_used_messages = both_used_db
            .diagnostics_for_document(&main)
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            both_used_messages,
            baseline_messages
                .iter()
                .filter(|message| *message != "unresolved name `award`"
                    && *message != "unused import `spare`")
                .cloned()
                .collect::<Vec<_>>(),
            "both bindings used {crlf}"
        );

        let removed = apply(document, &remove[0]);
        let first_line = document.text.find('\n').expect("first import") + 1;
        assert_eq!(
            removed,
            document.text[first_line..],
            "whole removal edit {crlf}"
        );
        let mut removed_fixture = fixture.clone();
        removed_fixture.disk.get_mut(MAIN).expect("main").text = removed;
        let removed_db = databases(&removed_fixture, &spec.oracle["schema"]);
        let messages = removed_db
            .diagnostics_for_document(&main)
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            messages,
            baseline_messages
                .iter()
                .filter(|message| *message != "unused import `spare`")
                .cloned()
                .collect::<Vec<_>>(),
            "remove unused {crlf}"
        );
    }
}
