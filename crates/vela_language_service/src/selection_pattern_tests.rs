use crate::matrix_fixture::{Document, FixtureWorkspace, selection_patterns as oracle};
use crate::{
    DocumentId, LanguageServiceDatabases, SourceFileSnapshot, SourceVersion, Workspace,
    WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};
use serde_json::{Value, json};

fn uri(file: &str) -> String {
    format!("file:///workspace/selection-patterns/{file}")
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
        assert_eq!(
            db.parse_db()
                .syntax_parse(&id)
                .expect("cached owned CST")
                .syntax_node()
                .text()
                .to_string(),
            doc.text
        );
        assert!(
            db.parse_db()
                .parse_diagnostics(&id)
                .expect("parsed source")
                .is_empty(),
            "{file} {} source={} diagnostics={:?}",
            case["id"],
            doc.text,
            db.parse_db().parse_diagnostics(&id)
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
    for _ in 0..3 {
        for (file, doc, queries) in [
            ("scripts/main.vela", main, &case["queries"]),
            (
                "scripts/helper.vela",
                helper,
                &oracle::spec(false, false).oracle["helperQueries"],
            ),
        ] {
            let positions = oracle::positions(doc, queries, false)
                .as_array()
                .expect("positions")
                .iter()
                .map(|p| {
                    crate::Position::new(
                        p["line"].as_u64().expect("line") as usize,
                        p["character"].as_u64().expect("column") as usize,
                    )
                })
                .collect::<Vec<_>>();
            let actual = db.selection_ranges(&DocumentId::from(uri(file)), &positions);
            assert_eq!(
                Value::Array(actual.iter().map(project).collect()),
                oracle::expected(doc, queries, false),
                "whole authored selection vector {} {file}",
                case["id"]
            );
            assert!(
                db.selection_ranges(&DocumentId::from(uri(file)), &[])
                    .is_empty()
            );
        }
        let missing = DocumentId::from(uri("scripts/missing.vela"));
        assert_eq!(
            db.selection_ranges(&missing, &[crate::Position::new(0, 0)]),
            vec![crate::SelectionRange::new(
                crate::DiagnosticRange::new(crate::Position::new(0, 0), crate::Position::new(0, 0)),
                None
            )]
        );
        assert!(db.selection_ranges(&missing, &[]).is_empty());
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

fn project(selection: &crate::SelectionRange) -> Value {
    let range = selection.range();
    let mut row = json!({"range":{"start":{"line":range.start().line,"character":range.start().character},
        "end":{"line":range.end().line,"character":range.end().character}}});
    if let Some(parent) = selection.parent() {
        row["parent"] = project(parent);
    }
    row
}

#[test]
fn selection_patterns_pin_complete_match_guards_loops_and_point_vectors() {
    for crlf in [false, true] {
        for shifted in [false, true] {
            let spec = oracle::spec(crlf, shifted);
            let disk = FixtureWorkspace::new(&spec).expect("physical source model");
            let cases = spec.oracle["cases"].as_array().expect("79 cases");
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
