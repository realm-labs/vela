use serde_json::{Value, json};

use crate::matrix_fixture::{FixtureWorkspace, load, schema_artifact};
use crate::{
    CompletionContextKind, CompletionSymbol, DocumentId, LanguageServiceDatabases, Position,
    SourceFileSnapshot, TextRange, Workspace, WorkspaceConfig, WorkspaceRoot,
    assemble_project_sources,
};

#[test]
fn type_completion_matrix_preserves_context_ownership_edits_and_resolved_documentation() {
    assert_type_matrix("completion-type-positions");
}

#[test]
fn builtin_type_completion_matrix_covers_public_spellings_and_erased_boundaries() {
    assert_type_matrix("completion-builtin-types");
}

fn assert_type_matrix(fixture_id: &str) {
    for crlf in [false, true] {
        let mut spec = load(fixture_id);
        if crlf {
            for source in spec.files.values_mut() {
                *source = source.replace('\n', "\r\n");
            }
        }
        let fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let databases = databases(&fixture, &spec.oracle["schema"]);
        for query in spec.oracle["queries"].as_array().expect("queries") {
            let file = query["file"].as_str().expect("file");
            let source = fixture.document(file).expect("source");
            let point = source.markers["cursor"].start;
            let position = byte_position(&source.text, point.byte);
            let completion = databases.completion_items(&uri(file), position);
            if let Some(expected) = query.get("typeLocation") {
                let crate::CompletionAnalysisKind::Path(path) = completion.analysis().kind() else {
                    panic!("expected type path: {query}: {:?}", completion.analysis());
                };
                assert_eq!(path.kind(), crate::PathCompletionKind::Type);
                let location = match path.type_location().expect("type location") {
                    crate::TypeLocation::BuiltinTypeArgument {
                        container,
                        argument_index,
                    } => json!({"container":container,"argument":argument_index}),
                    other => json!(format!("{other:?}")),
                };
                assert_eq!(&location, expected, "{query}");
            }
            assert_eq!(
                completion.context().kind(),
                if query["context"] == "Member" {
                    CompletionContextKind::Member
                } else {
                    CompletionContextKind::TypeHint
                },
                "{query}"
            );
            if let Some(expected) = query.get("receiver") {
                let crate::CompletionAnalysisKind::DotAccess(dot) = completion.analysis().kind()
                else {
                    panic!("expected dot analysis");
                };
                assert_eq!(
                    json!(dot.receiver_fact().map(|fact| fact.display_name())),
                    *expected
                );
                assert!(
                    completion
                        .analysis()
                        .visible_scope()
                        .contains(&"powder".to_owned())
                );
            }
            let range = source.markers["replace"];
            assert_eq!(
                completion.context().replace_range(),
                TextRange::new(range.start.byte, range.end.byte),
                "{query}"
            );
            assert_eq!(
                databases.completion_items(&uri(file), position).items(),
                completion.items()
            );
            if let Some(expected) = query["typeInventory"].as_array() {
                let mut actual = completion
                    .items()
                    .iter()
                    .filter(|item| item.kind() == crate::CompletionKind::Type)
                    .map(|item| item.label())
                    .collect::<Vec<_>>();
                actual.sort_unstable();
                assert_eq!(json!(actual), json!(expected));
            }
            if query["empty"] == true {
                assert!(completion.items().is_empty(), "{query}: {completion:?}");
                continue;
            }
            for excluded in query["exclude"].as_array().expect("exclusions") {
                assert!(
                    completion
                        .items()
                        .iter()
                        .all(|item| item.label() != excluded.as_str().expect("label")),
                    "{query}: {completion:?}"
                );
            }
            for expected in query["items"].as_array().expect("items") {
                let matches = completion
                    .items()
                    .iter()
                    .filter(|item| item.label() == expected["label"].as_str().expect("label"))
                    .collect::<Vec<_>>();
                assert_eq!(matches.len(), 1, "{query}: {completion:?}");
                let item = matches[0];
                let symbol = match item.symbol().expect("owned type") {
                    CompletionSymbol::Source(name) => json!({"kind":"source","name":name}),
                    CompletionSymbol::Schema(name) => json!({"kind":"schema","name":name}),
                    CompletionSymbol::Builtin(name) => json!({"kind":"builtin","name":name}),
                    other => panic!("unexpected type owner: {other:?}"),
                };
                assert_eq!(format!("{:?}", item.kind()), expected["kind"]);
                assert_eq!(item.detail(), expected["detail"].as_str().expect("detail"));
                assert_eq!(item.lookup(), expected["lookup"].as_str().expect("lookup"));
                assert_eq!(item.filter_text(), item.lookup());
                assert_eq!(json!(item.label_details().description()), expected["owner"]);
                assert_eq!(
                    symbol,
                    json!({"kind":expected["symbolKind"],"name":expected["symbol"]})
                );
                assert!(item.documentation().is_none());
                assert_eq!(
                    json!(databases.completion_documentation(
                        item.resolve_payload().expect("resolve payload")
                    )),
                    expected["docs"]
                );
                let edit = item.text_edit().expect("explicit type insertion");
                assert_eq!(
                    edit.range(),
                    TextRange::new(range.start.byte, range.end.byte)
                );
                let insertion = expected["insert"].as_str().unwrap_or(item.label());
                assert_eq!(edit.new_text(), insertion);
                assert_eq!(item.insert_text(), Some(insertion));
                if item.label() == query["apply"].as_str().expect("applied candidate") {
                    let mut edited = source.text.clone();
                    edited.replace_range(edit.range().start..edit.range().end, edit.new_text());
                    let expected_text = format!(
                        "{}{}{}",
                        &source.text[..range.start.byte],
                        query["apply"].as_str().expect("apply"),
                        &source.text[range.end.byte..]
                    );
                    assert_eq!(edited, expected_text);
                    let mut fresh_fixture = FixtureWorkspace::new(&spec).expect("fresh fixture");
                    fresh_fixture.disk.get_mut(file).expect("file").text = edited.clone();
                    let fresh = self::databases(&fresh_fixture, &spec.oracle["schema"]);
                    let again = fresh.completion_items(
                        &uri(file),
                        byte_position(
                            &edited,
                            range.start.byte
                                + query["requeryOffset"].as_u64().map_or_else(
                                    || query["apply"].as_str().expect("apply").len(),
                                    |offset| usize::try_from(offset).expect("offset"),
                                ),
                        ),
                    );
                    if let Some(expected) = query["requeryTypeInventory"].as_array() {
                        assert_eq!(again.context().kind(), CompletionContextKind::TypeHint);
                        let mut labels = again
                            .items()
                            .iter()
                            .filter(|candidate| candidate.kind() == crate::CompletionKind::Type)
                            .map(|candidate| candidate.label())
                            .collect::<Vec<_>>();
                        labels.sort_unstable();
                        assert_eq!(json!(labels), json!(expected));
                        assert_eq!(
                            again
                                .items()
                                .iter()
                                .find(|candidate| candidate.label() == item.label())
                                .expect("applied unit")
                                .symbol(),
                            item.symbol()
                        );
                        continue;
                    }
                    assert_eq!(
                        again
                            .items()
                            .iter()
                            .map(|item| item.label())
                            .collect::<Vec<_>>(),
                        [query["apply"].as_str().expect("apply")],
                        "{query}: {again:?}"
                    );
                    assert_eq!(again.items()[0].symbol(), item.symbol());
                }
            }
        }
    }
}

fn uri(file: &str) -> DocumentId {
    DocumentId::from(format!("/workspace/{file}"))
}

fn byte_position(text: &str, byte: usize) -> Position {
    Position::new(
        text[..byte].bytes().filter(|ch| *ch == b'\n').count(),
        byte - text[..byte].rfind('\n').map_or(0, |index| index + 1),
    )
}

fn databases(fixture: &FixtureWorkspace, schema: &Value) -> LanguageServiceDatabases {
    let files = fixture
        .disk
        .iter()
        .filter(|(file, _)| file.ends_with(".vela"))
        .map(|(file, doc)| SourceFileSnapshot::new(uri(file), doc.text.as_str()))
        .collect::<Vec<_>>();
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&assemble_project_sources(
        &config,
        &files,
        &Workspace::new().snapshot(),
    ));
    let artifact = schema_artifact(schema, fixture, |_| panic!("metadata-only schema"));
    databases.load_schema_artifact_json("/workspace/schema.json", &artifact.to_string());
    assert!(
        databases.schema_db().diagnostics().is_empty(),
        "schema fixture must load"
    );
    databases
}
