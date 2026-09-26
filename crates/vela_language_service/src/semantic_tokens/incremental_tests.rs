use std::collections::BTreeMap;

use crate::matrix_fixture::{
    Document, FixtureWorkspace, load, parse_markers, semantic_tokens as oracle,
};
use crate::{
    BackgroundResult, DiagnosticRange, DocumentId, LanguageServiceDatabases, Position,
    SemanticTokens, SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot,
    assemble_project_sources,
};
use serde_json::Value;

#[test]
fn incremental_tokens_follow_fingerprints_dependencies_and_reject_old_results() {
    for crlf in [false, true] {
        let mut spec = load("semantic-token-incremental");
        if crlf {
            for text in spec.files.values_mut() {
                *text = text.replace('\n', "\r\n");
            }
        }
        let mut fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let mut db = fresh(&fixture);
        let original = check(
            &db,
            &fixture,
            &spec.oracle["initial"],
            crlf,
            &BTreeMap::new(),
        );
        let mut previous = original.clone();
        for step in spec.oracle["steps"].as_array().expect("steps") {
            let file = step["file"].as_str().expect("file");
            let id = uri(file);
            let before = db.clone();
            let key = before.source_db().records()[&id].module_key();
            let fingerprint = before
                .parse_db()
                .module_fingerprint(key)
                .expect("fingerprint");
            let main = uri("scripts/main.vela");
            let token = db.begin_background_request();
            let stale_delta = BackgroundResult::new(
                token.clone(),
                db.semantic_token_delta(&main, original["scripts/main.vela"].result_id()),
            );
            let stale_range = BackgroundResult::new(
                token.clone(),
                db.semantic_tokens_in_range(&main, whole(&fixture.disk["scripts/main.vela"])),
            );
            let stale_full = BackgroundResult::new(token, db.semantic_tokens(&main));
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
                [&id]
            );
            assert_eq!(
                db.hir_db().rebuild_count(),
                before.hir_db().rebuild_count() + 1
            );
            assert_eq!(
                db.project_db().rebuild_count() - before.project_db().rebuild_count(),
                usize::from(step["declarationChanged"] == true || step["importChanged"] == true)
            );
            let current = db.parse_db().module_fingerprint(key).expect("fingerprint");
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
            assert!(db.accept_background_result(stale_delta).is_none());
            assert!(db.accept_background_result(stale_range).is_none());
            assert!(db.accept_background_result(stale_full).is_none());
            for (file, prior) in &previous {
                assert_eq!(
                    before.semantic_tokens(&uri(file)),
                    *prior,
                    "immutable pre-edit snapshot"
                );
            }
            previous = check(&db, &fixture, &step["queries"], crlf, &previous);
            for (file, base) in &original {
                assert_delta(&db, file, base, &previous[file]);
            }
        }
        assert_eq!(
            previous, original,
            "complete repair restores streams and result IDs"
        );
    }
}

