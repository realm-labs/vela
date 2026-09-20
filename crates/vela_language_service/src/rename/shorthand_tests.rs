use super::collision_tests::{offset, position, uri};
use crate::matrix_fixture::{Document, rename_shorthand};
use crate::{
    DiagnosticRange, LanguageServiceDatabases, SourceFileSnapshot, Workspace, WorkspaceConfig,
    WorkspaceRoot, assemble_project_sources,
};

#[test]
fn local_shorthand_rename_preserves_field_labels_and_binding_owners() {
    for crlf in [false, true] {
        for (case, before, after) in rename_shorthand::cases(crlf) {
            let id = case["id"].as_str().expect("id");
            check(&before, id);
            check(&after, id);
            let db = database(&before.text);
            for (site, _) in case["edits"].as_object().expect("edits") {
                let marker = before.markers[site];
                let point = position(&before, marker.start);
                let prepared = db
                    .prepare_rename(&uri("scripts/main.vela"), point)
                    .expect(id);
                assert_eq!(prepared.placeholder(), "value", "{id}/{site}");
                assert_eq!(
                    prepared.range(),
                    DiagnosticRange::new(point, position(&before, marker.end))
                );
                let edit = db
                    .rename(
                        &uri("scripts/main.vela"),
                        point,
                        case["newName"].as_str().expect("name"),
                    )
                    .expect(id);
                assert_eq!(edit.document_edits().len(), 1, "{id}/{site}");
                let plan = &edit.document_edits()[0];
                assert_eq!(plan.document_id(), &uri("scripts/main.vela"));
                let mut expected = case["edits"]
                    .as_object()
                    .expect("edits")
                    .iter()
                    .map(|(site, text)| {
                        let marker = before.markers[site];
                        (
                            marker.start.byte,
                            marker.end.byte,
                            text.as_str().expect("text").to_owned(),
                        )
                    })
                    .collect::<Vec<_>>();
                expected.sort();
                let mut actual = plan
                    .edits()
                    .iter()
                    .map(|edit| {
                        (
                            offset(&before.text, edit.range().start()),
                            offset(&before.text, edit.range().end()),
                            edit.new_text().to_owned(),
                        )
                    })
                    .collect::<Vec<_>>();
                actual.sort();
                assert_eq!(actual, expected, "{id}/{site}");
                let mut applied = before.text.clone();
                for (start, end, text) in actual.into_iter().rev() {
                    applied.replace_range(start..end, &text);
                }
                assert_eq!(applied, after.text, "{id}/{site}");
                check(
                    &Document {
                        text: applied,
                        markers: after.markers.clone(),
                    },
                    id,
                );
            }
        }
    }
}

fn check(doc: &Document, id: &str) {
    assert!(
        vela_syntax::parse::parse_source(&doc.text)
            .diagnostics()
            .is_empty(),
        "{id}"
    );
    let db = database(&doc.text);
    let definition = db
        .definition(
            &uri("scripts/main.vela"),
            position(doc, doc.markers["read"].start),
        )
        .unwrap_or_else(|| panic!("{id}: {}", doc.text));
    let target = doc.markers["target"];
    assert_eq!(definition.document_id(), &uri("scripts/main.vela"), "{id}");
    assert_eq!(
        definition.range(),
        DiagnosticRange::new(position(doc, target.start), position(doc, target.end)),
        "{id}"
    );
    for include in [true, false] {
        let references = db.references(
            &uri("scripts/main.vela"),
            position(doc, target.start),
            include,
        );
        assert!(
            references
                .iter()
                .all(|reference| reference.document_id() == &uri("scripts/main.vela")),
            "{id}"
        );
        let mut actual = references
            .iter()
            .map(|reference| reference.range())
            .collect::<Vec<_>>();
        let mut expected = doc
            .markers
            .iter()
            .filter(|(name, _)| include || name.as_str() != "target")
            .map(|(_, marker)| {
                DiagnosticRange::new(position(doc, marker.start), position(doc, marker.end))
            })
            .collect::<Vec<_>>();
        actual.sort_by_key(|r| (r.start().line, r.start().character));
        expected.sort_by_key(|r| (r.start().line, r.start().character));
        assert_eq!(actual, expected, "{id}, include={include}");
    }
}
fn database(text: &str) -> LanguageServiceDatabases {
    let files = [SourceFileSnapshot::new(uri("scripts/main.vela"), text)];
    let project = assemble_project_sources(
        &WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]),
        &files,
        &Workspace::new().snapshot(),
    );
    let mut db = LanguageServiceDatabases::new();
    db.update(&project);
    db
}
