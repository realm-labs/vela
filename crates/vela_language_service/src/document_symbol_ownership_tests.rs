use crate::document_symbol_matrix_tests::project;
use crate::matrix_fixture::{FixtureWorkspace, document_symbols as oracle, schema_artifact};
use crate::{
    DocumentId, LanguageServiceDatabases, SourceFileSnapshot, Workspace, WorkspaceConfig,
    WorkspaceRoot, assemble_project_sources,
};
use serde_json::Value;

fn uri(file: &str) -> DocumentId {
    DocumentId::from(format!("/workspace/中文 % outline ownership/{file}"))
}

fn database(fixture: &FixtureWorkspace, facts: &Value) -> LanguageServiceDatabases {
    let mut db = LanguageServiceDatabases::new();
    let sources = fixture
        .disk
        .iter()
        .filter(|(file, _)| file.ends_with(".vela"))
        .map(|(file, doc)| SourceFileSnapshot::new(uri(file), doc.text.as_str()))
        .collect::<Vec<_>>();
    db.update(&assemble_project_sources(
        &WorkspaceConfig::workspace([WorkspaceRoot::from(
            "/workspace/中文 % outline ownership/scripts",
        )]),
        &sources,
        &Workspace::new().snapshot(),
    ));
    let artifact = schema_artifact(facts, fixture, |file| {
        db.source_db().records()[&uri(file)].source_id().get()
    });
    db.load_schema_artifact_json(
        "/workspace/中文 % outline ownership/schema.json",
        &artifact.to_string(),
    );
    assert!(
        db.schema_db().diagnostics().is_empty(),
        "valid bound schema"
    );
    let loaded = db.schema_db().facts();
    assert_eq!(loaded.types().count(), 4);
    assert_eq!(loaded.fields().count(), 5);
    assert_eq!(loaded.methods().count(), 3);
    assert_eq!(loaded.trait_methods().count(), 1);
    assert_eq!(loaded.functions().count(), 2);
    assert_eq!(loaded.variants().count(), 3);
    let locations = db.schema_db().source_locations();
    assert!(locations.type_span("host::Box").is_some());
    assert!(locations.trait_span("host::Readable").is_some());
    assert!(locations.function_span("host::make").is_some());
    assert!(locations.field_span("host::Box", "value").is_some());
    assert!(locations.method_span("host::Box", "read").is_some());
    assert!(
        locations
            .trait_method_span("host::Readable", "read")
            .is_some()
    );
    assert!(locations.variant_span("host::Choice", "Pair").is_some());
    assert!(locations.type_span("metadata::Box").is_none());
    db
}

#[test]
fn document_symbol_members_and_imports_preserve_whole_source_trees_under_schema_collisions() {
    for crlf in [false, true] {
        let (fixture, authored) = oracle::ownership(crlf);
        let db = database(&fixture, &authored["schema"]);
        let fresh = database(&fixture, &authored["schema"]);
        let mut nodes = 0;
        for (file, rows) in authored["trees"].as_object().expect("authored file trees") {
            let doc = &fixture.disk[file];
            assert!(
                db.parse_db()
                    .parse_diagnostics(&uri(file))
                    .expect("parsed")
                    .is_empty(),
                "valid syntax {file}"
            );
            let expected = oracle::expected(doc, rows, false);
            nodes += oracle::assert_ancestry(&expected, None);
            for current in [&db, &fresh] {
                for _ in 0..3 {
                    assert_eq!(
                        Value::Array(
                            current
                                .document_symbols(&uri(file))
                                .iter()
                                .map(project)
                                .collect()
                        ),
                        expected,
                        "whole owned tree {file} crlf={crlf}"
                    );
                }
            }
        }
        assert_eq!(nodes, 46);
        assert!(db.document_symbols(&uri("scripts/missing.vela")).is_empty());
        assert!(db.document_symbols(&uri("schema.json")).is_empty());
    }
}
