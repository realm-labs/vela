use crate::matrix_fixture::{FixtureWorkspace, Spec, selection_lifecycle as oracle};
use crate::{
    DocumentId, LanguageServiceDatabases, Position, SourceFileSnapshot, SourceVersion, Workspace,
    WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};
use serde_json::{Value, json};

fn uri(file: &str) -> String {
    format!("file:///workspace/selection-lifecycle/{file}")
}
fn database(fixture: &FixtureWorkspace, db: &mut LanguageServiceDatabases) {
    let sources = fixture
        .disk
        .iter()
        .map(|(file, doc)| SourceFileSnapshot::new(DocumentId::from(uri(file)), doc.text.as_str()))
        .collect::<Vec<_>>();
    let mut open = Workspace::new();
    for (file, doc) in &fixture.open {
        open.open_document(
            DocumentId::from(uri(file)),
            doc.text.as_str(),
            SourceVersion::new(1),
        );
    }
    let snapshot = open.snapshot();
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from(uri("scripts"))]);
    db.update_with_open_documents(
        &assemble_project_sources(&config, &sources, &snapshot),
        &snapshot.open_document_ids().collect(),
    );
}
fn counts(db: &LanguageServiceDatabases) -> (crate::WorkspaceGeneration, usize, usize, usize) {
    (
        db.generation(),
        db.parse_db().parse_count(),
        db.project_db().rebuild_count(),
        db.hir_db().rebuild_count(),
    )
}
fn project(row: &crate::SelectionRange) -> Value {
    let r = row.range();
    let mut result = json!({"range":{"start":{"line":r.start().line,"character":r.start().character},"end":{"line":r.end().line,"character":r.end().character}}});
    if let Some(parent) = row.parent() {
        result["parent"] = project(parent);
    }
    result
}
fn check(db: &LanguageServiceDatabases, spec: &Spec, phase: &Value) {
    let before = counts(db);
    assert_eq!(
        db.source_db().records().len(),
        phase["views"]
            .as_object()
            .expect("views")
            .values()
            .filter(|v| !v.is_null())
            .count()
    );
    assert_eq!(db.schema_db().facts().types().count(), 0);
    assert_eq!(db.schema_db().facts().functions().count(), 0);
    for (file, id) in phase["views"].as_object().expect("views") {
        let key = DocumentId::from(uri(file));
        let variant = id.as_str().map(|id| &spec.oracle["variants"][id]);
        if let Some(variant) = variant {
            let doc = oracle::document(variant);
            assert_eq!(db.source_db().records()[&key].text(), doc.text);
            let parsed = db.parse_db().syntax_parse(&key).expect("owned parse");
            oracle::assert_cst(&parsed.syntax_node(), &doc, variant);
            let errors = db
                .parse_db()
                .parse_diagnostics(&key)
                .expect("parse diagnostics");
            let actual = errors.iter().map(|error| {
                let span = error.span.expect("owned diagnostic span");
                assert_eq!(span.source, db.parse_db().source_id(&key).expect("source id"));
                assert_eq!(format!("{:?}",error.severity), "Error");
                let start = db.source_db().records()[&key].text()[..span.start as usize].chars().filter(|c|*c=='\n').count();
                let end = db.source_db().records()[&key].text()[..span.end as usize].chars().filter(|c|*c=='\n').count();
                let column = |offset| { let text=&doc.text[..offset];offset-text.rfind('\n').map_or(0,|i|i+1) };
                json!({"code":error.code,"message":error.message,"severity":1,"source":"vela","range":{"start":{"line":start,"character":column(span.start as usize)},"end":{"line":end,"character":column(span.end as usize)}}})
            }).collect::<Vec<_>>();
            assert_eq!(
                json!(actual),
                oracle::syntax_diagnostics(variant, false),
                "exact cached diagnostics {}: {file}",
                phase["id"]
            );
        } else {
            assert!(!db.source_db().records().contains_key(&key));
            assert!(db.parse_db().syntax_parse(&key).is_none());
            assert!(db.parse_db().parse_diagnostics(&key).is_none());
        }
        let (positions, expected) = oracle::queries(variant, false);
        let positions = positions
            .as_array()
            .expect("positions")
            .iter()
            .map(|p| {
                Position::new(
                    p["line"].as_u64().expect("line") as usize,
                    p["character"].as_u64().expect("column") as usize,
                )
            })
            .collect::<Vec<_>>();
        for _ in 0..3 {
            assert_eq!(
                json!(
                    db.selection_ranges(&key, &positions)
                        .iter()
                        .map(project)
                        .collect::<Vec<_>>()
                ),
                expected,
                "whole vector {}: {file}",
                phase["id"]
            );
            assert!(db.selection_ranges(&key, &[]).is_empty());
        }
    }
    assert_eq!(counts(db), before);
}

#[test]
fn selection_lifecycle_preserves_whole_current_fresh_and_retained_cst_vectors_through_source_dependency_recovery()
 {
    for crlf in [false, true] {
        for shifted in [false, true] {
            let spec = oracle::spec(crlf, shifted);
            let mut fixture = FixtureWorkspace::new(&spec).expect("disk fixture");
            let mut current = LanguageServiceDatabases::new();
            let mut retained = Vec::new();
            for phase in spec.oracle["phases"].as_array().expect("phases") {
                for action in oracle::actions(&spec, phase) {
                    fixture.apply(&action).expect("finite action");
                }
                oracle::assert_state(&fixture, &spec, phase);
                database(&fixture, &mut current);
                check(&current, &spec, phase);
                let mut fresh = LanguageServiceDatabases::new();
                database(&oracle::fresh(&spec, phase), &mut fresh);
                check(&fresh, &spec, phase);
                for (old, old_phase) in &retained {
                    check(old, &spec, old_phase);
                }
                retained.push((current.clone(), phase.clone()));
            }
        }
    }
}
