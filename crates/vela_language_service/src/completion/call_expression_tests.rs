use crate::matrix_fixture::{FixtureWorkspace, load};
use crate::{
    DocumentId, LanguageServiceDatabases, Position, SourceFileSnapshot, TextRange, Workspace,
    WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn call_expression_matrix_preserves_complete_choices_and_distinct_insertions() {
    for crlf in [false, true] {
        let mut spec = load("completion-call-expressions");
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
        assert!(db.schema_db().diagnostics().is_empty());
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
                assert_eq!(
                    item.label_details().description(),
                    expected["named"]
                        .as_bool()
                        .expect("named argument role")
                        .then_some("named argument")
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
            }
        }
    }
}

fn uri(file: &str) -> DocumentId {
    DocumentId::from(format!("/workspace/{file}"))
}