fn check(
    db: &LanguageServiceDatabases,
    fixture: &FixtureWorkspace,
    queries: &Value,
    crlf: bool,
    previous: &BTreeMap<String, SemanticTokens>,
) -> BTreeMap<String, SemanticTokens> {
    let fresh = fresh(fixture);
    let mut result = BTreeMap::new();
    for (file, query) in queries.as_object().expect("queries") {
        let source = query["source"]
            .as_str()
            .expect("query source")
            .replace('\n', if crlf { "\r\n" } else { "\n" });
        let document = parse_markers(&source).expect("query document");
        assert_eq!(document.text, fixture.disk[file].text);
        let expected = oracle::expected(&document, &query["tokens"], false);
        let id = uri(file);
        let counts = counters(db);
        let full = db.semantic_tokens(&id);
        oracle::assert_stream(
            &super::test_support::rows(&document, full.tokens()),
            &expected,
        );
        assert_eq!(db.semantic_tokens(&id), full);
        assert_eq!(fresh.semantic_tokens(&id), full, "fresh {file}");
        assert!(
            db.semantic_token_delta(&id, full.result_id())
                .edits()
                .is_empty()
        );
        if let Some(prior) = previous.get(file) {
            assert_delta(db, file, prior, &full);
        }
        for row in &expected {
            let start = Position::new(row.line, row.column);
            let end = Position::new(row.line, row.column + row.length);
            let range = db.semantic_tokens_in_range(&id, DiagnosticRange::new(start, end));
            oracle::assert_stream(
                &super::test_support::rows(&document, range.tokens()),
                std::slice::from_ref(row),
            );
            assert_eq!(
                db.semantic_tokens_in_range(&id, DiagnosticRange::new(start, end)),
                range
            );
            assert_eq!(
                fresh.semantic_tokens_in_range(&id, DiagnosticRange::new(start, end)),
                range
            );
            assert!(
                db.semantic_tokens_in_range(&id, DiagnosticRange::new(end, end))
                    .tokens()
                    .is_empty()
            );
        }
        for line in 0..document.text.lines().count() {
            let selected = expected
                .iter()
                .filter(|row| row.line == line)
                .cloned()
                .collect::<Vec<_>>();
            let range = db.semantic_tokens_in_range(
                &id,
                DiagnosticRange::new(Position::new(line, 0), Position::new(line + 1, 0)),
            );
            oracle::assert_stream(
                &super::test_support::rows(&document, range.tokens()),
                &selected,
            );
        }
        let start = Position::new(0, 0);
        assert!(
            db.semantic_tokens_in_range(&id, DiagnosticRange::new(start, start))
                .tokens()
                .is_empty()
        );
        check_cancellation(db, &id, whole(&document), &full);
        assert_eq!(
            counters(db),
            counts,
            "read-only repeated/delta/range/cancellation queries"
        );
        result.insert(file.clone(), full);
    }
    result
}

fn check_cancellation(
    db: &LanguageServiceDatabases,
    id: &DocumentId,
    range: DiagnosticRange,
    full: &SemanticTokens,
) {
    for cancel_first in [false, true] {
        let (token, handle) = db.begin_cancellable_background_request();
        if cancel_first {
            handle.cancel();
        }
        let delta = BackgroundResult::new(token.clone(), db.semantic_token_delta(id, ""));
        let selected = BackgroundResult::new(token.clone(), db.semantic_tokens_in_range(id, range));
        let complete = BackgroundResult::new(token, db.semantic_tokens(id));
        handle.cancel();
        assert!(db.accept_background_result(delta).is_none());
        assert!(db.accept_background_result(selected).is_none());
        assert!(db.accept_background_result(complete).is_none());
    }
    let token = db.begin_background_request();
    assert_eq!(
        db.accept_background_result(BackgroundResult::new(token.clone(), db.semantic_tokens(id))),
        Some(full.clone())
    );
    assert!(
        db.accept_background_result(BackgroundResult::new(
            token.clone(),
            db.semantic_token_delta(id, full.result_id())
        ))
        .expect("current delta")
        .edits()
        .is_empty()
    );
    assert_eq!(
        db.accept_background_result(BackgroundResult::new(
            token,
            db.semantic_tokens_in_range(id, range)
        )),
        Some(full.clone())
    );
}

fn assert_delta(
    db: &LanguageServiceDatabases,
    file: &str,
    previous: &SemanticTokens,
    full: &SemanticTokens,
) {
    let delta = db.semantic_token_delta(&uri(file), previous.result_id());
    let mut applied = previous.tokens().to_vec();
    for edit in delta.edits().iter().rev() {
        assert!(edit.start() + edit.delete_count() <= applied.len());
        applied.splice(
            edit.start()..edit.start() + edit.delete_count(),
            edit.tokens().iter().copied(),
        );
    }
    assert_eq!(applied, full.tokens(), "actual applied delta {file}");
    assert_eq!(delta.result_id(), full.result_id());
}
fn whole(document: &Document) -> DiagnosticRange {
    DiagnosticRange::new(
        Position::new(0, 0),
        Position::new(document.text.lines().count(), 0),
    )
}
fn counters(db: &LanguageServiceDatabases) -> (usize, usize, usize, u64) {
    (
        db.parse_db().parse_count(),
        db.project_db().rebuild_count(),
        db.hir_db().rebuild_count(),
        db.generation().get(),
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
