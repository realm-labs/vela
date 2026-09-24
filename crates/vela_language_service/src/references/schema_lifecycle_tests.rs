use serde_json::{Value, json};

use crate::matrix_fixture::{
    FixtureWorkspace, Point, Spec, lifecycle_facts, load, schema_artifact,
};
use crate::{
    DiagnosticRange, DocumentId, LanguageServiceDatabases, Position, RenameRiskKind,
    SourceFileSnapshot, SymbolRef, Workspace, WorkspaceConfig, WorkspaceRoot,
    assemble_project_sources,
};

#[test]
fn schema_reference_rename_lifecycle_retargets_clears_and_restores_owners() {
    for crlf in [false, true] {
        let mut spec = load("reference-rename-schema-lifecycle");
        if crlf {
            for source in spec.files.values_mut() {
                *source = source.replace('\n', "\r\n");
            }
        }
        let fixture = FixtureWorkspace::new(&spec).expect("schema fixture");
        let mut db = databases(&fixture);
        for step in spec.oracle["states"].as_array().expect("states") {
            let facts = lifecycle_facts(&spec.oracle["schema"], step);
            apply_schema(&mut db, &fixture, step, &facts);
            let mut fresh = databases(&fixture);
            apply_schema(&mut fresh, &fixture, step, &facts);
            for database in [&db, &fresh] {
                for _ in 0..2 {
                    check(database, &fixture, &spec, step, crlf);
                }
            }
        }
    }
}

fn databases(fixture: &FixtureWorkspace) -> LanguageServiceDatabases {
    let files = fixture
        .disk
        .iter()
        .filter(|(file, _)| file.ends_with(".vela"))
        .map(|(file, document)| SourceFileSnapshot::new(uri(file), document.text.as_str()))
        .collect::<Vec<_>>();
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let mut db = LanguageServiceDatabases::new();
    db.update(&assemble_project_sources(
        &config,
        &files,
        &Workspace::new().snapshot(),
    ));
    db
}

fn apply_schema(
    db: &mut LanguageServiceDatabases,
    fixture: &FixtureWorkspace,
    step: &Value,
    facts: &Value,
) {
    let path = "/workspace/target/schema.json";
    match step["op"].as_str().expect("operation") {
        "delete" => db.mark_schema_missing(path),
        "invalid" => db.load_schema_artifact_json(path, "{ broken"),
        "restore" | "replace" => {
            let artifact = schema_artifact(facts, fixture, |file| {
                db.source_db().records()[&uri(file)].source_id().get()
            });
            db.load_schema_artifact_json(path, &artifact.to_string());
        }
        _ => panic!("unknown schema operation"),
    }
}

