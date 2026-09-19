use super::*;
use crate::matrix_fixture::{Document, FixtureWorkspace, Point, references, rename_collisions};
use crate::{
    SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn declaration_rename_collision_matrix_preserves_call_ownership() {
    for crlf in [false, true] {
        for (id, allowed, spec) in rename_collisions::cases(crlf) {
            let fixture = FixtureWorkspace::new(&spec).expect("fixture");
            references::assert_parsed(&fixture);
            let db = databases(&fixture);
            assert_owner(&db, &fixture, &id);
            for (file, doc) in &fixture.disk {
                for (marker, site) in &doc.markers {
                    if marker == "alias" {
                        continue;
                    }
                    let position = position(doc, site.start);
                    assert!(
                        db.prepare_rename(&uri(file), position).is_some(),
                        "{id}/{marker}"
                    );
                    let edit = db.rename(&uri(file), position, "award");
                    assert_eq!(edit.is_some(), allowed, "{id}/{marker}: {edit:?}");
                    let Some(edit) = edit else {
                        continue;
                    };
                    let expected =
                        FixtureWorkspace::new(&rename_collisions::renamed(&spec)).expect("renamed");
                    let mut actual = fixture.clone();
                    for document in edit.document_edits() {
                        let (_, text) = actual
                            .disk
                            .iter_mut()
                            .find(|(file, _)| uri(file) == *document.document_id())
                            .expect("owned document");
                        let mut edits = document.edits().iter().collect::<Vec<_>>();
                        edits.sort_by_key(|edit| {
                            (edit.range().start().line, edit.range().start().character)
                        });
                        for edit in edits.into_iter().rev() {
                            let start = offset(&text.text, edit.range().start());
                            let end = offset(&text.text, edit.range().end());
                            text.text.replace_range(start..end, edit.new_text());
                        }
                    }
                    for (file, doc) in &actual.disk {
                        assert_eq!(doc.text, expected.disk[file].text, "{id}/{file}");
                    }
                    references::assert_parsed(&expected);
                    assert_owner(&databases(&expected), &expected, &id);
                }
            }
        }
    }
}

fn assert_owner(db: &LanguageServiceDatabases, fixture: &FixtureWorkspace, id: &str) {
    let source = &fixture.disk["scripts/main.vela"];
    let marker = source
        .markers
        .get("use")
        .or_else(|| source.markers.get("alias"))
        .expect("use");
    let definition = db
        .definition(&uri("scripts/main.vela"), position(source, marker.start))
        .expect("owned call");
    let target = &fixture.disk["scripts/helpers.vela"];
    let declaration = target.markers["declaration"];
    assert_eq!(
        definition.document_id(),
        &uri("scripts/helpers.vela"),
        "{id}"
    );
    assert_eq!(
        definition.range(),
        DiagnosticRange::new(
            position(target, declaration.start),
            position(target, declaration.end)
        ),
        "{id}"
    );
}

fn uri(file: &str) -> DocumentId {
    DocumentId::from(format!("/workspace/{file}"))
}
fn position(doc: &Document, point: Point) -> Position {
    let line_start = doc.text[..point.byte]
        .rfind('\n')
        .map_or(0, |offset| offset + 1);
    Position::new(point.line, point.byte - line_start)
}
fn offset(text: &str, position: Position) -> usize {
    text.split_inclusive('\n')
        .take(position.line)
        .map(str::len)
        .sum::<usize>()
        + position.character
}
fn databases(fixture: &FixtureWorkspace) -> LanguageServiceDatabases {
    let files = fixture
        .disk
        .iter()
        .filter(|(file, _)| file.ends_with(".vela"))
        .map(|(file, doc)| SourceFileSnapshot::new(uri(file), doc.text.clone()))
        .collect::<Vec<_>>();
    let project = assemble_project_sources(
        &WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]),
        &files,
        &Workspace::new().snapshot(),
    );
    let mut db = LanguageServiceDatabases::new();
    db.update(&project);
    db
}
