use crate::matrix_fixture::{
    FixtureWorkspace, Spec, workspace_symbol_sources as states, workspace_symbols as oracle,
};
use crate::workspace_symbol_ownership_tests::{project as project_symbol, uri};
use crate::{
    DocumentId, LanguageServiceDatabases, SourceFileSnapshot, SourceVersion, Workspace,
    WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};
use serde_json::Value;

fn update(
    db: &mut LanguageServiceDatabases,
    fixture: &FixtureWorkspace,
    phase: &Value,
    version: u64,
    roots_changed: bool,
) {
    let files = fixture
        .disk
        .iter()
        .map(|(file, doc)| SourceFileSnapshot::new(DocumentId::from(uri(file)), doc.text.as_str()))
        .collect::<Vec<_>>();
    let mut workspace = Workspace::new();
    for (file, doc) in &fixture.open {
        workspace.open_document(
            DocumentId::from(uri(file)),
            doc.text.as_str(),
            SourceVersion::new(version),
        );
    }
    let roots = phase["roots"].as_array().expect("authored roots");
    let config = if roots.is_empty() && !fixture.open.is_empty() {
        WorkspaceConfig::scratch(DocumentId::from(uri(fixture
            .open
            .keys()
            .next()
            .expect("one scratch document"))))
    } else {
        WorkspaceConfig::workspace(
            roots
                .iter()
                .map(|root| WorkspaceRoot::from(uri(root.as_str().expect("root")))),
        )
    };
    let snapshot = workspace.snapshot();
    let sources = assemble_project_sources(&config, &files, &snapshot);
    let open = snapshot.open_document_ids().collect();
    if roots_changed {
        db.update_after_project_config_change_with_open_documents(&sources, &open);
    } else {
        db.update_with_open_documents(&sources, &open);
    }
}

fn check(db: &LanguageServiceDatabases, fixture: &FixtureWorkspace, phase: &Value) {
    let owned = states::owned_files(phase);
    assert_eq!(
        db.source_db()
            .records()
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>(),
        owned
            .iter()
            .map(|file| DocumentId::from(uri(file)))
            .collect()
    );
    let effective = states::effective(fixture);
    for file in &owned {
        let id = DocumentId::from(uri(file));
        assert_eq!(
            db.source_db().records()[&id].text(),
            effective.disk[file].text
        );
        assert!(
            db.parse_db()
                .parse_diagnostics(&id)
                .expect("parsed source")
                .is_empty()
        );
    }
    assert!(db.schema_db().diagnostics().is_empty());
    assert_eq!(db.schema_db().facts().types().count(), 0);
    assert_eq!(db.schema_db().facts().functions().count(), 0);
    let before = (
        db.generation(),
        db.parse_db().parse_count(),
        db.project_db().rebuild_count(),
        db.hir_db().rebuild_count(),
    );
    for query in phase["workspace"]["queries"].as_array().expect("queries") {
        let wanted = oracle::expected(
            &effective,
            &phase["workspace"],
            &query["symbols"],
            false,
            &uri,
        );
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
    assert_eq!(
        (
            db.generation(),
            db.parse_db().parse_count(),
            db.project_db().rebuild_count(),
            db.hir_db().rebuild_count()
        ),
        before,
        "read-only complete sets"
    );
}

fn run(sequence: &str) {
    for crlf in [false, true] {
        for shifted in [false, true] {
            let spec: Spec = states::spec(crlf, shifted);
            let phases = spec.oracle["sequences"][sequence]
                .as_array()
                .expect("finite phases");
            let mut db = LanguageServiceDatabases::new();
            let mut frozen = Vec::new();
            for (index, phase) in phases.iter().enumerate() {
                let fixture = states::fixture(&spec, phase);
                let roots_changed = index > 0 && phases[index - 1]["roots"] != phase["roots"];
                update(&mut db, &fixture, phase, index as u64 + 1, roots_changed);
                check(&db, &fixture, phase);
                let mut fresh = LanguageServiceDatabases::new();
                update(&mut fresh, &fixture, phase, index as u64 + 1, false);
                check(&fresh, &fixture, phase);
                for (snapshot, old_fixture, old_phase) in &frozen {
                    check(snapshot, old_fixture, old_phase);
                }
                frozen.push((db.clone(), fixture, phase.clone()));
            }
        }
    }
}

#[test]
fn workspace_symbol_configured_roots_disk_rename_and_missing_scratch_pin_whole_owned_sets() {
    run("workspace");
}

#[test]
fn workspace_symbol_no_root_scratch_open_dirty_close_reopen_pin_whole_owned_sets() {
    run("scratch");
}
