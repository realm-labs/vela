use serde_json::{Value, json};

use crate::matrix_fixture::{FixtureWorkspace, load, schema_artifact};
use crate::{
    CompletionSymbol, DocumentId, LanguageServiceDatabases, Position, SourceFileSnapshot,
    TextRange, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn callable_return_matrix_preserves_nested_owner_members_and_resolved_docs() {
    assert_member_matrix("completion-callable-returns");
}

#[test]
fn callable_return_schema_lifecycle_refreshes_nested_facts_and_docs() {
    let spec = load("completion-callable-returns");
    let fixture = FixtureWorkspace::new(&spec).expect("fixture");
    let file = "scripts/function.vela";
    let source = fixture.document(file).expect("source");
    let cursor = position(&source.text, source.markers["cursor"].start.byte);
    let call = position(
        &source.text,
        source.text.find("accept(rows)").expect("call") + 7,
    );
    let mut db = databases(&fixture, &spec.oracle["schema"]);
    for step in spec.oracle["lifecycle"].as_array().expect("states") {
        let text = if step["mode"] == "invalid" {
            "{".to_owned()
        } else {
            json!({"formatVersion":1,"facts":step["schema"]}).to_string()
        };
        db.load_schema_artifact_json("/workspace/schema.json", &text);
        assert_eq!(
            !db.schema_db().diagnostics().is_empty(),
            step["mode"] == "invalid"
        );
        let result = db.completion_items(&uri(file), cursor);
        let mut fresh = databases(&fixture, &json!({}));
        fresh.load_schema_artifact_json("/workspace/schema.json", &text);
        assert_eq!(result, fresh.completion_items(&uri(file), cursor), "{step}");
        let actual = result
            .items()
            .iter()
            .map(|item| {
                json!({"label":item.label(),"detail":item.detail(),
            "docs":db.completion_documentation(item.resolve_payload().expect("resolve"))})
            })
            .collect::<Vec<_>>();
        assert_eq!(json!(actual), step["items"], "{step}");
        let signature = db.signature_help(&uri(file), call).expect("signature");
        assert_eq!(
            signature.signatures()[0].label(),
            step["signature"].as_str().expect("label")
        );
        assert_eq!(
            signature,
            fresh
                .signature_help(&uri(file), call)
                .expect("fresh signature")
        );
    }
}

#[test]
fn member_matrix_preserves_exact_owner_sets_docs_edits_and_erased_boundaries() {
    assert_member_matrix("completion-members");
}

#[test]
fn enum_matrix_preserves_variant_identity_fields_edits_and_unknown_boundaries() {
    assert_member_matrix("completion-enums");
}

fn assert_member_matrix(fixture_id: &str) {
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
            let source = fixture.document(file).expect("document");
            let point = source.markers["cursor"].start;
            let range = source.markers["replace"];
            let completion =
                databases.completion_items(&uri(file), position(&source.text, point.byte));
            assert_eq!(
                format!("{:?}", completion.context().kind()),
                query["context"],
                "{query}"
            );
            assert_eq!(
                completion.context().replace_range(),
                TextRange::new(range.start.byte, range.end.byte)
            );
            if let Some(expected_receiver) = query.get("receiver") {
                let crate::CompletionAnalysisKind::DotAccess(dot) = completion.analysis().kind()
                else {
                    panic!("dot analysis");
                };
                assert_eq!(
                    json!(dot.receiver_fact().map(|fact| fact.display_name())),
                    *expected_receiver,
                    "{query}"
                );
                assert!(
                    completion
                        .analysis()
                        .visible_scope()
                        .contains(&"level".to_owned())
                );
            }
            let repeated =
                databases.completion_items(&uri(file), position(&source.text, point.byte));
            assert_eq!(repeated.items(), completion.items());
            let mut actual = completion
                .items()
                .iter()
                .map(|item| item.label())
                .collect::<Vec<_>>();
            actual.sort_unstable();
            let mut expected = query["items"]
                .as_array()
                .expect("items")
                .iter()
                .map(|item| item["label"].as_str().expect("label"))
                .collect::<Vec<_>>();
            expected.sort_unstable();
            assert_eq!(
                actual,
                expected,
                "{}: {:?}",
                query["id"],
                completion.analysis()
            );
            for expected in query["items"].as_array().expect("items") {
                let item = completion
                    .items()
                    .iter()
                    .find(|item| item.label() == expected["label"])
                    .expect("item");
                assert_eq!(format!("{:?}", item.kind()), expected["kind"]);
                assert_eq!(
                    item.detail(),
                    expected["detail"].as_str().expect("detail"),
                    "{query}"
                );
                let symbol = match item.symbol().expect("owned member") {
                    CompletionSymbol::Source(name) => json!({"kind":"source","name":name}),
                    CompletionSymbol::Schema(name) => json!({"kind":"schema","name":name}),
                    other => panic!("unexpected symbol: {other:?}"),
                };
                assert_eq!(
                    symbol,
                    json!({"kind":expected["symbolKind"],"name":expected["symbol"]}),
                    "{query}"
                );
                assert!(item.documentation().is_none());
                assert_eq!(
                    json!(
                        databases
                            .completion_documentation(item.resolve_payload().expect("resolve"))
                    ),
                    expected["docs"]
                );
                assert_eq!(item.lookup(), item.label());
                assert_eq!(item.filter_text(), item.label());
                assert_eq!(
                    item.edit_range(),
                    Some(TextRange::new(range.start.byte, range.end.byte))
                );
                let insertion = item.insert_text().expect("explicit insertion");
                assert_eq!(insertion, expected["insert"].as_str().expect("insertion"));
                {
                    let edit = item.text_edit().expect("explicit member edit");
                    assert_eq!(
                        edit.range(),
                        TextRange::new(range.start.byte, range.end.byte)
                    );
                    assert_eq!(edit.new_text(), insertion);
                }
                if item.label() == query["apply"] {
                    let inserted = insertion.replace("$0", "");
                    let mut edited = source.text.clone();
                    edited.replace_range(range.start.byte..range.end.byte, &inserted);
                    assert_eq!(
                        edited,
                        format!(
                            "{}{}{}",
                            &source.text[..range.start.byte],
                            expected["insert"]
                                .as_str()
                                .expect("insert")
                                .replace("$0", ""),
                            &source.text[range.end.byte..]
                        )
                    );
                    let mut fresh_fixture = FixtureWorkspace::new(&spec).expect("fresh fixture");
                    fresh_fixture.disk.get_mut(file).expect("file").text = edited.clone();
                    let fresh = self::databases(&fresh_fixture, &spec.oracle["schema"]);
                    let again = fresh.completion_items(
                        &uri(file),
                        position(&edited, range.start.byte + item.label().len()),
                    );
                    if query["context"] == "RecordField" {
                        assert!(
                            again.items().is_empty(),
                            "used field must disappear: {query}"
                        );
                    } else {
                        let reapplied = again
                            .items()
                            .iter()
                            .find(|candidate| candidate.label() == item.label())
                            .expect("requery applied member");
                        assert_eq!(reapplied.symbol(), item.symbol());
                        assert_eq!(reapplied.detail(), item.detail());
                    }
                }
            }
        }
    }
}

fn uri(file: &str) -> DocumentId {
    DocumentId::from(format!("/workspace/{file}"))
}
fn position(text: &str, byte: usize) -> Position {
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
    databases.load_schema_artifact_json(
        "/workspace/schema.json",
        &schema_artifact(schema, fixture, |_| panic!("metadata only")).to_string(),
    );
    assert!(
        databases.schema_db().diagnostics().is_empty(),
        "valid schema"
    );
    databases
}
