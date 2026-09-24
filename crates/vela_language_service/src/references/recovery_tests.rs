use serde_json::{Value, json};

use crate::matrix_fixture::{FixtureWorkspace, Point, Spec, load};
use crate::{
    DiagnosticRange, DocumentId, LanguageServiceDatabases, Position, SourceFileSnapshot, SymbolRef,
    Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn reference_rename_recovery_keeps_valid_neighbors_and_drops_malformed_names() {
    for crlf in [false, true] {
        let mut spec = load("reference-rename-recovery");
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
        let mut fixture = FixtureWorkspace::new(&spec).expect("recovery fixture");
        let mut db = LanguageServiceDatabases::new();
        for (index, action) in spec.actions.iter().enumerate() {
            fixture.apply(action).expect("recovery action");
            update(&mut db, &fixture);
            let mut fresh = LanguageServiceDatabases::new();
            update(&mut fresh, &fixture);
            let state = &spec.oracle["states"][index];
            for database in [&db, &fresh] {
                for _ in 0..2 {
                    check(database, &fixture, &spec, state, crlf);
                }
            }
        }
    }
}

fn update(db: &mut LanguageServiceDatabases, fixture: &FixtureWorkspace) {
    let mut effective = fixture.disk.clone();
    effective.extend(fixture.open.clone());
    let files = effective
        .iter()
        .filter(|(file, _)| file.ends_with(".vela"))
        .map(|(file, source)| SourceFileSnapshot::new(uri(file), source.text.as_str()))
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
    state: &Value,
    crlf: bool,
) {
    let main = "scripts/main.vela";
    let context = format!("{} CRLF={crlf}", state["id"].as_str().expect("state"));
    let source = fixture.document(main).expect("main");
    let malformed = state["syntaxError"].as_bool().expect("syntax state");
    assert_eq!(
        !vela_syntax::parse::parse_source(&source.text)
            .diagnostics()
            .is_empty(),
        malformed,
        "parser {context}"
    );
    let diagnostics = db.diagnostics_for_document(&uri(main));
    assert_eq!(
        diagnostics
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == Some("E_PARSE")),
        malformed,
        "service diagnostics {context}: {diagnostics:?}"
    );
    let point = marker_point(fixture, main, "call");
    let actual = db.references(&uri(main), point, true).iter().map(|reference| {
        assert_eq!(reference.symbol(), &SymbolRef::Source("helper::increment".into()), "{context}");
        json!({"uri":reference.document_id().as_str(),"range":range(reference.range()),"kind":format!("{:?}",reference.kind())})
    }).collect::<Vec<_>>();
    assert_eq!(
        sorted(actual),
        sorted(sites(fixture, &spec.oracle["sites"])),
        "references {context}"
    );
    let without = db.references(&uri(main), point, false).iter().map(|reference|
        json!({"uri":reference.document_id().as_str(),"range":range(reference.range()),"kind":format!("{:?}",reference.kind())})
    ).collect::<Vec<_>>();
    let expected = sites(fixture, &spec.oracle["sites"])
        .into_iter()
        .filter(|site| site["kind"] != "Declaration")
        .collect::<Vec<_>>();
    assert_eq!(
        sorted(without),
        sorted(expected),
        "without declaration {context}"
    );
    let highlights = db.document_highlights(&uri(main),point).iter().map(|highlight|
        json!({"range":range(highlight.range()),"kind":format!("{:?}",highlight.kind())})
    ).collect::<Vec<_>>();
    assert_eq!(
        sorted(highlights),
        sorted(vec![
            json!({"range":marker_range(fixture,main,"import"),"kind":"Text"}),
            json!({"range":marker_range(fixture,main,"call"),"kind":"Call"})
        ]),
        "highlights {context}"
    );
    let prepare = db.prepare_rename(&uri(main), point).expect("valid prepare");
    assert_eq!(
        prepare.symbol(),
        &SymbolRef::Source("helper::increment".into())
    );
    assert_eq!(prepare.placeholder(), "increment");
    assert_eq!(range(prepare.range()), marker_range(fixture, main, "call"));
    let rename = db
        .rename(&uri(main), point, "advance")
        .expect("valid rename");
    let actual = rename.document_edits().iter().flat_map(|document|document.edits().iter().map(|edit|
        json!({"uri":document.document_id().as_str(),"range":range(edit.range()),"newText":edit.new_text()})
    )).collect::<Vec<_>>();
    let expected = spec.oracle["sites"].as_array().expect("sites").iter().map(|site| {
        let file=site["file"].as_str().expect("file");
        json!({"uri":uri(file).as_str(),"range":marker_range(fixture,file,site["marker"].as_str().expect("marker")),"newText":"advance"})
    }).collect::<Vec<_>>();
    assert_eq!(sorted(actual), sorted(expected), "rename {context}");
    let decoy = "scripts/decoy.vela";
    let point = marker_point(fixture, decoy, "decoy-call");
    let actual = db.references(&uri(decoy),point,true).iter().map(|reference|{
        assert_eq!(reference.symbol(),&SymbolRef::Source("decoy::increment".into()));
        json!({"uri":reference.document_id().as_str(),"range":range(reference.range()),"kind":format!("{:?}",reference.kind())})
    }).collect::<Vec<_>>();
    assert_eq!(
        sorted(actual),
        sorted(sites(fixture, &spec.oracle["decoy"])),
        "decoy {context}"
    );
    if state["bad"] == true {
        let bad = marker_point(fixture, main, "bad");
        assert!(
            db.references(&uri(main), bad, true).is_empty(),
            "bad refs {context}"
        );
        assert!(
            db.document_highlights(&uri(main), bad).is_empty(),
            "bad highlights {context}"
        );
        assert!(
            db.prepare_rename(&uri(main), bad).is_none(),
            "bad prepare {context}"
        );
        assert!(
            db.rename(&uri(main), bad, "advance").is_none(),
            "bad rename {context}"
        );
    }
}

fn sites(fixture: &FixtureWorkspace, sites: &Value) -> Vec<Value> {
    sites.as_array().expect("sites").iter().map(|site|{
        let file=site["file"].as_str().expect("file");
        json!({"uri":uri(file).as_str(),"range":marker_range(fixture,file,site["marker"].as_str().expect("marker")),"kind":site["kind"]})
    }).collect()
}
fn marker_point(fixture: &FixtureWorkspace, file: &str, name: &str) -> Position {
    let source = fixture.document(file).expect("document");
    position(&source.text, source.markers[name].start)
}
fn marker_range(fixture: &FixtureWorkspace, file: &str, name: &str) -> Value {
    let source = fixture.document(file).expect("document");
    let marker = source.markers[name];
    positions(
        position(&source.text, marker.start),
        position(&source.text, marker.end),
    )
}
fn position(text: &str, point: Point) -> Position {
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
