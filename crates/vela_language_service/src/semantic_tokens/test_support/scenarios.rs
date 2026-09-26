use super::rows;
use crate::matrix_fixture::{
    Document, FixtureWorkspace, Spec, load, parse_markers, semantic_tokens as oracle,
};
use crate::{
    DiagnosticRange, DocumentId, LanguageServiceDatabases, Position, SourceFileSnapshot, Workspace,
    WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

fn update(db: &mut LanguageServiceDatabases, document: &Document, fixture: &FixtureWorkspace) {
    let sources: Vec<_> = fixture
        .disk
        .iter()
        .filter(|(file, _)| file.ends_with(".vela"))
        .map(|(file, source)| {
            SourceFileSnapshot::new(
                format!("/workspace/{file}"),
                if file == "scripts/main.vela" {
                    document.text.as_str()
                } else {
                    source.text.as_str()
                },
            )
        })
        .collect();
    db.update(&assemble_project_sources(
        &WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]),
        &sources,
        &Workspace::new().snapshot(),
    ));
}

fn load_marked_schema(db: &mut LanguageServiceDatabases, spec: &Spec, fixture: &FixtureWorkspace) {
    if spec.oracle["schema"].is_null() {
        return;
    }
    let artifact =
        crate::matrix_fixture::schema_artifact(&spec.oracle["schema"], fixture, |file| {
            db.source_db().records()[&DocumentId::from(format!("/workspace/{file}"))]
                .source_id()
                .get()
        });
    db.load_schema_artifact_json("/workspace/schema.json", &artifact.to_string());
}

pub(in super::super) fn assert_fixture(fixture: &str) {
    let spec = load(fixture);
    let id = DocumentId::from("/workspace/scripts/main.vela");
    for crlf in [false, true] {
        let source = |text: &str| {
            parse_markers(&if crlf {
                text.replace('\n', "\r\n")
            } else {
                text.to_owned()
            })
            .expect("document")
        };
        let positive = source(&spec.files["scripts/main.vela"]);
        let negative = source(
            spec.oracle["negative"]["source"]
                .as_str()
                .expect("negative source"),
        );
        let mut fixture = FixtureWorkspace::new(&spec).expect("fixture");
        for (file, document) in &mut fixture.disk {
            *document = source(&spec.files[file]);
        }
        let mut db = LanguageServiceDatabases::new();
        if let Some(schema) = spec.files.get("schema.json") {
            db.load_schema_artifact_json("/workspace/schema.json", schema);
        }
        update(&mut db, &positive, &fixture);
        load_marked_schema(&mut db, &spec, &fixture);
        let original = db.semantic_tokens(&id);
        let mut previous = original.clone();
        for (document, expected) in [
            (&positive, &spec.oracle["positive"]),
            (&negative, &spec.oracle["negative"]["tokens"]),
            (&positive, &spec.oracle["positive"]),
        ] {
            update(&mut db, document, &fixture);
            let expected = oracle::expected(document, expected, false);
            let full = db.semantic_tokens(&id);
            oracle::assert_stream(&rows(document, full.tokens()), &expected);
            let delta = db.semantic_token_delta(&id, previous.result_id());
            let mut applied = previous.tokens().to_vec();
            for edit in delta.edits().iter().rev() {
                applied.splice(
                    edit.start()..edit.start() + edit.delete_count(),
                    edit.tokens().iter().copied(),
                );
            }
            assert_eq!(applied, full.tokens(), "actual applied delta");
            assert_eq!(delta.result_id(), full.result_id());
            assert!(
                db.semantic_token_delta(&id, full.result_id())
                    .edits()
                    .is_empty()
            );
            let mut fresh = LanguageServiceDatabases::new();
            if let Some(schema) = spec.files.get("schema.json") {
                fresh.load_schema_artifact_json("/workspace/schema.json", schema);
            }
            update(&mut fresh, document, &fixture);
            load_marked_schema(&mut fresh, &spec, &fixture);
            assert_eq!(fresh.semantic_tokens(&id), full, "incremental equals fresh");
            for line in 0..document.text.lines().count() {
                let selected: Vec<_> = expected
                    .iter()
                    .filter(|row| row.line == line)
                    .cloned()
                    .collect();
                let range = db.semantic_tokens_in_range(
                    &id,
                    DiagnosticRange::new(Position::new(line, 0), Position::new(line + 1, 0)),
                );
                oracle::assert_stream(&rows(document, range.tokens()), &selected);
            }
            for token in &expected {
                let start = Position::new(token.line, token.column);
                let end = Position::new(token.line, token.column + token.length);
                let range = db.semantic_tokens_in_range(&id, DiagnosticRange::new(start, end));
                oracle::assert_stream(&rows(document, range.tokens()), std::slice::from_ref(token));
                let empty = db.semantic_tokens_in_range(&id, DiagnosticRange::new(end, end));
                assert!(empty.tokens().is_empty(), "empty boundary at {end:?}");
            }
            previous = full;
        }
        assert_eq!(previous, original, "repair restores complete stream and ID");
    }
}
