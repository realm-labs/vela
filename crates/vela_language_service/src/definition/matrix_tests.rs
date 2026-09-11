use crate::matrix_fixture::{FixtureWorkspace, load};
use crate::{
    DocumentId, LanguageServiceDatabases, Position, SourceFileSnapshot, Workspace, WorkspaceConfig,
    WorkspaceRoot, assemble_project_sources,
};

#[test]
fn navigation_declaration_matrix_pins_all_three_targets_and_negative_policy() {
    for crlf in [false, true] {
        assert_navigation_matrix(crlf);
    }
}

fn assert_navigation_matrix(crlf: bool) {
    let mut spec = load("navigation-declarations");
    if crlf {
        for source in spec.files.values_mut() {
            *source = source.replace('\n', "\r\n");
        }
    }
    let fixture = FixtureWorkspace::new(&spec).expect("fixture");
    let uri = |file: &str| DocumentId::from(format!("/workspace/{file}"));
    let sources = fixture
        .disk
        .iter()
        .map(|(file, document)| SourceFileSnapshot::new(uri(file), document.text.as_str()))
        .collect::<Vec<_>>();
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&assemble_project_sources(
        &config,
        &sources,
        &Workspace::new().snapshot(),
    ));
    for query in spec.oracle["queries"].as_array().expect("query matrix") {
        let file = query["file"].as_str().expect("file");
        let document = fixture.document(file).expect("document");
        let cursor = document.markers[query["cursor"].as_str().expect("cursor")].start;
        let position = byte_position(&document.text, cursor);
        for method in ["definition", "declaration", "type-definition"] {
            let actual = match method {
                "definition" => databases.definition(&uri(file), position),
                "declaration" => databases.declaration(&uri(file), position),
                _ => databases.type_definition(&uri(file), position),
            };
            let label = format!("{}: {method}, CRLF={crlf}", query["id"]);
            let expected = query.get(method).expect("explicit method oracle");
            if expected.is_null() {
                assert!(actual.is_none(), "{label}: {actual:?}");
                continue;
            }
            let target = expected.as_str().expect("target marker or explicit null");
            let target_file = query["target-file"].as_str().unwrap_or(file);
            let target_document = fixture.document(target_file).expect("target document");
            let marker = target_document.markers[target];
            let actual = actual.unwrap_or_else(|| panic!("{label}: target should exist"));
            assert_eq!(actual.document_id(), &uri(target_file), "{label}");
            assert_eq!(
                [actual.range().start(), actual.range().end(),],
                [
                    byte_position(&target_document.text, marker.start),
                    byte_position(&target_document.text, marker.end),
                ],
                "{label}"
            );
        }
    }
}

fn byte_position(text: &str, point: crate::matrix_fixture::Point) -> Position {
    Position::new(
        point.line,
        point.byte - text[..point.byte].rfind('\n').map_or(0, |index| index + 1),
    )
}
