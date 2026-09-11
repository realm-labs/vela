use crate::matrix_fixture::{FixtureWorkspace, load, schema_artifact};
use crate::{
    DocumentId, LanguageServiceDatabases, Position, SourceFileSnapshot, Workspace, WorkspaceConfig,
    WorkspaceRoot, assemble_project_sources,
};

#[test]
fn navigation_owned_declaration_matrix_preserves_exact_names_and_body_bindings() {
    for crlf in [false, true] {
        assert_navigation_matrix("navigation-owned-declarations", crlf);
    }
}

#[test]
fn navigation_constructor_matrix_preserves_field_label_and_value_ownership() {
    for crlf in [false, true] {
        assert_navigation_matrix("navigation-constructors", crlf);
    }
}

#[test]
fn navigation_call_matrix_preserves_parameter_ownership_and_unknown_boundaries() {
    for crlf in [false, true] {
        assert_navigation_matrix("navigation-calls", crlf);
    }
}

#[test]
fn navigation_import_matrix_preserves_path_alias_visibility_and_value_ownership() {
    for crlf in [false, true] {
        assert_navigation_matrix("navigation-imports", crlf);
    }
}

#[test]
fn navigation_recovery_matrix_keeps_valid_neighbors_and_rejects_incomplete_targets() {
    for crlf in [false, true] {
        assert_navigation_matrix("navigation-recovery", crlf);
    }
}

#[test]
fn navigation_type_position_matrix_covers_builtin_nested_and_declaration_hints() {
    for crlf in [false, true] {
        assert_navigation_matrix("navigation-type-positions", crlf);
    }
}

#[test]
fn navigation_dynamic_matrix_preserves_known_any_return_boundaries() {
    for crlf in [false, true] {
        assert_navigation_matrix("navigation-dynamic", crlf);
    }
}

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
    let mut databases = fixture_databases(&fixture);
    if let Some(facts) = spec.oracle.get("schema") {
        let artifact = schema_artifact(facts, &fixture, |file| {
            databases.source_db().records()[&uri(file)]
                .source_id()
                .get()
        });
        databases.load_schema_artifact_json("/workspace/target/schema.json", &artifact.to_string());
    }
    if let Some(cases) = spec.oracle["knownCallables"].as_array() {
        for case in cases {
            let file = case["file"].as_str().expect("file");
            let document = fixture.document(file).expect("source");
            let point = document.markers[case["cursor"].as_str().expect("cursor")].start;
            let hover = databases
                .hover(&uri(file), byte_position(&document.text, point))
                .expect("known callable must resolve before asserting source-navigation null");
            assert_eq!(hover.label(), case["label"].as_str().expect("label"));
            assert!(matches!(hover.symbol(), Some(crate::SymbolRef::Builtin(_))));
            assert_eq!(format!("{:?}", hover.kind()).to_lowercase(), case["kind"]);
        }
    }
    if let Some(cases) = spec.oracle["diagnosticCandidates"].as_array() {
        for case in cases {
            let diagnostics =
                databases.diagnostics_for_document(&uri(case["file"].as_str().expect("file")));
            assert!(
                diagnostics.diagnostics().iter().any(|diagnostic| {
                    diagnostic.code() == case["code"].as_str()
                        && diagnostic.candidates().iter().any(|candidate| {
                            Some(candidate.replacement()) == case["replacement"].as_str()
                        })
                }),
                "expected a real diagnostic candidate before rejecting its navigation: {diagnostics:?}"
            );
        }
    }
    assert_queries(
        &databases,
        &fixture,
        &spec.oracle["queries"],
        &format!("CRLF={crlf}"),
    );
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

pub(super) fn fixture_databases(fixture: &FixtureWorkspace) -> LanguageServiceDatabases {
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
    databases
}

pub(super) fn assert_queries(
    databases: &LanguageServiceDatabases,
    fixture: &FixtureWorkspace,
    queries: &serde_json::Value,
    context: &str,
) {
    for query in queries.as_array().expect("query matrix") {
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
            let label = format!("{}: {method}, {context}", query["id"]);
            let expected = query.get(method).expect("explicit method oracle");
            if expected.is_null() {
                assert!(actual.is_none(), "{label}: {actual:?}");
                continue;
            }
            let target = expected.as_str().expect("target marker or explicit null");
            let target_file = query["target-files"][method]
                .as_str()
                .or_else(|| query["target-file"].as_str())
                .unwrap_or(file);
            let target_document = fixture.document(target_file).expect("target document");
            let marker = target_document.markers[target];
            let actual = actual.unwrap_or_else(|| panic!("{label}: target should exist"));
            if method != "type-definition"
                && let Some(symbol) = query["source-symbol"].as_str()
            {
                assert_eq!(
                    actual.symbol(),
                    Some(&crate::SymbolRef::Source(symbol.to_owned())),
                    "{label}"
                );
            }
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

pub(super) fn uri(file: &str) -> DocumentId {
    DocumentId::from(format!("/workspace/{file}"))
}
