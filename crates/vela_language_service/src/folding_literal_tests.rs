use crate::matrix_fixture::{Document, FixtureWorkspace, folding_literals as oracle};
use crate::{
    DocumentId, LanguageServiceDatabases, SourceFileSnapshot, SourceVersion, Workspace,
    WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};
use serde_json::{Value, json};

fn uri(file: &str) -> String {
    format!("file:///workspace/folding-literals/{file}")
}
fn update(db: &mut LanguageServiceDatabases, disk: &FixtureWorkspace, open: &Workspace) {
    let files = disk
        .disk
        .iter()
        .map(|(file, doc)| SourceFileSnapshot::new(DocumentId::from(uri(file)), doc.text.as_str()))
        .collect::<Vec<_>>();
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from(uri("scripts"))]);
    let snapshot = open.snapshot();
    db.update_with_open_documents(
        &assemble_project_sources(&config, &files, &snapshot),
        &snapshot.open_document_ids().collect(),
    );
}
fn check(db: &LanguageServiceDatabases, main: &Document, helper: &Document, case: &Value) {
    assert_eq!(db.source_db().records().len(), 2);
    for (file, doc) in [("scripts/main.vela", main), ("scripts/helper.vela", helper)] {
        let id = DocumentId::from(uri(file));
        assert_eq!(db.source_db().records()[&id].text(), doc.text);
        assert!(
            db.parse_db()
                .parse_diagnostics(&id)
                .expect("parsed source")
                .is_empty()
        );
    }
    assert_eq!(db.schema_db().facts().types().count(), 0);
    assert_eq!(db.schema_db().facts().functions().count(), 0);
    let before = (
        db.generation(),
        db.parse_db().parse_count(),
        db.project_db().rebuild_count(),
        db.hir_db().rebuild_count(),
    );
    let id = DocumentId::from(uri("scripts/main.vela"));
    for _ in 0..3 {
        let actual = db
            .folding_ranges(&id)
            .iter()
            .map(|range| {
                json!({
                    "kind": format!("{:?}", range.kind()),
                    "start": {"line": range.start().line, "character": range.start().character},
                    "end": {"line": range.end().line, "character": range.end().character},
                })
            })
            .collect();
        assert_eq!(
            Value::Array(actual),
            oracle::expected(case, false),
            "whole authored folding set {}",
            case["id"]
        );
        assert!(
            db.folding_ranges(&DocumentId::from(uri("scripts/helper.vela")))
                .is_empty()
        );
        assert!(
            db.folding_ranges(&DocumentId::from(uri("scripts/missing.vela")))
                .is_empty()
        );
    }
    assert_eq!(
        (
            db.generation(),
            db.parse_db().parse_count(),
            db.project_db().rebuild_count(),
            db.hir_db().rebuild_count()
        ),
        before
    );
}

#[test]
fn folding_literals_pin_complete_literal_operator_and_empty_sets() {
    for crlf in [false, true] {
        for shifted in [false, true] {
            let spec = oracle::spec(crlf, shifted);
            let disk = FixtureWorkspace::new(&spec).expect("physical source model");
            let cases = spec.oracle["cases"].as_array().expect("59 cases");
            let helper = &disk.disk["scripts/helper.vela"];
            let mut db = LanguageServiceDatabases::new();
            let mut workspace = Workspace::new();
            let main_id = DocumentId::from(uri("scripts/main.vela"));
            update(&mut db, &disk, &workspace);
            check(&db, &disk.disk["scripts/main.vela"], helper, &cases[0]);
            let mut frozen = vec![(
                db.clone(),
                disk.disk["scripts/main.vela"].clone(),
                cases[0].clone(),
            )];
            for (index, case) in cases.iter().enumerate() {
                let doc = oracle::document(case);
                workspace.open_document(
                    main_id.clone(),
                    doc.text.as_str(),
                    SourceVersion::new(index as u64 + 1),
                );
                update(&mut db, &disk, &workspace);
                check(&db, &doc, helper, case);
                let mut fresh = LanguageServiceDatabases::new();
                update(&mut fresh, &disk, &workspace);
                check(&fresh, &doc, helper, case);
                for (snapshot, old_doc, old_case) in &frozen {
                    check(snapshot, old_doc, helper, old_case);
                }
                assert_eq!(
                    disk.disk["scripts/main.vela"].text,
                    oracle::document(&cases[0]).text,
                    "dirty source never modifies the disk model"
                );
                frozen.push((db.clone(), doc, case.clone()));
            }
            workspace.close_document(&main_id);
            update(&mut db, &disk, &workspace);
            check(&db, &disk.disk["scripts/main.vela"], helper, &cases[0]);
            let mut fresh = LanguageServiceDatabases::new();
            update(&mut fresh, &disk, &workspace);
            check(&fresh, &disk.disk["scripts/main.vela"], helper, &cases[0]);
            for (snapshot, doc, case) in &frozen {
                check(snapshot, doc, helper, case);
            }
        }
    }
}
