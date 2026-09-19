use crate::matrix_fixture::{FixtureWorkspace, load, parse_markers};
use crate::{
    CompletionKind, DocumentId, LanguageServiceDatabases, Position, SourceFileSnapshot, SymbolRef,
    TextRange, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};
use serde_json::Value;

#[test]
fn incremental_completion_links_fingerprints_invalidation_and_current_owned_edits() {
    for crlf in [false, true] {
        let mut spec = load("completion-incremental-ownership");
        if crlf {
            for source in spec.files.values_mut() {
                *source = source.replace('\n', "\r\n");
            }
        }
        let mut fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let mut db = fresh(&fixture);
        assert_candidates(&db, &fixture, &spec.oracle["initial"]);
        for step in spec.oracle["steps"].as_array().expect("steps") {
            let file = step["file"].as_str().expect("file");
            let key = db.source_db().records()[&uri(file)].module_key().clone();
            let before = db.clone();
            let fingerprint = before
                .parse_db()
                .module_fingerprint(&key)
                .expect("fingerprint");
            let stale = db.begin_background_request();
            let source = step["source"]
                .as_str()
                .expect("source")
                .replace('\n', if crlf { "\r\n" } else { "\n" });
            fixture
                .disk
                .insert(file.to_owned(), parse_markers(&source).expect("source"));
            let report = db.update(&project(&fixture));
            assert_eq!(
                db.parse_db().parse_count(),
                before.parse_db().parse_count() + 1,
                "{step}"
            );
            assert_eq!(
                report.reparsed_documents().iter().collect::<Vec<_>>(),
                [&uri(file)]
            );
            assert_eq!(
                db.hir_db().rebuild_count(),
                before.hir_db().rebuild_count() + 1
            );
            assert_eq!(
                db.project_db().rebuild_count() - before.project_db().rebuild_count(),
                usize::from(step["declarationChanged"] == true || step["importChanged"] == true)
            );
            let current = db.parse_db().module_fingerprint(&key).expect("fingerprint");
            assert_eq!(
                current.declaration() != fingerprint.declaration(),
                step["declarationChanged"] == true,
                "{step}"
            );
            assert_eq!(
                current.import() != fingerprint.import(),
                step["importChanged"] == true,
                "{step}"
            );
            let invalidated = report
                .analysis_invalidated_modules()
                .iter()
                .map(|key| key.path.join())
                .collect::<Vec<_>>();
            assert_eq!(
                serde_json::json!(invalidated),
                step["invalidated"],
                "{step}"
            );
            assert_eq!(
                report.hir_invalidated_modules(),
                report.analysis_invalidated_modules()
            );
            assert!(db.generation() > before.generation());
            assert!(
                db.cancellable_completion_items(
                    &uri("scripts/main.vela"),
                    cursor(&fixture),
                    &stale
                )
                .is_none()
            );
            assert_candidates(&db, &fixture, &step["items"]);
        }
    }
}

