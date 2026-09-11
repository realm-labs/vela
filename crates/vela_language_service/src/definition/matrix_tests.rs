use crate::matrix_fixture::{FixtureWorkspace, load, schema_artifact};
use crate::{
    DocumentId, LanguageServiceDatabases, Position, SourceFileSnapshot, Workspace, WorkspaceConfig,
    WorkspaceRoot, assemble_project_sources,
};

#[test]
fn navigation_declaration_matrix_pins_all_three_targets_and_negative_policy() {
    for crlf in [false, true] {
        assert_navigation_matrix("navigation-declarations", crlf);
    }
}

#[test]
fn navigation_member_matrix_pins_source_ownership_and_negative_policy() {
    for crlf in [false, true] {
        assert_navigation_matrix("navigation-members", crlf);
    }
}

#[test]
fn navigation_schema_matrix_distinguishes_source_spans_from_metadata_only() {
    for crlf in [false, true] {
        assert_navigation_matrix("navigation-schema", crlf);
    }
}

fn assert_navigation_matrix(fixture_id: &str, crlf: bool) {
    let mut spec = load(fixture_id);
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
        .filter(|(file, _)| file.ends_with(".vela"))
        .map(|(file, document)| SourceFileSnapshot::new(uri(file), document.text.as_str()))
        .collect::<Vec<_>>();
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&assemble_project_sources(
        &config,
        &sources,
        &Workspace::new().snapshot(),
    ));
    if let Some(facts) = spec.oracle.get("schema") {
        let artifact = schema_artifact(facts, &fixture, |file| {
            databases.source_db().records()[&uri(file)]
                .source_id()
                .get()
        });
        databases.load_schema_artifact_json("/workspace/target/schema.json", &artifact.to_string());
    }
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
    if let Some(cases) = spec.oracle["invalidSchemaSpans"].as_array() {
        let artifact = schema_artifact(&spec.oracle["schema"], &fixture, |file| {
            databases.source_db().records()[&uri(file)]
                .source_id()
                .get()
        });
        let document = fixture.document("scripts/main.vela").expect("caller");
        let position = byte_position(&document.text, document.markers["box-hint"].start);
        for case in cases {
            let mut invalid = artifact.clone();
            invalid["facts"]["types"][0]["sourceSpan"]
                .as_object_mut()
                .expect("span")
                .extend(case["patch"].as_object().expect("span patch").clone());
            databases
                .load_schema_artifact_json("/workspace/target/schema.json", &invalid.to_string());
            for actual in [
                databases.definition(&uri("scripts/main.vela"), position),
                databases.declaration(&uri("scripts/main.vela"), position),
                databases.type_definition(&uri("scripts/main.vela"), position),
            ] {
                assert!(
                    actual.is_none(),
                    "{} CRLF={crlf}: invalid schema span must not navigate: {actual:?}",
                    case["id"]
                );
            }
            databases
                .load_schema_artifact_json("/workspace/target/schema.json", &artifact.to_string());
            let expected = fixture.document("scripts/origins.vela").expect("origin");
            for actual in [
                databases.definition(&uri("scripts/main.vela"), position),
                databases.declaration(&uri("scripts/main.vela"), position),
                databases.type_definition(&uri("scripts/main.vela"), position),
            ] {
                let actual = actual.expect("valid schema restoration must recover navigation");
                assert_eq!(actual.document_id(), &uri("scripts/origins.vela"));
                assert_eq!(
                    actual.range().start(),
                    byte_position(&expected.text, expected.markers["box"].start)
                );
                assert_eq!(
                    actual.range().end(),
                    byte_position(&expected.text, expected.markers["box"].end)
                );
            }
        }
    }
}

fn byte_position(text: &str, point: crate::matrix_fixture::Point) -> Position {
    Position::new(
        point.line,
        point.byte - text[..point.byte].rfind('\n').map_or(0, |index| index + 1),
    )
}
