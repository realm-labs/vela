use crate::matrix_fixture::workspace_symbols as oracle;
use crate::{
    DocumentId, LanguageServiceDatabases, SourceFileSnapshot, SymbolRef, Workspace,
    WorkspaceConfig, WorkspaceRoot, WorkspaceSymbol, WorkspaceSymbolLocation,
    assemble_project_sources,
};
use serde_json::{Value, json};

#[test]
fn workspace_symbol_declarations_preserve_whole_owned_query_sets_and_ranges() {
    for crlf in [false, true] {
        for shifted in [false, true] {
            let (fixture, authored) = oracle::fixture(crlf, shifted);
            let uri = |file: &str| format!("/workspace/中文 % workspace symbols/{file}");
            let sources = fixture
                .disk
                .iter()
                .map(|(file, doc)| {
                    SourceFileSnapshot::new(DocumentId::from(uri(file)), doc.text.as_str())
                })
                .collect::<Vec<_>>();
            let config = WorkspaceConfig::workspace([WorkspaceRoot::from(uri("scripts"))]);
            let mut db = LanguageServiceDatabases::new();
            db.update(&assemble_project_sources(
                &config,
                &sources,
                &Workspace::new().snapshot(),
            ));
            for file in fixture.disk.keys() {
                assert!(
                    db.parse_db()
                        .parse_diagnostics(&DocumentId::from(uri(file)))
                        .expect("parsed source")
                        .is_empty()
                );
            }
            let before = db.clone();
            for query in authored["queries"].as_array().expect("32 authored queries") {
                let wanted = oracle::expected(&fixture, &authored, &query["symbols"], false, &uri);
                for repeat in 0..3 {
                    let actual = Value::Array(
                        db.workspace_symbols(query["query"].as_str().expect("query"))
                            .iter()
                            .map(project)
                            .collect(),
                    );
                    assert_eq!(
                        actual, wanted,
                        "crlf={crlf}, shifted={shifted}, query={}, repeat={repeat}",
                        query["id"]
                    );
                }
            }
            for (file, doc) in &fixture.disk {
                assert_eq!(
                    db.source_db().records()[&DocumentId::from(uri(file))].text(),
                    doc.text
                );
            }
            assert_eq!(
                db.workspace_symbols(""),
                before.workspace_symbols(""),
                "read-only query set"
            );
        }
    }
}

fn project(symbol: &WorkspaceSymbol) -> Value {
    let SymbolRef::Source(identity) = symbol.symbol() else {
        panic!("no schema/provider symbol in this source-only corpus")
    };
    let WorkspaceSymbolLocation::Source { document_id, range } = symbol.location() else {
        panic!("actual owned source location")
    };
    assert_eq!(symbol.name_parts().render(), symbol.name());
    assert_eq!(
        symbol
            .detail_parts()
            .map(crate::DisplayParts::render)
            .as_deref(),
        symbol.detail()
    );
    let mut row = json!({"name":symbol.name(),"kind":format!("{:?}",symbol.kind()),"identity":identity,
        "location":{"uri":document_id.as_str(),"range":{"start":{"line":range.start().line,"character":range.start().character},"end":{"line":range.end().line,"character":range.end().character}}}});
    if let Some(detail) = symbol.detail() {
        row["detail"] = json!(detail);
    }
    if let Some(container) = symbol.container_name() {
        row["containerName"] = json!(container);
    }
    row
}
