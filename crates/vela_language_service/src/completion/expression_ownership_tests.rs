use crate::matrix_fixture::{FixtureWorkspace, load};
use crate::{
    CompletionAnalysisKind, DocumentId, LanguageServiceDatabases, Position, SourceFileSnapshot,
    SymbolRef, TextRange, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn expression_ownership_matrix_applied_types_resolve_to_the_selected_owner() {
    for crlf in [false, true] {
        let mut spec = load("completion-expression-ownership");
        if crlf {
            for text in spec.files.values_mut() {
                *text = text.replace('\n', "\r\n");
            }
        }
        let fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let db = databases(&fixture);
        for case in spec.oracle["queries"].as_array().expect("queries") {
            let file = case["file"].as_str().expect("file");
            let source = fixture.document(file).expect("source");
            let range = source.markers["replace"];
            let result = db.completion_items(
                &uri(file),
                position(&source.text, source.markers["cursor"].start.byte),
            );
            let expected = case["items"].as_array().expect("items");
            let mut actual = result
                .items()
                .iter()
                .map(|i| {
                    (
                        i.label().to_owned(),
                        i.insert_text().map(str::to_owned),
                        format!("{:?}", i.kind()),
                    )
                })
                .collect::<Vec<_>>();
            let mut keys = expected
                .iter()
                .map(|i| {
                    (
                        string(i, "label").to_owned(),
                        Some(string(i, "insert").to_owned()),
                        string(i, "kind").to_owned(),
                    )
                })
                .collect::<Vec<_>>();
            actual.sort();
            keys.sort();
            assert_eq!(actual, keys, "{case}");
            for expected in expected.iter().filter(|i| i["kind"] == "Type") {
                let item = result
                    .items()
                    .iter()
                    .find(|i| {
                        i.label() == expected["label"]
                            && i.insert_text() == expected["insert"].as_str()
                    })
                    .expect("owned type");
                let symbol = if expected["origin"] == "source" {
                    SymbolRef::Source(string(expected, "symbol").to_owned())
                } else {
                    SymbolRef::Schema(string(expected, "symbol").to_owned())
                };
                assert_eq!(item.symbol(), Some(&symbol));
                assert_eq!(
                    item.filter_text(),
                    expected["lookup"]
                        .as_str()
                        .unwrap_or(string(expected, "symbol"))
                );
                let edit = item.text_edit().expect("edit");
                assert_eq!(
                    edit.range(),
                    TextRange::new(range.start.byte, range.end.byte)
                );
                assert_eq!(edit.new_text(), string(expected, "insert"));
                let mut edited = source.text.clone();
                edited.replace_range(range.start.byte..range.end.byte, edit.new_text());
                assert!(
                    vela_syntax::parse::parse_source(&edited)
                        .diagnostics()
                        .is_empty(),
                    "{case}"
                );
                let mut fresh = FixtureWorkspace::new(&spec).expect("fresh fixture");
                fresh.disk.get_mut(file).expect("file").text = edited.clone();
                let fresh = databases(&fresh);
                let name_start =
                    range.start.byte + edit.new_text().rfind("::").map_or(0, |i| i + 2);
                if let Some(target) = expected["target"].as_str() {
                    let definition = fresh
                        .definition(&uri(file), position(&edited, name_start + 1))
                        .unwrap_or_else(|| panic!("missing definition {case} {expected}"));
                    assert_eq!(definition.symbol(), Some(&symbol));
                    assert_eq!(definition.document_id(), &uri(target));
                    let target_source = fixture.document(target).expect("target");
                    let target_range = target_source.markers["type"];
                    assert_eq!(
                        definition.range().start(),
                        position(&target_source.text, target_range.start.byte)
                    );
                    assert_eq!(
                        definition.range().end(),
                        position(&target_source.text, target_range.end.byte)
                    );
                }
                if case["receiver"] == true {
                    let byte = source.markers["member"].start.byte + edit.new_text().len()
                        - (range.end.byte - range.start.byte);
                    let members = fresh.completion_items(&uri(file), position(&edited, byte));
                    let CompletionAnalysisKind::DotAccess(dot) = members.analysis().kind() else {
                        panic!("dot context")
                    };
                    assert_eq!(
                        dot.receiver_fact().map(|f| f.display_name()),
                        Some(string(expected, "symbol").to_owned()),
                        "{case} {expected}"
                    );
                    assert_eq!(
                        members
                            .items()
                            .iter()
                            .map(|i| i.label())
                            .collect::<Vec<_>>(),
                        ["tag"]
                    );
                    assert_eq!(
                        members.items()[0].detail(),
                        string(expected, "memberDetail"),
                        "{case} {expected}"
                    );
                }
            }
        }
    }
}

fn string<'a>(value: &'a serde_json::Value, key: &str) -> &'a str {
    value[key].as_str().expect("fixture string")
}
fn uri(file: &str) -> DocumentId {
    DocumentId::from(format!("/workspace/{file}"))
}
fn position(text: &str, byte: usize) -> Position {
    Position::new(
        text[..byte].bytes().filter(|c| *c == b'\n').count(),
        byte - text[..byte].rfind('\n').map_or(0, |i| i + 1),
    )
}
fn databases(fixture: &FixtureWorkspace) -> LanguageServiceDatabases {
    let files = fixture
        .disk
        .iter()
        .filter(|(f, _)| f.ends_with(".vela"))
        .map(|(f, s)| SourceFileSnapshot::new(uri(f), s.text.as_str()))
        .collect::<Vec<_>>();
    let mut db = LanguageServiceDatabases::new();
    db.update(&assemble_project_sources(
        &WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]),
        &files,
        &Workspace::new().snapshot(),
    ));
    db.load_schema_artifact_json("/workspace/schema.json", &fixture.disk["schema.json"].text);
    assert!(db.schema_db().diagnostics().is_empty());
    db
}
