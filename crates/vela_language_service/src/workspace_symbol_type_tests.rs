use crate::matrix_fixture::{schema_artifact, workspace_symbols as oracle};
use crate::{
    DocumentId, LanguageServiceDatabases, SourceFileSnapshot, Workspace, WorkspaceConfig,
    WorkspaceRoot, assemble_project_sources,
};
use serde_json::Value;

fn uri(file: &str) -> String {
    format!("/workspace/中文 % workspace types/{file}")
}

#[test]
fn workspace_symbol_type_positions_preserve_complete_hint_details_and_owner_exclusions() {
    for crlf in [false, true] {
        for shifted in [false, true] {
            let (fixture, authored) = oracle::type_positions(crlf, shifted);
            let mut db = LanguageServiceDatabases::new();
            let sources = fixture
                .disk
                .iter()
                .filter(|(file, _)| file.ends_with(".vela"))
                .map(|(file, doc)| {
                    SourceFileSnapshot::new(DocumentId::from(uri(file)), doc.text.as_str())
                })
                .collect::<Vec<_>>();
            let project = assemble_project_sources(
                &WorkspaceConfig::workspace([WorkspaceRoot::from(uri("scripts"))]),
                &sources,
                &Workspace::new().snapshot(),
            );
            db.update(&project);
            let artifact = schema_artifact(&authored["schema"], &fixture, |file| {
                db.source_db().records()[&DocumentId::from(uri(file))]
                    .source_id()
                    .get()
            });
            db.load_schema_artifact_json(&uri("schema.json"), &artifact.to_string());
            assert!(
                db.schema_db().diagnostics().is_empty(),
                "valid static type facts"
            );
            assert_eq!(db.source_db().records().len(), 3);
            for file in fixture.disk.keys().filter(|file| file.ends_with(".vela")) {
                let diagnostics = db
                    .parse_db()
                    .parse_diagnostics(&DocumentId::from(uri(file)))
                    .expect("parsed");
                assert!(
                    diagnostics.is_empty(),
                    "valid syntax {file}: {diagnostics:?}"
                );
            }
            let before = db.clone();
            let mut fresh = LanguageServiceDatabases::new();
            fresh.update(&project);
            fresh.load_schema_artifact_json(&uri("schema.json"), &artifact.to_string());
            for query in authored["queries"].as_array().expect("80 authored queries") {
                let wanted = oracle::expected(&fixture, &authored, &query["symbols"], false, &uri);
                for current in [&db, &fresh] {
                    for repeat in 0..3 {
                        assert_eq!(
                            Value::Array(
                                current
                                    .workspace_symbols(query["query"].as_str().expect("query"))
                                    .iter()
                                    .map(crate::workspace_symbol_ownership_tests::project)
                                    .collect()
                            ),
                            wanted,
                            "crlf={crlf},shifted={shifted},query={},repeat={repeat}",
                            query["id"]
                        );
                    }
                }
            }
            for (file, doc) in &fixture.disk {
                if file.ends_with(".vela") {
                    assert_eq!(
                        db.source_db().records()[&DocumentId::from(uri(file))].text(),
                        doc.text
                    );
                }
            }
            assert_eq!(
                db.workspace_symbols(""),
                before.workspace_symbols(""),
                "immutable source/schema ownership"
            );
        }
    }
}
