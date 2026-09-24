use serde_json::{Value, json};

use crate::matrix_fixture::{FixtureWorkspace, Point, Spec, load};
use crate::{
    DocumentId, LanguageServiceDatabases, Position, SourceFileSnapshot, SymbolRef, Workspace,
    WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn reference_rename_lifecycle_tracks_overlay_disk_close_and_dependency_states() {
    for crlf in [false, true] {
        let mut spec = load("reference-rename-lifecycle");
        if crlf {
            for source in spec.files.values_mut() {
                *source = source.replace('\n', "\r\n");
            }
            for action in &mut spec.actions {
                if let Some(source) = &mut action.source {
                    *source = source.replace('\n', "\r\n");
                }
            }
        }
        let mut fixture = FixtureWorkspace::new(&spec).expect("lifecycle fixture");
        let mut db = LanguageServiceDatabases::new();
        for (index, action) in spec.actions.iter().enumerate() {
            fixture.apply(action).expect("lifecycle action");
            update(&mut db, &fixture);
            let owner = spec.oracle["ownerAfterEachAction"][index]
                .as_str()
                .expect("explicit owner state");
            check(&db, &fixture, &spec, owner, index, crlf);
            let mut fresh = LanguageServiceDatabases::new();
            update(&mut fresh, &fixture);
            check(&fresh, &fixture, &spec, owner, index, crlf);
        }
    }
}

fn update(db: &mut LanguageServiceDatabases, fixture: &FixtureWorkspace) {
    let mut effective = fixture.disk.clone();
    effective.extend(fixture.open.clone());
    let files = effective
        .iter()
        .filter(|(file, _)| file.ends_with(".vela"))
        .map(|(file, document)| SourceFileSnapshot::new(uri(file), document.text.as_str()))
        .collect::<Vec<_>>();
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    db.update(&assemble_project_sources(
        &config,
        &files,
        &Workspace::new().snapshot(),
    ));
}

fn check(
    db: &LanguageServiceDatabases,
    fixture: &FixtureWorkspace,
    spec: &Spec,
    owner: &str,
    index: usize,
    crlf: bool,
) {
    let main = "scripts/main.vela";
    let query = marker(fixture, main, "call");
    let point = byte_position(&fixture.document(main).expect("main").text, query.start);
    let context = format!("state {index}, CRLF={crlf}");
    let (sites, symbol) = match owner {
        "helper" => (&spec.oracle["sites"], "helper::increment"),
        "other" => (&spec.oracle["rebound"], "other::increment"),
        "unresolved" => (&spec.oracle["sites"], ""),
        _ => panic!("unknown owner state"),
    };
    let expected = if owner == "unresolved" {
        Vec::new()
    } else {
        expected_sites(fixture, sites)
    };
    let actual = db.references(&uri(main), point, true).iter().map(|reference| {
        assert_eq!(reference.symbol(), &SymbolRef::Source(symbol.into()), "{context}");
        json!({"uri":reference.document_id().as_str(),"range":range(reference.range()),"kind":format!("{:?}",reference.kind())})
    }).collect::<Vec<_>>();
    assert_eq!(
        sorted(actual),
        sorted(expected.clone()),
        "references {context}"
    );
    let without_declaration = db.references(&uri(main), point, false).iter().map(|reference| {
        json!({"uri":reference.document_id().as_str(),"range":range(reference.range()),"kind":format!("{:?}",reference.kind())})
    }).collect::<Vec<_>>();
    assert_eq!(
        sorted(without_declaration),
        sorted(
            expected
                .into_iter()
                .filter(|site| site["kind"] != "Declaration")
                .collect()
        ),
        "references without declaration {context}"
    );
    let actual_highlights = db.document_highlights(&uri(main), point).iter().map(|highlight| {
        json!({"range":range(highlight.range()),"kind":format!("{:?}",highlight.kind())})
    }).collect::<Vec<_>>();
    let expected_highlights = if owner != "unresolved" {
        [
            json!({"range":marker_range(fixture, main, "import"),"kind":"Text"}),
            json!({"range":marker_range(fixture, main, "call"),"kind":"Call"}),
        ]
        .to_vec()
    } else {
        Vec::new()
    };
    assert_eq!(
        sorted(actual_highlights),
        sorted(expected_highlights),
        "highlights {context}"
    );
    let prepared = db.prepare_rename(&uri(main), point);
    let edit = db.rename(
        &uri(main),
        point,
        spec.oracle["replacement"].as_str().expect("replacement"),
    );
    if owner != "unresolved" {
        let prepared = prepared.expect("owned prepare");
        assert_eq!(prepared.symbol(), &SymbolRef::Source(symbol.into()));
        assert_eq!(prepared.placeholder(), "increment");
        assert_eq!(
            range(prepared.range()),
            marker_range(fixture, main, "call"),
            "{context}"
        );
        let edit = edit.expect("owned rename");
        let actual = edit.document_edits().iter().flat_map(|document| document.edits().iter().map(|edit| {
            json!({"uri":document.document_id().as_str(),"range":range(edit.range()),"newText":edit.new_text()})
        })).collect::<Vec<_>>();
        let expected = sites.as_array().expect("owned sites").iter().map(|site| {
            let file = site["file"].as_str().expect("file");
            let name = site["marker"].as_str().expect("marker");
            json!({"uri":uri(file).as_str(),"range":marker_range(fixture,file,name),"newText":spec.oracle["replacement"]})
        }).collect::<Vec<_>>();
        assert_eq!(sorted(actual), sorted(expected), "rename edits {context}");
    } else {
        assert!(prepared.is_none(), "unresolved prepare {context}");
        assert!(edit.is_none(), "unresolved rename {context}");
    }
    let other = "scripts/other.vela";
    let point = byte_position(
        &fixture.document(other).expect("other").text,
        marker(fixture, other, "other-decl").start,
    );
    let actual = db.references(&uri(other), point, true).iter().map(|reference| {
        assert_eq!(reference.symbol(), &SymbolRef::Source("other::increment".into()), "{context}");
        json!({"uri":reference.document_id().as_str(),"range":range(reference.range()),"kind":format!("{:?}",reference.kind())})
    }).collect::<Vec<_>>();
    let other_sites = if owner == "other" {
        &spec.oracle["rebound"]
    } else {
        &spec.oracle["independent"]
    };
    assert_eq!(
        sorted(actual),
        sorted(expected_sites(fixture, other_sites)),
        "other owner {context}"
    );
    if owner == "other" {
        let helper = "scripts/helper.vela";
        let point = byte_position(
            &fixture.document(helper).expect("helper").text,
            marker(fixture, helper, "definition").start,
        );
        let actual = db.references(&uri(helper),point,true).iter().map(|reference| {
            assert_eq!(reference.symbol(), &SymbolRef::Source("helper::increment".into()));
            json!({"uri":reference.document_id().as_str(),"range":range(reference.range()),"kind":format!("{:?}",reference.kind())})
        }).collect::<Vec<_>>();
        assert_eq!(
            sorted(actual),
            sorted(expected_sites(fixture, &spec.oracle["helperDuringRebind"])),
            "helper during rebind {context}"
        );
    }
}

fn expected_sites(fixture: &FixtureWorkspace, sites: &Value) -> Vec<Value> {
    sites.as_array().expect("sites").iter().map(|site| {
        let file = site["file"].as_str().expect("file");
        let name = site["marker"].as_str().expect("marker");
        json!({"uri":uri(file).as_str(),"range":marker_range(fixture,file,name),"kind":site["kind"]})
    }).collect()
}

fn marker(fixture: &FixtureWorkspace, file: &str, name: &str) -> crate::matrix_fixture::Marker {
    fixture.document(file).expect("document").markers[name]
}

fn marker_range(fixture: &FixtureWorkspace, file: &str, name: &str) -> Value {
    let document = fixture.document(file).expect("document");
    let marker = marker(fixture, file, name);
    positions(
        byte_position(&document.text, marker.start),
        byte_position(&document.text, marker.end),
    )
}

fn range(range: crate::DiagnosticRange) -> Value {
    positions(range.start(), range.end())
}

fn positions(start: Position, end: Position) -> Value {
    json!([start.line, start.character, end.line, end.character])
}

fn byte_position(text: &str, point: Point) -> Position {
    Position::new(
        point.line,
        point.byte - text[..point.byte].rfind('\n').map_or(0, |index| index + 1),
    )
}

fn uri(file: &str) -> DocumentId {
    DocumentId::from(format!("/workspace/{file}"))
}

fn sorted(mut values: Vec<Value>) -> Vec<Value> {
    values.sort_by_key(Value::to_string);
    values
}
