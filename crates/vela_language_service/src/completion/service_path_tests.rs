use crate::matrix_fixture::{FixtureWorkspace, load};
use crate::{
    CompletionSymbol, DocumentId, LanguageServiceDatabases, Position, SourceFileSnapshot,
    TextRange, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn service_path_matrix_owns_exact_candidates_and_edits() {
    for (crlf, mode) in [false, true]
        .into_iter()
        .flat_map(|crlf| ["full", "missing", "ordinary"].map(|mode| (crlf, mode)))
    {
        let mut spec = load("completion-service-paths");
        if crlf {
            for text in spec.files.values_mut() {
                *text = text.replace('\n', "\r\n");
            }
        }
        if mode == "missing" {
            spec.files.remove("schema.json");
        }
        if mode == "ordinary" {
            let mut schema: serde_json::Value =
                serde_json::from_str(&spec.files["schema.json"]).expect("schema");
            schema.as_object_mut().expect("object").remove("serviceSet");
            spec.files
                .insert("schema.json".to_owned(), schema.to_string());
        }
        let fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let db = databases(&fixture);
        for case in spec.oracle["queries"].as_array().expect("queries") {
            let file = case["file"].as_str().expect("file");
            let source = fixture.document(file).expect("source");
            let result = db.completion_items(
                &uri(file),
                position(&source.text, source.markers["cursor"].start.byte),
            );
            let expected = if mode == "full" {
                case["items"].as_array().expect("items").as_slice()
            } else {
                &[]
            };
            let mut labels = result.items().iter().map(|i| i.label()).collect::<Vec<_>>();
            labels.sort_unstable();
            let mut expected_labels = expected
                .iter()
                .map(|i| i["label"].as_str().expect("label"))
                .collect::<Vec<_>>();
            expected_labels.sort_unstable();
            assert_eq!(labels, expected_labels, "{mode}, CRLF={crlf}: {case}");
            assert_eq!(
                db.completion_items(
                    &uri(file),
                    position(&source.text, source.markers["cursor"].start.byte)
                ),
                result
            );
            for expected in expected {
                let item = result
                    .items()
                    .iter()
                    .find(|i| i.label() == expected["label"])
                    .expect("candidate");
                assert_eq!(format!("{:?}", item.kind()), expected["kind"]);
                assert_eq!(item.detail(), expected["detail"].as_str().expect("detail"));
                assert_eq!(
                    item.symbol(),
                    Some(&CompletionSymbol::Schema(
                        expected["symbol"].as_str().expect("symbol").to_owned()
                    ))
                );
                assert!(item.documentation().is_none());
                assert_eq!(
                    serde_json::json!(
                        db.completion_documentation(item.resolve_payload().expect("resolve"))
                    ),
                    expected["docs"]
                );
                assert_eq!(item.lookup(), item.label());
                assert_eq!(item.filter_text(), item.label());
                let range = source.markers["replace"];
                let edit = item.text_edit().expect("explicit edit");
                assert_eq!(
                    edit.range(),
                    TextRange::new(range.start.byte, range.end.byte)
                );
                assert_eq!(
                    edit.new_text(),
                    expected["insert"].as_str().expect("insert")
                );
                assert_eq!(item.insert_text(), Some(edit.new_text()));
                let inserted = edit.new_text().replace("$0", "");
                let mut text = source.text.clone();
                text.replace_range(edit.range().start..edit.range().end, &inserted);
                assert_eq!(
                    text,
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
                let parsed = vela_syntax::parse::parse_source(&format!(
                    "{text}{}",
                    case["recoverySuffix"].as_str().expect("suffix")
                ));
                assert!(
                    parsed.diagnostics().is_empty(),
                    "{case}: {:?}",
                    parsed.diagnostics()
                );
                let mut edited = FixtureWorkspace::new(&spec).expect("edited fixture");
                edited.disk.get_mut(file).expect("file").text = text.clone();
                let fresh = databases(&edited);
                let again = fresh.completion_items(
                    &uri(file),
                    position(&text, range.start.byte + item.label().len()),
                );
                let reapplied = again
                    .items()
                    .iter()
                    .find(|candidate| candidate.label() == item.label())
                    .expect("re-query item");
                assert_eq!(reapplied.symbol(), item.symbol());
                assert_eq!(reapplied.detail(), item.detail());
            }
        }
    }
}

fn uri(file: &str) -> DocumentId {
    DocumentId::from(format!("/workspace/{file}"))
}

fn position(text: &str, byte: usize) -> Position {
    Position::new(
        text[..byte].bytes().filter(|b| *b == b'\n').count(),
        byte - text[..byte].rfind('\n').map_or(0, |i| i + 1),
    )
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
    if let Some(schema) = fixture.disk.get("schema.json") {
        db.load_schema_artifact_json("/workspace/schema.json", &schema.text);
    }
    assert!(db.schema_db().diagnostics().is_empty());
    db
}
