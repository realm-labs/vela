use crate::matrix_fixture::{FixtureWorkspace, schema_artifact, workspace_symbols as oracle};
use crate::{
    DocumentId, LanguageServiceDatabases, SourceFileSnapshot, SymbolRef, Workspace,
    WorkspaceConfig, WorkspaceRoot, WorkspaceSymbol, WorkspaceSymbolLocation,
    assemble_project_sources,
};
use serde_json::{Value, json};

fn uri(file: &str) -> String {
    format!("/workspace/中文 % workspace ownership/{file}")
}

fn database(fixture: &FixtureWorkspace, facts: &Value) -> LanguageServiceDatabases {
    let sources = fixture
        .disk
        .iter()
        .filter(|(file, _)| file.ends_with(".vela"))
        .map(|(file, doc)| SourceFileSnapshot::new(DocumentId::from(uri(file)), doc.text.as_str()))
        .collect::<Vec<_>>();
    let mut db = LanguageServiceDatabases::new();
    db.update(&assemble_project_sources(
        &WorkspaceConfig::workspace([WorkspaceRoot::from(uri("scripts"))]),
        &sources,
        &Workspace::new().snapshot(),
    ));
    let artifact = schema_artifact(facts, fixture, |file| {
        db.source_db().records()[&DocumentId::from(uri(file))]
            .source_id()
            .get()
    });
    db.load_schema_artifact_json(&uri("schema.json"), &artifact.to_string());
    assert!(
        db.schema_db().diagnostics().is_empty(),
        "valid bound static schema"
    );
    let locations = db.schema_db().source_locations();
    assert!(locations.type_span("host::Box").is_some());
    assert!(locations.type_span("host::Choice").is_some());
    assert!(locations.trait_span("host::Readable").is_some());
    assert!(locations.function_span("host::make").is_some());
    assert!(locations.field_span("host::Box", "value").is_some());
    assert!(locations.method_span("host::Box", "read").is_some());
    assert!(
        locations
            .trait_method_span("host::Readable", "read")
            .is_some()
    );
    for variant in ["Empty", "Pair", "Named"] {
        assert!(locations.variant_span("host::Choice", variant).is_some());
    }
    assert!(locations.type_span("metadata::Box").is_none());
    assert!(locations.field_span("metadata::Box", "value").is_none());
    assert!(locations.method_span("metadata::Box", "read").is_none());
    assert_eq!(db.source_db().records().len(), 8);
    db
}

#[test]
fn workspace_symbol_members_and_imports_pin_complete_source_and_schema_query_sets() {
    for crlf in [false, true] {
        for shifted in [false, true] {
            let (fixture, authored) = oracle::ownership(crlf, shifted);
            let db = database(&fixture, &authored["schema"]);
            let before = db.clone();
            let fresh = database(&fixture, &authored["schema"]);
            for file in fixture.disk.keys().filter(|file| file.ends_with(".vela")) {
                assert!(
                    db.parse_db()
                        .parse_diagnostics(&DocumentId::from(uri(file)))
                        .expect("parsed")
                        .is_empty(),
                    "valid syntax {file}"
                );
            }
            for query in authored["queries"].as_array().expect("49 authored queries") {
                let wanted = oracle::expected(&fixture, &authored, &query["symbols"], false, &uri);
                for current in [&db, &fresh] {
                    for repeat in 0..3 {
                        assert_eq!(
                            Value::Array(
                                current
                                    .workspace_symbols(query["query"].as_str().expect("query"))
                                    .iter()
                                    .map(project)
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
                "query cannot mutate source or schema ownership"
            );
        }
    }
}

fn project(symbol: &WorkspaceSymbol) -> Value {
    match (symbol.symbol(), symbol.location()) {
        (SymbolRef::Source(_), WorkspaceSymbolLocation::Source { .. }) => {
            let mut row = crate::workspace_symbol_matrix_tests::project(symbol);
            row["ownership"] = json!("Source");
            row
        }
        (SymbolRef::Schema(identity), WorkspaceSymbolLocation::Schema) => {
            assert_eq!(symbol.name_parts().render(), symbol.name());
            assert_eq!(
                symbol
                    .detail_parts()
                    .map(crate::DisplayParts::render)
                    .as_deref(),
                symbol.detail()
            );
            let mut row = json!({"name":symbol.name(),"kind":format!("{:?}",symbol.kind()),"identity":identity,"ownership":"Schema","location":{"schema":true}});
            if let Some(detail) = symbol.detail() {
                row["detail"] = json!(detail);
            }
            if let Some(container) = symbol.container_name() {
                row["containerName"] = json!(container);
            }
            row
        }
        pair => panic!("no builtin/local/invented source-backed metadata rows: {pair:?}"),
    }
}
