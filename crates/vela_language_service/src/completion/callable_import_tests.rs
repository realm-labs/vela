use crate::matrix_fixture::{FixtureWorkspace, load};
use crate::{
    DocumentId, LanguageServiceDatabases, Position, SourceFileSnapshot, TextRange, Workspace,
    WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn callable_import_matrix_agrees_on_owned_parameters_signatures_and_edits() {
    for crlf in [false, true] {
        let mut spec = load("callable-imports");
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
            let point = source.markers["cursor"].start;
            let position = Position::new(
                point.line,
                point.byte - source.text[..point.byte].rfind('\n').map_or(0, |i| i + 1),
            );
            let help = db.signature_help(&uri(file), position);
            let query =
                crate::QueryContext::from_databases(&db, &uri(file), position).expect("query");
            let call = query.call_argument_facts().expect("call");
            let callables = query.callable_facts_by_path(&db, call.callee_path().expect("callee"));
            assert_eq!(
                callables.len(),
                usize::from(!case["signature"].is_null()),
                "{case}"
            );
            if let Some(callable) = callables.first() {
                let symbol = match callable.symbol() {
                    crate::SymbolRef::Source(name) => ("Source", name),
                    crate::SymbolRef::Schema(name) => ("Schema", name),
                    crate::SymbolRef::Builtin(name) => ("Builtin", name),
                    other => panic!("unexpected symbol: {other:?}"),
                };
                assert_eq!(symbol.0, case["symbolKind"].as_str().expect("symbol kind"));
                assert_eq!(symbol.1, case["symbol"].as_str().expect("symbol"));
            }
            if let Some(signature) = case["signature"].as_str() {
                let help = help.expect("signature");
                assert_eq!(help.signatures().len(), 1, "{case}");
                assert_eq!(help.signatures()[0].label(), signature, "{case}");
                assert_eq!(help.active_parameter(), 0);
            } else {
                assert!(help.is_none(), "{case}: {help:?}");
            }
            let result = db.completion_items(&uri(file), position);
            assert_eq!(result, db.completion_items(&uri(file), position));
            assert_eq!(
                result.analysis().expected_name(),
                case["expectedName"].as_str(),
                "{case}"
            );
            assert_eq!(
                serde_json::json!(result.analysis().expected_type().map(|t| t.display_name())),
                case["expectedType"],
                "{case}"
            );
            let expected = case["parameters"].as_array().expect("parameters");
            let mut labels = result.items().iter().map(|i| i.label()).collect::<Vec<_>>();
            labels.sort_unstable();
            let mut names = expected
                .iter()
                .map(|i| i["name"].as_str().expect("name"))
                .collect::<Vec<_>>();
            names.extend(crate::matrix_fixture::expected_expression_labels(
                &spec.oracle,
                case,
            ));
            names.sort_unstable();
            assert_eq!(labels, names, "{case}");
            for expected in expected {
                let item = result
                    .items()
                    .iter()
                    .find(|i| {
                        i.label() == expected["name"]
                            && i.label_details().description() == Some("named argument")
                    })
                    .expect("item");
                assert_eq!(item.kind(), crate::CompletionKind::Parameter);
                assert_eq!(item.detail(), expected["detail"].as_str().expect("detail"));
                let edit = item.text_edit().expect("explicit edit");
                let range = source.markers["replace"];
                assert_eq!(
                    edit.range(),
                    TextRange::new(range.start.byte, range.end.byte)
                );
                assert_eq!(
                    edit.new_text(),
                    expected["insert"].as_str().expect("insert")
                );
                let mut edited = source.text.clone();
                edited.replace_range(
                    edit.range().start..edit.range().end,
                    &format!("{}1", edit.new_text()),
                );
                assert_eq!(
                    edited,
                    format!(
                        "{}{}1{}",
                        &source.text[..range.start.byte],
                        expected["insert"].as_str().expect("insert"),
                        &source.text[range.end.byte..]
                    )
                );
                let parsed = vela_syntax::parse::parse_source(&format!(
                    "{edited}{}",
                    case["recoverySuffix"].as_str().expect("suffix")
                ));
                assert!(
                    parsed.diagnostics().is_empty(),
                    "{case}: {:?}",
                    parsed.diagnostics()
                );
                let mut fresh = FixtureWorkspace::new(&spec).expect("fresh");
                fresh.disk.get_mut(file).expect("file").text = edited;
                let fresh = databases(&fresh);
                let help = fresh
                    .signature_help(
                        &uri(file),
                        Position::new(
                            position.line,
                            position.character + edit.new_text().len() + 1,
                        ),
                    )
                    .expect("edited signature");
                assert_eq!(help.signatures().len(), 1);
                assert_eq!(
                    help.signatures()[0].label(),
                    case["signature"].as_str().expect("signature")
                );
            }
        }
    }
}

fn uri(file: &str) -> DocumentId {
    DocumentId::from(format!("/workspace/{file}"))
}

fn databases(fixture: &FixtureWorkspace) -> LanguageServiceDatabases {
    let files = fixture
        .disk
        .iter()
        .filter(|(file, _)| file.ends_with(".vela"))
        .map(|(file, source)| SourceFileSnapshot::new(uri(file), source.text.as_str()))
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
