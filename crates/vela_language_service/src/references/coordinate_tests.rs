use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::matrix_fixture::{FixtureWorkspace, Point, Spec, references as oracle};
use crate::{
    DiagnosticRange, DocumentId, LanguageServiceDatabases, Position, SourceFileSnapshot, SymbolRef,
    TextRange, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn reference_rename_coordinate_matrix_preserves_sets_owners_and_applied_edits() {
    run_matrix(oracle::spec);
}

#[test]
fn named_parameter_matrix_preserves_sets_owners_and_applied_edits() {
    run_matrix(oracle::named_spec);
}

#[test]
fn method_parameter_matrix_preserves_sets_owners_and_applied_edits() {
    run_matrix(oracle::method_parameter_spec);
}

#[test]
fn required_parameter_matrix_preserves_sets_owners_and_applied_edits() {
    run_matrix(oracle::required_parameter_spec);
}

#[test]
fn record_field_matrix_preserves_sets_owners_and_applied_edits() {
    run_matrix(oracle::field_spec);
}

#[test]
fn variant_field_matrix_preserves_sets_owners_and_applied_edits() {
    run_matrix(oracle::variant_field_spec);
}

#[test]
fn tuple_field_matrix_preserves_sets_owners_and_applied_edits() {
    run_matrix(oracle::tuple_field_spec);
}

#[test]
fn default_binding_matrix_preserves_sets_owners_and_applied_edits() {
    run_matrix(oracle::default_binding_spec);
}

#[test]
fn schema_method_matrix_preserves_sets_owners_and_applied_edits() {
    run_matrix(oracle::schema_method_spec);
}

#[test]
fn schema_function_matrix_preserves_sets_owners_and_applied_edits() {
    run_matrix(oracle::schema_function_spec);
}

#[test]
fn schema_import_matrix_preserves_aliases_and_applied_edits() {
    run_matrix(oracle::schema_import_spec);
}

#[test]
fn import_boundary_matrix_preserves_public_aliases_and_excludes_private_stdlib_imports() {
    run_matrix(oracle::import_boundary_spec);
}

#[test]
fn schema_capture_matrix_rejects_changed_owners_and_applies_safe_edits() {
    run_matrix(oracle::schema_capture_spec);
}

#[test]
fn schema_lookup_matrix_preserves_unknown_and_exact_owners_after_rename() {
    run_matrix(oracle::schema_lookup_spec);
}

#[test]
fn schema_variant_matrix_preserves_expression_and_pattern_ownership() {
    run_matrix(oracle::schema_variant_spec);
}

#[test]
fn schema_variant_import_matrix_preserves_aliases_and_applied_edits() {
    run_matrix(oracle::schema_variant_import_spec);
}

#[test]
fn schema_variant_capture_matrix_rejects_changed_owners_and_applies_safe_edits() {
    run_matrix(oracle::schema_variant_capture_spec);
}

#[test]
fn schema_variant_lookup_matrix_preserves_unknown_and_short_name_owners() {
    run_matrix(oracle::schema_variant_lookup_spec);
}

#[test]
fn schema_variant_ambiguity_matrix_preserves_unresolved_paths() {
    run_matrix(oracle::schema_variant_ambiguity_spec);
}

#[test]
fn source_variant_import_matrix_preserves_source_and_schema_owners() {
    run_matrix(oracle::source_variant_import_spec);
}

#[test]
fn private_variant_import_matrix_preserves_aliases_and_applied_edits() {
    run_matrix(oracle::private_variant_import_spec);
}

#[test]
fn variant_ambiguity_removal_matrix_preserves_unresolved_imports() {
    run_matrix(oracle::variant_ambiguity_removal_spec);
}

#[test]
fn schema_field_matrix_preserves_sets_owners_and_applied_edits() {
    run_matrix(oracle::schema_field_spec);
}

fn run_matrix(spec_for: fn(bool) -> Spec) {
    for crlf in [false, true] {
        let spec = spec_for(crlf);
        let fixture = FixtureWorkspace::new(&spec).expect("fixture");
        oracle::assert_parsed(&fixture);
        let mut db = LanguageServiceDatabases::new();
        update(&mut db, &fixture);
        load_schema(&mut db, &spec, &fixture);
        check_queries(&db, &spec, &fixture);
        for (group, definition) in spec.oracle["groups"].as_object().expect("groups") {
            if definition["readonly"] == true {
                continue;
            }
            let site = &definition["sites"][0];
            let file = site["file"].as_str().expect("file");
            let marker = site["marker"].as_str().expect("marker");
            let new_name = definition["rename"].as_str().expect("new name");
            let edit = db
                .rename(&uri(file), position(&fixture, file, marker), new_name)
                .expect("rename");
            let expected = oracle::renamed(&spec, group, new_name);
            let mut applied = FixtureWorkspace::new(&expected).expect("renamed fixture");
            let mut actual: BTreeMap<_, _> = fixture
                .disk
                .iter()
                .map(|(file, doc)| (uri(file), doc.text.clone()))
                .collect();
            for document in edit.document_edits() {
                let text = actual.get_mut(document.document_id()).expect("owned file");
                let mut edits = document.edits().iter().collect::<Vec<_>>();
                edits.sort_by_key(|edit| {
                    (edit.range().start().line, edit.range().start().character)
                });
                for edit in edits.into_iter().rev() {
                    let start = byte_offset(text, edit.range().start());
                    let end = byte_offset(text, edit.range().end());
                    text.replace_range(start..end, edit.new_text());
                }
            }
            for (file, document) in &mut applied.disk {
                assert_eq!(actual[&uri(file)], document.text, "applied {group}/{file}");
                document.text = actual[&uri(file)].clone();
            }
            oracle::assert_parsed(&applied);
            update(&mut db, &applied);
            load_schema(&mut db, &expected, &applied);
            check_queries(&db, &expected, &applied);
            update(&mut db, &fixture);
            load_schema(&mut db, &spec, &fixture);
            check_queries(&db, &spec, &fixture);
        }
    }
}

fn check_queries(db: &LanguageServiceDatabases, spec: &Spec, fixture: &FixtureWorkspace) {
    for check in spec.oracle["localChecks"].as_array().into_iter().flatten() {
        let file = check["file"].as_str().expect("file");
        let query = oracle::local_marker(spec, fixture, check, false);
        let target = oracle::local_marker(spec, fixture, check, true);
        let definition = db
            .definition(
                &uri(file),
                byte_point(&fixture.disk[file].text, query.start),
            )
            .expect("preserved local binding");
        assert_eq!(definition.document_id(), &uri(file));
        assert_eq!(
            definition.range(),
            DiagnosticRange::new(
                byte_point(&fixture.disk[file].text, target.start),
                byte_point(&fixture.disk[file].text, target.end)
            )
        );
        let prepared = db
            .prepare_rename(
                &uri(file),
                byte_point(&fixture.disk[file].text, query.start),
            )
            .expect("local rename");
        assert_eq!(
            prepared.symbol(),
            &SymbolRef::local_at(
                "value",
                uri(file),
                TextRange::new(target.start.byte, target.end.byte)
            )
        );
    }
    for query in spec.oracle["queries"].as_array().expect("queries") {
        let file = query["file"].as_str().expect("query file");
        let marker = query["marker"].as_str().expect("query marker");
        let point = position(fixture, file, marker);
        let sites = oracle::sites(spec, query);
        let symbol = query["group"].as_str().map(|group| {
            let definition = &spec.oracle["groups"][group];
            if definition["origin"] == "schema" {
                SymbolRef::Schema(
                    definition["qualified"]
                        .as_str()
                        .expect("schema owner")
                        .into(),
                )
            } else if definition["origin"] == "source" {
                SymbolRef::Source(definition["qualified"].as_str().expect("qualified").into())
            } else {
                let declaration = &definition["sites"][0];
                let file = declaration["file"].as_str().expect("declaration file");
                let marker = declaration["marker"].as_str().expect("declaration marker");
                let range = fixture.disk[file].markers[marker];
                SymbolRef::local_at(
                    definition["name"].as_str().expect("name"),
                    uri(file),
                    TextRange::new(range.start.byte, range.end.byte),
                )
            }
        });
        for _ in 0..2 {
            for include in [false, true] {
                let references = db.references(&uri(file), point, include);
                let actual = references.iter().map(|reference| {
                    assert_eq!(Some(reference.symbol()), symbol.as_ref(), "owner {marker}");
                    json!({"uri":reference.document_id().as_str(),"range":range_json(reference.range()),"kind":format!("{:?}",reference.kind())})
                }).collect::<Vec<_>>();
                let expected = sites.iter().filter(|site| include || site["kind"] != "Declaration").map(|site| {
                    json!({"uri":uri(site["file"].as_str().expect("fixture string")).as_str(),"range":site_range(fixture,site),"kind":site["kind"]})
                }).collect::<Vec<_>>();
                assert_eq!(
                    sorted(actual),
                    sorted(expected),
                    "references {marker}, include={include}"
                );
            }
            let actual = db.document_highlights(&uri(file), point).iter().map(|highlight|
                json!({"range":range_json(highlight.range()),"kind":format!("{:?}",highlight.kind())})).collect();
            let expected = sites
                .iter()
                .filter(|site| site["file"] == file)
                .map(|site| {
                    let kind = match site["kind"].as_str().expect("fixture string") {
                        "Read" => "Read",
                        "Write" => "Write",
                        "Call" => "Call",
                        _ => "Text",
                    };
                    json!({"range":site_range(fixture,site),"kind":kind})
                })
                .collect();
            assert_eq!(sorted(actual), sorted(expected), "highlights {marker}");
            let prepared = db.prepare_rename(&uri(file), point);
            let renamed = db.rename(&uri(file), point, "renamed_symbol");
            if let Some(group) = query["group"].as_str() {
                if let Some(target) = spec.oracle["groups"][group]["typeTarget"].as_object() {
                    let target = &json!(target);
                    let definition = db
                        .type_definition(&uri(file), point)
                        .unwrap_or_else(|| panic!("owned type definition {marker}"));
                    assert_eq!(
                        definition.document_id(),
                        &uri(target["file"].as_str().expect("file"))
                    );
                    assert_eq!(range_json(definition.range()), site_range(fixture, target));
                }
                if spec.oracle["groups"][group]["readonly"] == true {
                    assert!(prepared.is_none());
                    assert!(renamed.is_none());
                    let definition = db.definition(&uri(file), point);
                    if spec.oracle["groups"][group]["sourceDefinition"] == true {
                        let target = &sites[0];
                        let definition = definition.expect("read-only source definition");
                        assert_eq!(
                            definition.document_id(),
                            &uri(target["file"].as_str().expect("file"))
                        );
                        assert_eq!(range_json(definition.range()), site_range(fixture, target));
                    } else {
                        assert!(definition.is_none());
                    }
                    continue;
                }
                if spec.oracle["checkDefinition"] == true {
                    let site = &sites[0];
                    let definition = db.definition(&uri(file), point).expect("owned definition");
                    assert_eq!(
                        definition.document_id(),
                        &uri(site["file"].as_str().expect("file"))
                    );
                    assert_eq!(range_json(definition.range()), site_range(fixture, site));
                }
                for rejected in oracle::rejections(spec, group) {
                    assert!(
                        db.rename(&uri(file), point, rejected.as_str().expect("name"))
                            .is_none(),
                        "collision {marker}: {rejected}"
                    );
                }
                let prepared = prepared.unwrap_or_else(|| panic!("prepare rename {marker}"));
                assert_eq!(prepared.document_id(), &uri(file));
                assert_eq!(range_json(prepared.range()), site_range(fixture, query));
                assert_eq!(
                    prepared.placeholder(),
                    spec.oracle["groups"][group]["name"]
                        .as_str()
                        .expect("fixture string")
                );
                assert_eq!(Some(prepared.symbol()), symbol.as_ref());
                let renamed = renamed.expect("rename plan");
                assert_eq!(renamed.symbol(), symbol.as_ref());
                if spec.oracle["groups"][group]["origin"] == "schema" {
                    assert!(
                        renamed
                            .risks()
                            .iter()
                            .any(|risk| risk.kind() == crate::RenameRiskKind::SchemaAbi)
                    );
                }
                let actual = renamed.document_edits().iter().flat_map(|document| document.edits().iter().map(|edit|
                    json!({"uri":document.document_id().as_str(),"range":range_json(edit.range()),"newText":edit.new_text()}))).collect();
                let expected = oracle::edits(spec, group).iter().map(|site| json!({"uri":uri(site["file"].as_str().expect("fixture string")).as_str(),"range":site_range(fixture,site),"newText":oracle::replacement(site,"renamed_symbol")})).collect();
                assert_eq!(sorted(actual), sorted(expected), "rename {marker}");
            } else {
                assert!(
                    prepared.is_none(),
                    "negative prepare {marker}: {prepared:?}"
                );
                assert!(renamed.is_none(), "negative rename {marker}: {renamed:?}");
            }
        }
    }
    if let Some(config) = spec.oracle["ambiguityRemoval"].as_object() {
        check_ambiguity_removal(db, fixture, &json!(config));
    }
    if let Some(config) = spec.oracle["schemaAmbiguityRemoval"].as_object() {
        check_schema_ambiguity_removal(db, fixture, &json!(config));
    }
}

fn check_schema_ambiguity_removal(
    db: &LanguageServiceDatabases,
    fixture: &FixtureWorkspace,
    config: &Value,
) {
    let new_name = config["newName"].as_str().expect("candidate name");
    for (case, sites) in [
        (
            &config["short"],
            vec![("declaration", "Declaration"), ("qualified", "Read")],
        ),
        (
            &config["import"],
            vec![
                ("declaration", "Declaration"),
                ("terminal", "Import"),
                ("qualified", "Read"),
            ],
        ),
    ] {
        let owner = case["owner"].as_str().expect("schema variant owner");
        let symbol = SymbolRef::Schema(owner.into());
        let expected = sites.iter().map(|(key, kind)| {
            let site = &case[key];
            json!({"uri":uri(site["file"].as_str().expect("file")).as_str(),"range":site_range(fixture,site),"kind":kind})
        }).collect::<Vec<_>>();
        for (key, _) in sites {
            let site = &case[key];
            let file = site["file"].as_str().expect("file");
            let marker = site["marker"].as_str().expect("marker");
            let point = position(fixture, file, marker);
            let actual = db.references(&uri(file), point, true).iter().map(|reference| {
                assert_eq!(reference.symbol(), &symbol, "{marker} owner");
                json!({"uri":reference.document_id().as_str(),"range":range_json(reference.range()),"kind":format!("{:?}",reference.kind())})
            }).collect::<Vec<_>>();
            assert_eq!(
                sorted(actual),
                sorted(expected.clone()),
                "{marker} references"
            );
            assert!(
                db.prepare_rename(&uri(file), point).is_some(),
                "{marker} prepare"
            );
            assert!(
                db.rename(&uri(file), point, new_name).is_none(),
                "ambiguity removal {marker}"
            );
        }
        let site = &case["unresolved"];
        let file = site["file"].as_str().expect("file");
        let point = position(fixture, file, site["marker"].as_str().expect("marker"));
        assert!(db.references(&uri(file), point, true).is_empty());
        assert!(db.prepare_rename(&uri(file), point).is_none());
        assert!(db.rename(&uri(file), point, new_name).is_none());
    }
    for (site, owner, markers) in [
        (
            &config["short"]["other"],
            "beta::Choice::Shared",
            vec![&config["short"]["other"]],
        ),
        (
            &config["import"]["otherTerminal"],
            "delta::Token::Item",
            vec![
                &config["import"]["otherTerminal"],
                &config["import"]["otherQualified"],
            ],
        ),
    ] {
        let file = site["file"].as_str().expect("file");
        let point = position(fixture, file, site["marker"].as_str().expect("marker"));
        let expected = markers.iter().map(|site| {
            json!({"uri":uri(site["file"].as_str().expect("file")).as_str(),"range":site_range(fixture,site)})
        }).collect::<Vec<_>>();
        let actual = db.references(&uri(file), point, true).iter().map(|reference| {
            assert_eq!(reference.symbol(), &SymbolRef::Schema(owner.into()));
            json!({"uri":reference.document_id().as_str(),"range":range_json(reference.range())})
        }).collect();
        assert_eq!(sorted(actual), sorted(expected));
    }
}

fn check_ambiguity_removal(
    db: &LanguageServiceDatabases,
    fixture: &FixtureWorkspace,
    config: &Value,
) {
    let file = config["file"].as_str().expect("ambiguity file");
    let new_name = config["newName"].as_str().expect("candidate name");
    for (side, owner) in [("first", "First"), ("second", "Second")] {
        let markers = config[side].as_array().expect("owner markers");
        let expected_symbol = SymbolRef::Source(format!("ambiguity::{owner}::Clash"));
        let expected = markers.iter().zip(["Declaration", "Import", "Read"]).map(|(marker, kind)| {
            json!({"uri":uri(file).as_str(),"range":site_range(fixture,&json!({"file":file,"marker":marker})),"kind":kind})
        }).collect::<Vec<_>>();
        for marker in markers {
            let marker = marker.as_str().expect("owner marker");
            let point = position(fixture, file, marker);
            let actual = db.references(&uri(file), point, true).iter().map(|reference| {
                assert_eq!(reference.symbol(), &expected_symbol, "{marker} owner");
                json!({"uri":reference.document_id().as_str(),"range":range_json(reference.range()),"kind":format!("{:?}",reference.kind())})
            }).collect();
            assert_eq!(
                sorted(actual),
                sorted(expected.clone()),
                "{marker} references"
            );
            assert_eq!(
                db.prepare_rename(&uri(file), point)
                    .expect("owned variant")
                    .placeholder(),
                "Clash"
            );
            assert!(
                db.rename(&uri(file), point, new_name).is_none(),
                "ambiguity removal {marker}"
            );
        }
    }
    let point = position(
        fixture,
        file,
        config["unresolved"].as_str().expect("unresolved marker"),
    );
    assert!(db.references(&uri(file), point, true).is_empty());
    assert!(db.prepare_rename(&uri(file), point).is_none());
    assert!(db.rename(&uri(file), point, new_name).is_none());
}

fn uri(file: &str) -> DocumentId {
    DocumentId::from(format!("/workspace/中文 % references/{file}"))
}
fn update(db: &mut LanguageServiceDatabases, fixture: &FixtureWorkspace) {
    let files = fixture
        .disk
        .iter()
        .filter(|(file, _)| file.ends_with(".vela"))
        .map(|(file, doc)| SourceFileSnapshot::new(uri(file), doc.text.as_str()))
        .collect::<Vec<_>>();
    db.update(&assemble_project_sources(
        &WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/中文 % references/scripts")]),
        &files,
        &Workspace::new().snapshot(),
    ));
}
fn byte_point(text: &str, point: Point) -> Position {
    Position::new(
        point.line,
        point.byte
            - text[..point.byte]
                .rfind('\n')
                .map_or(0, |offset| offset + 1),
    )
}
fn position(fixture: &FixtureWorkspace, file: &str, marker: &str) -> Position {
    let doc = &fixture.disk[file];
    let mut point = byte_point(&doc.text, doc.markers[marker].start);
    point.character += 1;
    point
}
fn site_range(fixture: &FixtureWorkspace, site: &Value) -> Value {
    let doc = &fixture.disk[site["file"].as_str().expect("fixture string")];
    let range = doc.markers[site["marker"].as_str().expect("fixture string")];
    range_json(DiagnosticRange::new(
        byte_point(&doc.text, range.start),
        byte_point(&doc.text, range.end),
    ))
}
fn range_json(range: DiagnosticRange) -> Value {
    json!([
        range.start().line,
        range.start().character,
        range.end().line,
        range.end().character
    ])
}
fn sorted(mut values: Vec<Value>) -> Vec<Value> {
    values.sort_by_key(Value::to_string);
    values
}
fn byte_offset(text: &str, point: Position) -> usize {
    text.split_inclusive('\n')
        .take(point.line)
        .map(str::len)
        .sum::<usize>()
        + point.character
}

fn load_schema(db: &mut LanguageServiceDatabases, spec: &Spec, fixture: &FixtureWorkspace) {
    if spec.oracle["schema"].is_null() {
        return;
    }
    let artifact =
        crate::matrix_fixture::schema_artifact(&spec.oracle["schema"], fixture, |file| {
            db.source_db().records()[&uri(file)].source_id().get()
        });
    db.load_schema_artifact_json(
        "/workspace/中文 % references/target/schema.json",
        &artifact.to_string(),
    );
}