fn assert_candidates(db: &LanguageServiceDatabases, fixture: &FixtureWorkspace, expected: &Value) {
    let document = uri("scripts/main.vela");
    let source = fixture.document("scripts/main.vela").expect("main");
    let position = cursor(fixture);
    let counts = counters(db);
    let first = db.completion_items(&document, position);
    assert_eq!(first, db.completion_items(&document, position));
    let fresh_token = db.begin_background_request();
    assert_eq!(
        db.cancellable_completion_items(&document, position, &fresh_token),
        Some(first.clone())
    );
    let (token, handle) = db.begin_cancellable_background_request();
    handle.cancel();
    assert!(
        db.cancellable_completion_items(&document, position, &token)
            .is_none()
    );
    assert_eq!(
        db.cancellable_completion_items(&document, position, &fresh_token),
        Some(first.clone())
    );
    assert_eq!(
        counters(db),
        counts,
        "queries and cancellation must not rebuild"
    );
    assert_eq!(first, fresh(fixture).completion_items(&document, position));
    let expected = expected.as_array().expect("items");
    assert_eq!(first.items().len(), expected.len());
    for (item, expected) in first.items().iter().zip(expected) {
        assert_eq!(item.label(), expected["label"]);
        assert_eq!(item.kind(), CompletionKind::Field);
        assert_eq!(item.detail(), expected["detail"]);
        let symbol = SymbolRef::Source(expected["symbol"].as_str().expect("symbol").to_owned());
        assert_eq!(item.symbol(), Some(&symbol));
        assert!(item.documentation().is_none());
        assert!(
            db.completion_documentation(item.resolve_payload().expect("resolve"))
                .is_none()
        );
        let range = source.markers["replace"];
        let edit = item.text_edit().expect("edit");
        assert_eq!(
            edit.range(),
            TextRange::new(range.start.byte, range.end.byte)
        );
        assert_eq!(edit.new_text(), expected["label"]);
        let mut applied = fixture.clone();
        let text = &mut applied
            .disk
            .get_mut("scripts/main.vela")
            .expect("main")
            .text;
        text.replace_range(range.start.byte..range.end.byte, edit.new_text());
        assert!(
            vela_syntax::parse::parse_source(text)
                .diagnostics()
                .is_empty()
        );
        let text = &applied.disk["scripts/main.vela"].text;
        let applied_db = fresh(&applied);
        let definition = applied_db
            .definition(&document, byte_position(text, range.start.byte + 1))
            .expect("definition");
        let file = expected["file"].as_str().expect("target");
        let target = fixture.document(file).expect("target");
        let marker = target.markers[expected["marker"].as_str().expect("marker")];
        assert_eq!(definition.document_id(), &uri(file));
        assert_eq!(definition.symbol(), Some(&symbol));
        assert_eq!(
            definition.range().start(),
            byte_position(&target.text, marker.start.byte)
        );
        assert_eq!(
            definition.range().end(),
            byte_position(&target.text, marker.end.byte)
        );
        let repeated = applied_db.completion_items(
            &document,
            byte_position(text, range.start.byte + edit.new_text().len()),
        );
        assert_eq!(repeated.items().len(), 1);
        assert_eq!(repeated.items()[0].symbol(), Some(&symbol));
        assert_eq!(repeated.items()[0].detail(), expected["detail"]);
    }
    assert_eq!(
        counters(db),
        counts,
        "resolve and applied independent analysis must not mutate live cache"
    );
}

fn counters(db: &LanguageServiceDatabases) -> (usize, usize, usize, u64) {
    (
        db.parse_db().parse_count(),
        db.project_db().rebuild_count(),
        db.hir_db().rebuild_count(),
        db.generation().get(),
    )
}
fn cursor(fixture: &FixtureWorkspace) -> Position {
    let source = fixture.document("scripts/main.vela").expect("main");
    byte_position(&source.text, source.markers["cursor"].start.byte)
}
fn byte_position(text: &str, byte: usize) -> Position {
    Position::new(
        text[..byte].bytes().filter(|c| *c == b'\n').count(),
        byte - text[..byte].rfind('\n').map_or(0, |i| i + 1),
    )
}
fn uri(file: &str) -> DocumentId {
    DocumentId::from(format!("/workspace/{file}"))
}
fn project(fixture: &FixtureWorkspace) -> crate::ProjectSources {
    let files = fixture
        .disk
        .iter()
        .filter(|(file, _)| file.ends_with(".vela"))
        .map(|(file, source)| SourceFileSnapshot::new(uri(file), source.text.as_str()))
        .collect::<Vec<_>>();
    assemble_project_sources(
        &WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]),
        &files,
        &Workspace::new().snapshot(),
    )
}
fn fresh(fixture: &FixtureWorkspace) -> LanguageServiceDatabases {
    let mut db = LanguageServiceDatabases::new();
    db.update(&project(fixture));
    db
}
