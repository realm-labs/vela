use crate::matrix_fixture::document_symbols as oracle;
use crate::{
    DiagnosticRange, DocumentId, DocumentSymbol, LanguageServiceDatabases, SourceFileSnapshot,
    SymbolRef, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};
use serde_json::{Value, json};

#[test]
fn document_symbol_declarations_preserve_complete_ranges_selections_and_ownership() {
    for crlf in [false, true] {
        let (fixture, authored) = oracle::fixture(crlf);
        let uri = |file: &str| DocumentId::from(format!("/workspace/中文 % symbols/{file}"));
        let config =
            WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/中文 % symbols/scripts")]);
        let sources = fixture
            .disk
            .iter()
            .map(|(file, doc)| SourceFileSnapshot::new(uri(file), doc.text.as_str()))
            .collect::<Vec<_>>();
        let mut db = LanguageServiceDatabases::new();
        db.update(&assemble_project_sources(
            &config,
            &sources,
            &Workspace::new().snapshot(),
        ));
        let file = authored["file"].as_str().expect("file");
        assert!(
            db.parse_db()
                .parse_diagnostics(&uri(file))
                .expect("parsed")
                .is_empty()
        );
        let expected = oracle::expected(&fixture.disk[file], &authored["symbols"], false);
        assert_eq!(oracle::assert_ancestry(&expected, None), 33);
        let actual = Value::Array(
            db.document_symbols(&uri(file))
                .iter()
                .map(project)
                .collect(),
        );
        assert_eq!(
            actual, expected,
            "whole ordered source-owned tree crlf={crlf}"
        );
        assert!(db.document_symbols(&uri("scripts/imports.vela")).is_empty());
        assert!(db.document_symbols(&uri("scripts/missing.vela")).is_empty());
    }
}

pub(super) fn project(symbol: &DocumentSymbol) -> Value {
    let SymbolRef::Source(identity) = symbol.symbol() else {
        panic!("source symbol");
    };
    let mut result = json!({"name":symbol.name(),"kind":format!("{:?}",symbol.kind()),"identity":identity,
        "range":range(symbol.range()),"selectionRange":range(symbol.selection_range())});
    assert_eq!(symbol.name_parts().render(), symbol.name());
    assert_eq!(
        symbol
            .detail_parts()
            .map(crate::DisplayParts::render)
            .as_deref(),
        symbol.detail()
    );
    if let Some(detail) = symbol.detail() {
        result["detail"] = json!(detail);
    }
    if !symbol.children().is_empty() {
        result["children"] = Value::Array(symbol.children().iter().map(project).collect());
    }
    result
}

fn range(r: DiagnosticRange) -> Value {
    json!({"start":{"line":r.start().line,"character":r.start().character},"end":{"line":r.end().line,"character":r.end().character}})
}