fn check(
    db: &LanguageServiceDatabases,
    fixture: &FixtureWorkspace,
    spec: &Spec,
    step: &Value,
    crlf: bool,
) {
    let context = format!("{} CRLF={crlf}", step["id"].as_str().expect("state"));
    let owner = step["owner"].as_str().expect("owner");
    let main = "scripts/main.vela";
    for group in ["grant", "award"] {
        let definition = &spec.oracle["groups"][group];
        let active = owner == group;
        let marker = format!("{group}-call");
        let point = marker_point(fixture, main, &marker);
        let expected = if active {
            sites(fixture, &definition["sites"])
        } else {
            Vec::new()
        };
        let references = db.references(&uri(main),point,true).iter().map(|reference| {
            assert_eq!(reference.symbol(),&SymbolRef::Schema(definition["qualified"].as_str().expect("symbol").into()),"{context}");
            json!({"uri":reference.document_id().as_str(),"range":range(reference.range()),"kind":format!("{:?}",reference.kind())})
        }).collect::<Vec<_>>();
        assert_eq!(
            sorted(references),
            sorted(expected.clone()),
            "{group} references {context}"
        );
        let without = db.references(&uri(main),point,false).iter().map(|reference|
            json!({"uri":reference.document_id().as_str(),"range":range(reference.range()),"kind":format!("{:?}",reference.kind())})
        ).collect::<Vec<_>>();
        assert_eq!(
            sorted(without),
            sorted(
                expected
                    .into_iter()
                    .filter(|site| site["kind"] != "Declaration")
                    .collect()
            ),
            "{group} without declaration {context}"
        );
        let highlights = db.document_highlights(&uri(main),point).iter().map(|highlight|
            json!({"range":range(highlight.range()),"kind":format!("{:?}",highlight.kind())})
        ).collect::<Vec<_>>();
        let expected_highlights = if active {
            vec![json!({"range":marker_range(fixture,main,&marker),"kind":"Call"})]
        } else {
            Vec::new()
        };
        assert_eq!(
            sorted(highlights),
            sorted(expected_highlights),
            "{group} highlights {context}"
        );
        let target = db.definition(&uri(main), point);
        let prepare = db.prepare_rename(&uri(main), point);
        let rename = db.rename(
            &uri(main),
            point,
            spec.oracle["replacement"].as_str().expect("replacement"),
        );
        if active {
            let target = target.expect("source-backed schema target");
            assert_eq!(target.document_id(), &uri("scripts/helpers.vela"));
            assert_eq!(
                range(target.range()),
                marker_range(fixture, "scripts/helpers.vela", &format!("{group}-decl"))
            );
            let prepare = prepare.expect("source-backed schema prepare");
            assert_eq!(
                prepare.symbol(),
                &SymbolRef::Schema(definition["qualified"].as_str().expect("symbol").into())
            );
            assert_eq!(
                prepare.placeholder(),
                definition["name"].as_str().expect("name")
            );
            assert_eq!(range(prepare.range()), marker_range(fixture, main, &marker));
            let rename = rename.expect("source-backed schema rename");
            assert!(
                rename
                    .risks()
                    .iter()
                    .any(|risk| risk.kind() == RenameRiskKind::SchemaAbi)
            );
            let actual = rename.document_edits().iter().flat_map(|document|document.edits().iter().map(|edit|
                json!({"uri":document.document_id().as_str(),"range":range(edit.range()),"newText":edit.new_text()})
            )).collect::<Vec<_>>();
            let expected = definition["sites"].as_array().expect("sites").iter().map(|site| {
                let file = site["file"].as_str().expect("file");
                json!({"uri":uri(file).as_str(),"range":marker_range(fixture,file,site["marker"].as_str().expect("marker")),"newText":spec.oracle["replacement"]})
            }).collect::<Vec<_>>();
            assert_eq!(sorted(actual), sorted(expected), "{group} edits {context}");
        } else {
            assert!(target.is_none(), "inactive definition {context}");
            assert!(prepare.is_none(), "inactive prepare {context}");
            assert!(rename.is_none(), "inactive rename {context}");
        }
    }
    let point = marker_point(fixture, main, "ping-call");
    let metadata = step["metadata"].as_bool().expect("metadata state");
    let references = db.references(&uri(main),point,true).iter().map(|reference| {
        assert_eq!(reference.symbol(),&SymbolRef::Schema("meta::ping".into()));
        json!({"uri":reference.document_id().as_str(),"range":range(reference.range()),"kind":format!("{:?}",reference.kind())})
    }).collect::<Vec<_>>();
    let expected = if metadata {
        sites(fixture, &spec.oracle["groups"]["metadata"]["sites"])
    } else {
        Vec::new()
    };
    assert_eq!(
        sorted(references),
        sorted(expected),
        "metadata references {context}"
    );
    assert_eq!(
        db.document_highlights(&uri(main), point).len(),
        usize::from(metadata)
    );
    assert!(db.definition(&uri(main), point).is_none());
    assert!(db.prepare_rename(&uri(main), point).is_none());
    assert!(db.rename(&uri(main), point, "new_ping").is_none());
    let unknown = marker_point(fixture, main, "unknown");
    assert!(db.references(&uri(main), unknown, true).is_empty());
    assert!(db.prepare_rename(&uri(main), unknown).is_none());
    assert!(db.rename(&uri(main), unknown, "new_grant").is_none());
    let dynamic = marker_point(fixture, main, "dynamic");
    assert!(db.references(&uri(main), dynamic, true).is_empty());
    assert!(db.document_highlights(&uri(main), dynamic).is_empty());
    assert!(db.prepare_rename(&uri(main), dynamic).is_none());
    assert!(db.rename(&uri(main), dynamic, "new_grant").is_none());
}

fn sites(fixture: &FixtureWorkspace, sites: &Value) -> Vec<Value> {
    sites.as_array().expect("sites").iter().map(|site| {
        let file = site["file"].as_str().expect("file");
        let marker = site["marker"].as_str().expect("marker");
        json!({"uri":uri(file).as_str(),"range":marker_range(fixture,file,marker),"kind":site["kind"]})
    }).collect()
}

fn marker_point(fixture: &FixtureWorkspace, file: &str, marker: &str) -> Position {
    let document = fixture.document(file).expect("document");
    let point = document.markers[marker].start;
    byte_point(&document.text, point)
}

fn marker_range(fixture: &FixtureWorkspace, file: &str, marker: &str) -> Value {
    let document = fixture.document(file).expect("document");
    let marker = document.markers[marker];
    positions(
        byte_point(&document.text, marker.start),
        byte_point(&document.text, marker.end),
    )
}

fn byte_point(text: &str, point: Point) -> Position {
    Position::new(
        point.line,
        point.byte - text[..point.byte].rfind('\n').map_or(0, |index| index + 1),
    )
}

fn range(range: DiagnosticRange) -> Value {
    positions(range.start(), range.end())
}
fn positions(start: Position, end: Position) -> Value {
    json!([start.line, start.character, end.line, end.character])
}
fn uri(file: &str) -> DocumentId {
    DocumentId::from(format!("/workspace/{file}"))
}
fn sorted(mut values: Vec<Value>) -> Vec<Value> {
    values.sort_by_key(Value::to_string);
    values
}
