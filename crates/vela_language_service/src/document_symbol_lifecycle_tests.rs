use crate::document_symbol_matrix_tests::project;
use crate::matrix_fixture::{
    FixtureWorkspace, Spec, document_symbol_lifecycle as oracle, document_symbols, schema_artifact,
};
use crate::{
    DocumentId, LanguageServiceDatabases, SourceFileSnapshot, SourceVersion, Workspace,
    WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};
use serde_json::Value;

pub(crate) fn uri(file: &str) -> DocumentId {
    DocumentId::from(format!("/workspace/中文 % outline lifecycle/{file}"))
}

pub(crate) fn update(db: &mut LanguageServiceDatabases, fixture: &FixtureWorkspace, version: u64) {
    let sources = fixture
        .disk
        .iter()
        .filter(|(f, _)| f.ends_with(".vela"))
        .map(|(file, doc)| SourceFileSnapshot::new(uri(file), doc.text.as_str()))
        .collect::<Vec<_>>();
    let mut workspace = Workspace::new();
    for (file, doc) in &fixture.open {
        workspace.open_document(uri(file), doc.text.as_str(), SourceVersion::new(version));
    }
    db.update_with_open_documents(
        &assemble_project_sources(
            &WorkspaceConfig::workspace([WorkspaceRoot::from(
                "/workspace/中文 % outline lifecycle/scripts",
            )]),
            &sources,
            &workspace.snapshot(),
        ),
        &workspace.snapshot().open_document_ids().collect(),
    );
}

pub(crate) fn schema(
    db: &mut LanguageServiceDatabases,
    fixture: &FixtureWorkspace,
    spec: &Spec,
    state: &Value,
) {
    let path = "/workspace/中文 % outline lifecycle/schema.json";
    match state["mode"].as_str().expect("schema mode") {
        "valid" => {
            let artifact = schema_artifact(
                &spec.oracle["schemas"][state["id"].as_str().expect("schema id")],
                fixture,
                |file| db.source_db().records()[&uri(file)].source_id().get(),
            );
            db.load_schema_artifact_json(path, &artifact.to_string());
        }
        "invalid" => {
            db.load_schema_artifact_json(path, state["text"].as_str().expect("invalid artifact"))
        }
        "missing" => db.mark_schema_missing(path),
        mode => panic!("unknown schema mode {mode}"),
    }
}

pub(crate) fn check_schema(db: &LanguageServiceDatabases, spec: &Spec, state: &Value) {
    let facts = db.schema_db().facts();
    match state["mode"].as_str().expect("schema mode") {
        "valid" => {
            assert!(db.schema_db().diagnostics().is_empty());
            assert_eq!(facts.types().count(), 1);
            let expected =
                &spec.oracle["schemas"][state["id"].as_str().expect("schema id")]["fields"][0];
            let fields = facts.fields().collect::<Vec<_>>();
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].name, expected["name"].as_str().expect("field"));
            assert_eq!(
                fields[0].fact.display_name(),
                spec.oracle["schemaFieldDisplays"][state["id"].as_str().expect("schema id")]
                    .as_str()
                    .expect("authored type display")
            );
            assert!(
                db.schema_db()
                    .source_locations()
                    .field_span("host::Box", &fields[0].name)
                    .is_some()
            );
        }
        mode @ ("invalid" | "missing") => {
            assert_eq!(facts.types().count(), 0);
            assert_eq!(facts.fields().count(), 0);
            assert_eq!(db.schema_db().diagnostics().len(), 1);
            let suffix = if mode == "invalid" {
                "is invalid: unsupported schema artifact format version 99; expected 1; host facts degrade to Any"
            } else {
                "is unavailable; host facts degrade to Any"
            };
            assert!(db.schema_db().diagnostics()[0].message().ends_with(suffix));
            assert!(
                db.schema_db()
                    .source_locations()
                    .type_span("host::Box")
                    .is_none()
            );
        }
        mode => panic!("unknown schema mode {mode}"),
    }
}

fn check(db: &LanguageServiceDatabases, spec: &Spec, phase: &Value) {
    for (file, variant) in phase["views"].as_object().expect("views") {
        let expected = variant.as_str().map_or_else(
            || Value::Array(Vec::new()),
            |v| {
                let doc = oracle::document(spec, v);
                let record = &db.source_db().records()[&uri(file)];
                assert_eq!(
                    record.text(),
                    doc.text,
                    "effective source {}: {file}",
                    phase["id"]
                );
                assert!(
                    db.parse_db()
                        .parse_diagnostics(&uri(file))
                        .expect("parsed")
                        .is_empty()
                );
                let expected =
                    document_symbols::expected(&doc, &spec.oracle["variants"][v]["symbols"], false);
                document_symbols::assert_ancestry(&expected, None);
                expected
            },
        );
        if variant.is_null() {
            assert!(!db.source_db().records().contains_key(&uri(file)));
        }
        for _ in 0..3 {
            assert_eq!(
                Value::Array(
                    db.document_symbols(&uri(file))
                        .iter()
                        .map(project)
                        .collect()
                ),
                expected,
                "whole tree {}: {file}",
                phase["id"]
            );
        }
    }
    check_schema(db, spec, &phase["schema"]);
}

#[test]
fn document_symbol_lifecycle_matches_authored_and_fresh_trees_and_keeps_frozen_snapshots() {
    for crlf in [false, true] {
        let spec = oracle::spec(crlf);
        let mut fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let mut db = LanguageServiceDatabases::new();
        let mut frozen = Vec::new();
        for (index, phase) in spec.oracle["phases"]
            .as_array()
            .expect("phases")
            .iter()
            .enumerate()
        {
            for action in oracle::actions(&spec, phase) {
                fixture.apply(&action).expect("finite source action");
            }
            oracle::assert_state(&fixture, &spec, phase);
            update(&mut db, &fixture, index as u64 + 1);
            if index == 0 || !phase["schemaAction"].is_null() {
                schema(&mut db, &fixture, &spec, &phase["schema"]);
            }
            check(&db, &spec, phase);
            let current = oracle::fresh_fixture(&spec, phase);
            let mut fresh = LanguageServiceDatabases::new();
            update(&mut fresh, &current, index as u64 + 1);
            schema(&mut fresh, &current, &spec, &phase["schema"]);
            check(&fresh, &spec, phase);
            for (old, old_phase) in &frozen {
                check(old, &spec, old_phase);
            }
            frozen.push((db.clone(), phase.clone()));
        }
    }
}
