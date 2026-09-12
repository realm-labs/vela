use crate::matrix_fixture::{FixtureWorkspace, load};
use crate::{
    DocumentId, LanguageServiceDatabases, Position, SourceFileSnapshot, Workspace, WorkspaceConfig,
    WorkspaceRoot, assemble_project_sources,
};

#[test]
fn service_call_matrix_preserves_positional_edits_signatures_and_expected_parameters() {
    for (crlf, mode) in [false, true]
        .into_iter()
        .flat_map(|crlf| ["full", "missing", "ordinary"].map(|mode| (crlf, mode)))
    {
        let mut spec = load("completion-service-calls");
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
        let uri = |file: &str| DocumentId::from(format!("/workspace/{file}"));
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
        for case in spec.oracle["queries"].as_array().expect("queries") {
            let file = case["file"].as_str().expect("file");
            let source = fixture.document(file).expect("source");
            let point = source.markers["cursor"].start;
            let position = Position::new(
                point.line,
                point.byte - source.text[..point.byte].rfind('\n').map_or(0, |i| i + 1),
            );
            let help = db.signature_help(&uri(file), position);
            if let Some(expected) = case["signature"].as_str().filter(|_| mode == "full") {
                let help = help.unwrap_or_else(|| panic!("signature: {case}"));
                assert_eq!(help.signatures().len(), 1, "{case}");
                assert_eq!(help.signatures()[0].label(), expected, "{case}");
                assert_eq!(
                    help.active_parameter(),
                    case["active"].as_u64().expect("active") as usize
                );
            } else {
                assert!(help.is_none(), "{case}: {help:?}");
            }
            let result = db.completion_items(&uri(file), position);
            assert!(
                matches!(
                    result.analysis().kind(),
                    crate::CompletionAnalysisKind::CallArgument(_)
                ),
                "{case}"
            );
            assert_eq!(
                db.completion_items(&uri(file), position),
                result,
                "repeat: {case}"
            );
            assert_eq!(
                result.analysis().expected_name(),
                case["expectedName"].as_str().filter(|_| mode == "full"),
                "{case}"
            );
            assert_eq!(
                result.analysis().expected_type().map(|t| t.display_name()),
                case["expectedType"]
                    .as_str()
                    .filter(|_| mode == "full")
                    .map(str::to_owned),
                "{case}"
            );
            assert_eq!(
                result.items().iter().map(|i| i.label()).collect::<Vec<_>>(),
                ["chosen_value"],
                "{case}"
            );
            let item = &result.items()[0];
            assert_eq!(item.detail(), "i64", "{case}");
            let edit = item.text_edit().expect("edit");
            let range = source.markers["replace"];
            assert_eq!(
                edit.range(),
                crate::TextRange::new(range.start.byte, range.end.byte)
            );
            assert_eq!(edit.new_text(), "chosen_value");
            let mut text = source.text.clone();
            text.replace_range(range.start.byte..range.end.byte, edit.new_text());
            assert!(
                vela_syntax::parse::parse_source(&format!(
                    "{text}{}",
                    case["recoverySuffix"].as_str().unwrap_or("")
                ))
                .diagnostics()
                .is_empty(),
                "{case}: {text}"
            );
            let edited_files = files
                .iter()
                .map(|source| {
                    if source.document_id() == &uri(file) {
                        SourceFileSnapshot::new(uri(file), text.as_str())
                    } else {
                        source.clone()
                    }
                })
                .collect::<Vec<_>>();
            let mut fresh = db.clone();
            fresh.update(&assemble_project_sources(
                &WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]),
                &edited_files,
                &Workspace::new().snapshot(),
            ));
            let again = fresh.completion_items(
                &uri(file),
                Position::new(
                    point.line,
                    range.start.byte
                        - source.text[..range.start.byte]
                            .rfind('\n')
                            .map_or(0, |i| i + 1)
                        + "chosen_value".len(),
                ),
            );
            assert_eq!(
                again.items().iter().map(|i| i.label()).collect::<Vec<_>>(),
                ["chosen_value"],
                "re-query: {case}"
            );
        }
    }
}
