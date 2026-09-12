use crate::matrix_fixture::{FixtureWorkspace, load};
use crate::{
    DocumentId, LanguageServiceDatabases, Position, SourceFileSnapshot, Workspace, WorkspaceConfig,
    WorkspaceRoot, assemble_project_sources,
};

#[test]
fn call_parameter_mapping_matrix_checks_semantic_slots_without_changing_cst_indices() {
    for crlf in [false, true] {
        let mut spec = load("call-parameter-mapping");
        if crlf {
            for text in spec.files.values_mut() {
                *text = text.replace('\n', "\r\n");
            }
        }
        let fixture = FixtureWorkspace::new(&spec).expect("fixture");
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
        assert!(
            db.schema_db().diagnostics().is_empty(),
            "{:?}",
            db.schema_db().diagnostics()
        );
        for case in spec.oracle["queries"].as_array().expect("queries") {
            let file = case["file"].as_str().expect("file");
            let source = fixture.document(file).expect("source");
            let point = source.markers["cursor"].start;
            let position = Position::new(
                point.line,
                point.byte - source.text[..point.byte].rfind('\n').map_or(0, |i| i + 1),
            );
            let help = db
                .signature_help(&uri(file), position)
                .unwrap_or_else(|| panic!("missing signature: {case}"));
            assert_eq!(
                Some(&help),
                db.signature_help(&uri(file), position).as_ref()
            );
            assert_eq!(help.signatures().len(), 1, "{case}");
            assert_eq!(
                help.signatures()[0].label(),
                case["signature"].as_str().expect("label"),
                "{case}"
            );
            assert_eq!(
                help.active_parameter(),
                case["active"].as_u64().expect("active") as usize,
                "{case}"
            );
            let query =
                crate::QueryContext::from_databases(&db, &uri(file), position).expect("query");
            assert_eq!(
                query
                    .call_argument_facts()
                    .expect("call")
                    .active_parameter(),
                case["ordinal"].as_u64().expect("ordinal") as usize,
                "{case}"
            );
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
        }
    }
}

fn uri(file: &str) -> DocumentId {
    DocumentId::from(format!("/workspace/{file}"))
}
