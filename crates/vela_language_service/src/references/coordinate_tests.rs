use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::matrix_fixture::{FixtureWorkspace, Point, Spec, references as oracle};
use crate::{
    DiagnosticRange, DocumentId, LanguageServiceDatabases, Position, SourceFileSnapshot, SymbolRef,
    TextRange, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn reference_rename_coordinate_matrix_preserves_sets_owners_and_applied_edits() {
    for crlf in [false, true] {
        let spec = oracle::spec(crlf);
        let fixture = FixtureWorkspace::new(&spec).expect("fixture");
        oracle::assert_parsed(&fixture);
        let mut db = LanguageServiceDatabases::new();
        update(&mut db, &fixture);
        check_queries(&db, &spec, &fixture);
        for (group, definition) in spec.oracle["groups"].as_object().expect("groups") {
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
            check_queries(&db, &expected, &applied);
            update(&mut db, &fixture);
            check_queries(&db, &spec, &fixture);
        }
    }
}

fn check_queries(db: &LanguageServiceDatabases, spec: &Spec, fixture: &FixtureWorkspace) {
    for query in spec.oracle["queries"].as_array().expect("queries") {
        let file = query["file"].as_str().expect("query file");
        let marker = query["marker"].as_str().expect("query marker");
        let point = position(fixture, file, marker);
        let sites = oracle::sites(spec, query);
        let symbol = query["group"].as_str().map(|group| {
            let definition = &spec.oracle["groups"][group];
            if definition["origin"] == "source" {
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
                let prepared = prepared.expect("prepare rename");
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
                let actual = renamed.document_edits().iter().flat_map(|document| document.edits().iter().map(|edit|
                    json!({"uri":document.document_id().as_str(),"range":range_json(edit.range()),"newText":edit.new_text()}))).collect();
                let expected = sites.iter().map(|site| json!({"uri":uri(site["file"].as_str().expect("fixture string")).as_str(),"range":site_range(fixture,site),"newText":"renamed_symbol"})).collect();
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
