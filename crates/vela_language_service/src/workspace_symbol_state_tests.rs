use crate::matrix_fixture::{
    FixtureWorkspace, workspace_symbol_states as states, workspace_symbols as oracle,
};
use crate::workspace_symbol_ownership_tests::{database, project, uri};
use crate::{
    DocumentId, LanguageServiceDatabases, SourceFileSnapshot, Workspace, WorkspaceConfig,
    WorkspaceRoot, assemble_project_sources,
};
use serde_json::Value;

fn check(db: &LanguageServiceDatabases, fixture: &FixtureWorkspace, authored: &Value) {
    assert_eq!(db.source_db().records().len(), 8);
    assert!(db.schema_db().diagnostics().is_empty());
    assert!(
        db.schema_db()
            .source_locations()
            .type_span("host::Box")
            .is_some()
    );
    assert!(
        db.schema_db()
            .source_locations()
            .field_span("host::Box", "value")
            .is_some()
    );
    assert!(
        db.schema_db()
            .source_locations()
            .method_span("host::Box", "read")
            .is_some()
    );
    assert!(
        db.schema_db()
            .source_locations()
            .function_span("host::make")
            .is_some()
    );
    for (file, doc) in fixture.disk.iter().filter(|(f, _)| f.ends_with(".vela")) {
        let id = DocumentId::from(uri(file));
        assert_eq!(db.source_db().records()[&id].text(), doc.text);
        assert!(
            db.parse_db()
                .parse_diagnostics(&id)
                .expect("parsed source")
                .is_empty()
        );
    }
    let before = db.workspace_symbols("");
    for query in authored["queries"].as_array().expect("queries") {
        let wanted = oracle::expected(fixture, authored, &query["symbols"], false, &uri);
        for _ in 0..3 {
            assert_eq!(
                Value::Array(
                    db.workspace_symbols(query["query"].as_str().expect("query text"))
                        .iter()
                        .map(project)
                        .collect()
                ),
                wanted,
                "query {}",
                query["id"]
            );
        }
    }
    assert_eq!(db.workspace_symbols(""), before);
}

fn run(group: &str) {
    for crlf in [false, true] {
        for shifted in [false, true] {
            let cases = states::cases(group, crlf, shifted);
            let mut db = database(&cases[0].1, &cases[0].2["schema"]);
            let mut frozen = Vec::new();
            for (step, fixture, authored) in &cases {
                let sources = fixture
                    .disk
                    .iter()
                    .filter(|(f, _)| f.ends_with(".vela"))
                    .map(|(file, doc)| {
                        SourceFileSnapshot::new(DocumentId::from(uri(file)), doc.text.as_str())
                    })
                    .collect::<Vec<_>>();
                db.update(&assemble_project_sources(
                    &WorkspaceConfig::workspace([WorkspaceRoot::from(uri("scripts"))]),
                    &sources,
                    &Workspace::new().snapshot(),
                ));
                check(&db, fixture, authored);
                let fresh = database(fixture, &authored["schema"]);
                check(&fresh, fixture, authored);
                for (old, old_fixture, old_authored) in &frozen {
                    check(old, old_fixture, old_authored);
                }
                frozen.push((db.clone(), fixture.clone(), authored.clone()));
                assert_eq!(db.workspace_symbols("").len(), 60, "{group}/{step}");
            }
        }
    }
}

#[test]
fn workspace_symbol_unresolved_states_preserve_whole_owned_sets_without_guessing() {
    run("unresolved");
}

#[test]
fn workspace_symbol_dynamic_states_preserve_whole_owned_sets_without_guessing() {
    run("dynamic");
}
