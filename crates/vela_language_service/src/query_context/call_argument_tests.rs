use crate::matrix_fixture::{FixtureWorkspace, load};
use crate::{
    CompletionAnalysisKind, DocumentId, LanguageServiceDatabases, Position, QueryContext,
    SourceFileSnapshot, SourceVersion, Workspace, WorkspaceConfig, WorkspaceRoot,
    assemble_project_sources,
};

#[test]
fn call_argument_matrix_uses_owned_separators_for_query_completion_and_signature() {
    for crlf in [false, true] {
        let mut spec = load("call-argument-context");
        if crlf {
            for text in spec.files.values_mut() {
                *text = text.replace('\n', "\r\n");
            }
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
        for case in spec.oracle["queries"].as_array().expect("queries") {
            let file = case["file"].as_str().expect("file");
            let source = fixture.document(file).expect("source");
            if case["validSyntax"] == true {
                assert!(
                    vela_syntax::parse::parse_source(&source.text)
                        .diagnostics()
                        .is_empty(),
                    "{case}: {}",
                    source.text
                );
            }
            let point = source.markers["cursor"].start;
            let position = Position::new(
                point.line,
                point.byte - source.text[..point.byte].rfind('\n').map_or(0, |i| i + 1),
            );
            let mut workspace = Workspace::new();
            workspace.open_document(uri(file), source.text.as_str(), SourceVersion::new(1));
            let snapshot = workspace.snapshot();
            let snapshot_query =
                QueryContext::from_workspace_snapshot(&snapshot, &uri(file), position)
                    .expect("snapshot query");
            let query = QueryContext::from_databases(&db, &uri(file), position).expect("query");
            if case["active"].is_null() {
                assert_eq!(
                    snapshot_query.call_active_parameter_index(),
                    None,
                    "snapshot: {case}"
                );
                assert_eq!(query.call_active_parameter_index(), None, "{case}");
                assert!(query.call_argument_facts().is_none(), "{case}");
                assert!(db.signature_help(&uri(file), position).is_none(), "{case}");
                continue;
            }
            let expected = case["active"].as_u64().expect("active") as usize;
            assert_eq!(
                snapshot_query.call_active_parameter_index(),
                Some(expected),
                "snapshot: {case}"
            );
            assert_eq!(
                query.call_active_parameter_index(),
                Some(expected),
                "{case}"
            );
            assert_eq!(
                query
                    .call_argument_facts()
                    .expect("call facts")
                    .active_parameter(),
                expected,
                "{case}"
            );
            let completion = db.completion_items(&uri(file), position);
            let CompletionAnalysisKind::CallArgument(call) = completion.analysis().kind() else {
                panic!("{case}: {:?}", completion.analysis())
            };
            assert_eq!(call.active_parameter(), expected, "{case}");
            assert_eq!(
                completion.analysis().expected_name(),
                Some(["first", "second", "third"][expected]),
                "{case}"
            );
            let signature = db.signature_help(&uri(file), position).expect("signature");
            assert_eq!(signature.active_parameter(), expected, "{case}");
            assert_eq!(signature.signatures().len(), 1, "{case}");
            let callee = case["callee"].as_str().expect("callee");
            assert!(
                signature.signatures()[0]
                    .label()
                    .starts_with(&format!("{callee}(")),
                "{case}"
            );
        }
    }
}
