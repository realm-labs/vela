use crate::matrix_fixture::{FixtureWorkspace, load};
use crate::{
    DocumentId, LanguageServiceDatabases, Position, SourceFileSnapshot, TextRange, Workspace,
    WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn call_expression_matrix_preserves_complete_choices_and_distinct_insertions() {
    verify_fixture("completion-call-expressions");
}

#[test]
fn task_operand_matrix_inserts_calls_and_static_continuation_paths() {
    verify_fixture("completion-task-operands");
}

#[test]
fn task_eligibility_matrix_keeps_static_async_workers_and_matching_continuations() {
    verify_fixture("completion-task-eligibility");
}

fn verify_fixture(name: &str) {
    for crlf in [false, true] {
        let mut spec = load(name);
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
            let result = db.completion_items(&uri(file), position);
            assert_eq!(result, db.completion_items(&uri(file), position));
            let expected = case["items"].as_array().expect("items");
            let mut actual_keys = result
                .items()
                .iter()
                .map(|i| (i.label(), i.insert_text()))
                .collect::<Vec<_>>();
            let mut expected_keys = expected
                .iter()
                .map(|i| {
                    (
                        i["label"].as_str().expect("fixture string"),
                        i["insert"].as_str(),
                    )
                })
                .collect::<Vec<_>>();
            actual_keys.sort();
            expected_keys.sort();
            assert_eq!(actual_keys, expected_keys, "{case}");
            for expected in expected {
                let item = result
                    .items()
                    .iter()
                    .find(|i| {
                        i.label() == expected["label"]
                            && i.insert_text() == expected["insert"].as_str()
                    })
                    .expect("item");
                assert_eq!(format!("{:?}", item.kind()), expected["kind"], "{case}");
                assert_eq!(
                    item.detail(),
                    expected["detail"].as_str().expect("detail"),
                    "{case}"
                );
                assert_eq!(format!("{:?}", item.insert_format()), expected["format"]);
                assert_eq!(item.filter_text(), item.label());
                if let Some(symbol) = expected["symbol"].as_str() {
                    let symbol = if expected["origin"] == "schema" {
                        crate::SymbolRef::Schema(symbol.to_owned())
                    } else {
                        crate::SymbolRef::Source(symbol.to_owned())
                    };
                    assert_eq!(item.symbol(), Some(&symbol));
                    assert!(
                        db.completion_documentation(
                            item.resolve_payload().expect("resolve payload")
                        )
                        .is_none()
                    );
                }
                assert_eq!(
                    item.label_details().description(),
                    expected["description"]
                        .as_str()
                        .or_else(|| expected["named"]
                            .as_bool()
                            .expect("named argument role")
                            .then_some("named argument"))
                );
                let edit = item.text_edit().expect("explicit edit");
                let range = source.markers["replace"];
                assert_eq!(
                    edit.range(),
                    TextRange::new(range.start.byte, range.end.byte),
                    "{case}"
                );
                let mut insertion = edit.new_text().replace("$0", "");
                if insertion.ends_with(" = ") {
                    insertion.push_str(case["value"].as_str().expect("value"));
                }
                let mut text = source.text.clone();
                text.replace_range(edit.range().start..edit.range().end, &insertion);
                assert_eq!(
                    text,
                    format!(
                        "{}{}{}",
                        &source.text[..range.start.byte],
                        insertion,
                        &source.text[range.end.byte..]
                    )
                );
                let parsed = vela_syntax::parse::parse_source(&format!(
                    "{text}{}",
                    case["recoverySuffix"].as_str().expect("suffix")
                ));
                assert!(
                    parsed.diagnostics().is_empty(),
                    "{case}: {:?}",
                    parsed.diagnostics()
                );
                if case["checkTask"] == true {
                    let mut fresh = FixtureWorkspace::new(&spec).expect("fresh fixture");
                    fresh.disk.get_mut(file).expect("file").text = text.clone();
                    let fresh = databases(&fresh);
                    let diagnostics = fresh.diagnostics_for_document(&uri(file));
                    let codes = diagnostics
                        .diagnostics()
                        .iter()
                        .filter_map(|d| d.code())
                        .filter(|code| {
                            code.starts_with("hir::task_")
                                || code.starts_with("analysis::task_")
                                || *code == "analysis::async_call_requires_await"
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(
                        codes,
                        expected["taskDiagnostic"]
                            .as_str()
                            .into_iter()
                            .collect::<Vec<_>>(),
                        "{case} {expected}"
                    );
                    if let Some(code) = expected["taskDiagnostic"].as_str() {
                        let diagnostic = diagnostics
                            .diagnostics()
                            .iter()
                            .find(|d| d.code() == Some(code))
                            .expect("task diagnostic");
                        let marked = source.markers["diagnostic"];
                        let shift =
                            insertion.len() as isize - (range.end.byte - range.start.byte) as isize;
                        let position = |byte: usize| {
                            let byte = byte.checked_add_signed(shift).expect("shift");
                            Position::new(
                                text[..byte].bytes().filter(|b| *b == b'\n').count(),
                                byte - text[..byte].rfind('\n').map_or(0, |i| i + 1),
                            )
                        };
                        let span = diagnostic.range().expect("diagnostic range");
                        assert_eq!(span.start(), position(marked.start.byte));
                        assert_eq!(span.end(), position(marked.end.byte));
                        assert_eq!(
                            diagnostic.severity(),
                            crate::ServiceDiagnosticSeverity::Error
                        );
                    }
                }
            }
        }
    }
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

fn uri(file: &str) -> DocumentId {
    DocumentId::from(format!("/workspace/{file}"))
}
