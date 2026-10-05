use crate::matrix_fixture::{
    FixtureWorkspace, workspace_symbol_incremental as states, workspace_symbols as oracle,
};
use crate::workspace_symbol_ownership_tests::{project as project_symbol, uri};
use crate::{
    BackgroundResult, DocumentId, LanguageServiceDatabases, ProjectSources, SourceFileSnapshot,
    Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};
use serde_json::{Value, json};

fn sources(fixture: &FixtureWorkspace) -> ProjectSources {
    let files = fixture
        .disk
        .iter()
        .map(|(file, doc)| SourceFileSnapshot::new(DocumentId::from(uri(file)), doc.text.as_str()))
        .collect::<Vec<_>>();
    assemble_project_sources(
        &WorkspaceConfig::workspace([WorkspaceRoot::from(uri("scripts"))]),
        &files,
        &Workspace::new().snapshot(),
    )
}

fn fresh(fixture: &FixtureWorkspace) -> LanguageServiceDatabases {
    let mut db = LanguageServiceDatabases::new();
    db.update(&sources(fixture));
    db
}

fn counters(db: &LanguageServiceDatabases) -> (usize, usize, usize, u64) {
    (
        db.parse_db().parse_count(),
        db.project_db().rebuild_count(),
        db.hir_db().rebuild_count(),
        db.generation().get(),
    )
}

fn check(db: &LanguageServiceDatabases, fixture: &FixtureWorkspace, phase: &Value) {
    assert_eq!(db.source_db().records().len(), 5);
    assert!(db.schema_db().diagnostics().is_empty());
    assert!(
        db.schema_db()
            .source_locations()
            .type_span("host::Box")
            .is_none()
    );
    for (file, doc) in &fixture.disk {
        let id = DocumentId::from(uri(file));
        assert_eq!(db.source_db().records()[&id].text(), doc.text);
        assert!(
            db.parse_db()
                .parse_diagnostics(&id)
                .expect("parsed source")
                .is_empty()
        );
    }
    let before = counters(db);
    let authored = &phase["workspace"];
    for query in authored["queries"].as_array().expect("queries") {
        let wanted = oracle::expected(fixture, authored, &query["symbols"], false, &uri);
        for _ in 0..3 {
            assert_eq!(
                Value::Array(
                    db.workspace_symbols(query["query"].as_str().expect("query"))
                        .iter()
                        .map(project_symbol)
                        .collect()
                ),
                wanted,
                "phase {} query {}",
                phase["id"],
                query["id"]
            );
        }
    }
    assert_eq!(counters(db), before, "read-only symbol queries");
}

#[test]
fn workspace_symbol_incremental_body_import_and_declaration_edits_pin_cache_ownership_and_all_old_snapshots()
 {
    for crlf in [false, true] {
        for shifted in [false, true] {
            let cases = states::cases(crlf, shifted);
            let mut db = fresh(&cases[0].0);
            let mut frozen = Vec::new();
            for (index, (fixture, phase)) in cases.iter().enumerate() {
                if index > 0 {
                    let file = phase["file"].as_str().expect("changed file");
                    let id = DocumentId::from(uri(file));
                    let key = db.source_db().records()[&id].module_key().clone();
                    let before = db.clone();
                    let fingerprint = before
                        .parse_db()
                        .module_fingerprint(&key)
                        .expect("old fingerprint");
                    let stale = db.begin_background_request();
                    let old_symbols = db.workspace_symbols("");
                    let report = db.update(&sources(fixture));
                    assert_eq!(
                        report.reparsed_documents().iter().collect::<Vec<_>>(),
                        [&id]
                    );
                    assert_eq!(
                        db.parse_db().parse_count(),
                        before.parse_db().parse_count() + 1
                    );
                    assert_eq!(
                        db.hir_db().rebuild_count(),
                        before.hir_db().rebuild_count() + 1
                    );
                    assert_eq!(
                        db.project_db().rebuild_count() - before.project_db().rebuild_count(),
                        usize::from(
                            phase["declarationChanged"] == true || phase["importChanged"] == true
                        )
                    );
                    let current = db
                        .parse_db()
                        .module_fingerprint(&key)
                        .expect("new fingerprint");
                    assert_eq!(
                        current.declaration() != fingerprint.declaration(),
                        phase["declarationChanged"] == true
                    );
                    assert_eq!(
                        current.import() != fingerprint.import(),
                        phase["importChanged"] == true
                    );
                    let invalidated = report
                        .analysis_invalidated_modules()
                        .iter()
                        .map(|key| key.path.join())
                        .collect::<Vec<_>>();
                    assert_eq!(json!(invalidated), phase["invalidated"], "{phase}");
                    assert_eq!(
                        report.hir_invalidated_modules(),
                        report.analysis_invalidated_modules()
                    );
                    assert!(db.generation() > before.generation());
                    assert!(
                        db.accept_background_result(BackgroundResult::new(
                            stale.clone(),
                            old_symbols.clone()
                        ))
                        .is_none()
                    );
                    assert_eq!(
                        before.accept_background_result(BackgroundResult::new(
                            stale,
                            old_symbols.clone()
                        )),
                        Some(old_symbols)
                    );
                }
                check(&db, fixture, phase);
                check(&fresh(fixture), fixture, phase);
                for (old_db, old_fixture, old_phase) in &frozen {
                    check(old_db, old_fixture, old_phase);
                }
                frozen.push((db.clone(), fixture.clone(), phase.clone()));
            }
        }
    }
}

#[test]
fn workspace_symbol_cancelled_publications_never_escape_or_poison_fresh_owned_results() {
    for crlf in [false, true] {
        for shifted in [false, true] {
            for (fixture, phase) in states::cases(crlf, shifted) {
                let db = fresh(&fixture);
                let before = counters(&db);
                let authored = &phase["workspace"];
                for query in authored["queries"].as_array().expect("queries") {
                    let text = query["query"].as_str().expect("query");
                    let rows = db.workspace_symbols(text);
                    let wanted =
                        oracle::expected(&fixture, authored, &query["symbols"], false, &uri);
                    assert_eq!(
                        Value::Array(rows.iter().map(project_symbol).collect()),
                        wanted
                    );
                    for cancel_before_compute in [false, true] {
                        let (token, handle) = db.begin_cancellable_background_request();
                        if cancel_before_compute {
                            handle.cancel();
                        }
                        let completed = BackgroundResult::new(token, db.workspace_symbols(text));
                        handle.cancel();
                        assert!(db.accept_background_result(completed).is_none());
                        let fresh = db.begin_background_request();
                        assert_eq!(
                            db.accept_background_result(BackgroundResult::new(
                                fresh,
                                db.workspace_symbols(text)
                            )),
                            Some(rows.clone())
                        );
                    }
                }
                assert_eq!(counters(&db), before, "publication gates do not rebuild");
                check(&db, &fixture, &phase);
            }
        }
    }
}
