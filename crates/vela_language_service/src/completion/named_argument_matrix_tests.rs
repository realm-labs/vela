use crate::matrix_fixture::{FixtureWorkspace, load};
use crate::{
    CompletionContextKind, DocumentId, LanguageServiceDatabases, Position, SourceFileSnapshot,
    TextRange, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn named_argument_matrix_checks_owner_sets_occupied_slots_and_parseable_edits() {
    for crlf in [false, true] {
        let mut spec = load("completion-named-arguments");
        if crlf {
            for source in spec.files.values_mut() {
                *source = source.replace('\n', "\r\n");
            }
        }
        let fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let db = databases(&fixture);
        for query in spec.oracle["queries"].as_array().expect("queries") {
            let file = query["file"].as_str().expect("file");
            let source = fixture.document(file).expect("source");
            let range = source.markers["replace"];
            let point = source.markers["cursor"].start;
            let result = db.completion_items(&uri(file), position(&source.text, point.byte));
            assert_eq!(
                result.context().kind(),
                CompletionContextKind::NamedArgument,
                "{query}"
            );
            assert_eq!(
                db.completion_items(&uri(file), position(&source.text, point.byte))
                    .items(),
                result.items()
            );
            let mut names = result
                .items()
                .iter()
                .map(|item| item.label())
                .collect::<Vec<_>>();
            names.sort_unstable();
            let mut expected = query["parameters"]
                .as_array()
                .expect("params")
                .iter()
                .map(|p| p["name"].as_str().expect("name"))
                .collect::<Vec<_>>();
            expected.sort_unstable();
            assert_eq!(names, expected, "{query}");
            if let Some(label) = query["signature"].as_str() {
                let help = db
                    .signature_help(&uri(file), position(&source.text, point.byte))
                    .expect("signature");
                assert_eq!(help.signatures().len(), 1, "{query}");
                assert_eq!(help.signatures()[0].label(), label, "{query}");
                if let Some(active) = query["activeParameter"].as_u64() {
                    assert_eq!(help.active_parameter(), active as usize, "{query}");
                }
            }
            for expected in query["parameters"].as_array().expect("params") {
                let item = result
                    .items()
                    .iter()
                    .find(|item| item.label() == expected["name"])
                    .expect("parameter");
                assert_eq!(item.kind(), crate::CompletionKind::Parameter);
                assert_eq!(item.detail(), expected["detail"].as_str().expect("detail"));
                let edit = item.text_edit().expect("explicit edit");
                assert_eq!(
                    edit.range(),
                    TextRange::new(range.start.byte, range.end.byte)
                );
                assert_eq!(
                    edit.new_text(),
                    expected["insert"].as_str().expect("insert")
                );
                assert_eq!(
                    item.insert_format(),
                    crate::CompletionInsertFormat::PlainText
                );
                if item.label() == query["apply"] {
                    let insertion = format!(
                        "{}{}",
                        edit.new_text(),
                        query["value"].as_str().expect("value")
                    );
                    let mut edited = source.text.clone();
                    edited.replace_range(range.start.byte..range.end.byte, &insertion);
                    let expected_text = format!(
                        "{}{}{}{}",
                        &source.text[..range.start.byte],
                        expected["insert"].as_str().expect("insert"),
                        query["value"].as_str().expect("value"),
                        &source.text[range.end.byte..]
                    );
                    assert_eq!(edited, expected_text);
                    assert!(
                        vela_syntax::parse::parse_source(&edited)
                            .diagnostics()
                            .is_empty(),
                        "applied call must parse: {edited}"
                    );
                    let mut fresh = FixtureWorkspace::new(&spec).expect("fresh");
                    fresh.disk.get_mut(file).expect("file").text = edited.clone();
                    let again = databases(&fresh).completion_items(
                        &uri(file),
                        position(&edited, range.start.byte + item.label().len()),
                    );
                    assert!(
                        again
                            .items()
                            .iter()
                            .all(|candidate| candidate.label() != item.label()),
                        "occupied name must disappear: {query}"
                    );
                }
            }
        }
    }
}
fn uri(file: &str) -> DocumentId {
    DocumentId::from(format!("/workspace/{file}"))
}
fn position(text: &str, byte: usize) -> Position {
    Position::new(
        text[..byte].bytes().filter(|c| *c == b'\n').count(),
        byte - text[..byte].rfind('\n').map_or(0, |i| i + 1),
    )
}
fn databases(fixture: &FixtureWorkspace) -> LanguageServiceDatabases {
    let files = fixture
        .disk
        .iter()
        .filter(|(file, _)| file.ends_with(".vela"))
        .map(|(file, doc)| SourceFileSnapshot::new(uri(file), doc.text.as_str()))
        .collect::<Vec<_>>();
    let mut db = LanguageServiceDatabases::new();
    db.update(&assemble_project_sources(
        &WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]),
        &files,
        &Workspace::new().snapshot(),
    ));
    if let Some(schema) = fixture.disk.get("schema.json") {
        db.load_schema_artifact_json("/workspace/schema.json", &schema.text);
        assert!(db.schema_db().diagnostics().is_empty(), "valid schema");
    }
    db
}
