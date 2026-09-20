use super::collision_tests::{databases, offset, position, uri};
use crate::matrix_fixture::{FixtureWorkspace, Spec, references, rename_collisions as oracle};
use crate::{DiagnosticRange, LanguageServiceDatabases};

#[test]
fn local_rename_collision_matrix_preserves_all_binding_owners() {
    for crlf in [false, true] {
        for spec in oracle::local_cases(crlf) {
            let id = spec.oracle["id"].as_str().expect("id");
            let fixture = FixtureWorkspace::new(&spec).expect(id);
            references::assert_parsed(&fixture);
            let db = databases(&fixture);
            check_owners(&db, &fixture, &spec);
            let doc = &fixture.disk["scripts/main.vela"];
            for (name, marker) in doc
                .markers
                .iter()
                .filter(|(name, _)| name.as_str() == "target" || name.starts_with("use"))
            {
                let point = position(doc, marker.start);
                let prepare = db
                    .prepare_rename(&uri("scripts/main.vela"), point)
                    .expect(id);
                assert_eq!(prepare.placeholder(), "value", "{id}/{name}");
                assert_eq!(
                    prepare.range(),
                    DiagnosticRange::new(point, position(doc, marker.end)),
                    "{id}/{name}"
                );
                let edit = db.rename(
                    &uri("scripts/main.vela"),
                    point,
                    spec.oracle["newName"].as_str().expect("new name"),
                );
                assert_eq!(
                    edit.is_some(),
                    spec.oracle["allowed"].as_bool().expect("allowed"),
                    "{id}/{name}: {edit:?}"
                );
                let Some(edit) = edit else {
                    continue;
                };
                assert_eq!(edit.document_edits().len(), 1, "{id}");
                let plan = &edit.document_edits()[0];
                assert_eq!(plan.document_id(), &uri("scripts/main.vela"));
                let mut actual = doc.text.clone();
                let mut edits = plan.edits().iter().collect::<Vec<_>>();
                edits.sort_by_key(|edit| {
                    (edit.range().start().line, edit.range().start().character)
                });
                for edit in edits.into_iter().rev() {
                    let start = offset(&actual, edit.range().start());
                    let end = offset(&actual, edit.range().end());
                    actual.replace_range(start..end, edit.new_text());
                }
                let expected =
                    FixtureWorkspace::new(&oracle::renamed_local(&spec)).expect("renamed");
                assert_eq!(
                    actual, expected.disk["scripts/main.vela"].text,
                    "{id}/{name}"
                );
                references::assert_parsed(&expected);
                check_owners(&databases(&expected), &expected, &spec);
            }
        }
    }
}

fn check_owners(db: &LanguageServiceDatabases, fixture: &FixtureWorkspace, spec: &Spec) {
    let id = spec.oracle["id"].as_str().expect("id");
    let file = "scripts/main.vela";
    let doc = &fixture.disk[file];
    for (site, owner) in spec.oracle["probes"].as_object().expect("probes") {
        let definition = db.definition(&uri(file), position(doc, doc.markers[site].start));
        let Some(owner) = owner.as_str() else {
            assert!(definition.is_none(), "{id}/{site}");
            continue;
        };
        let target_file = if owner == "global" {
            "scripts/helpers.vela"
        } else {
            file
        };
        let target = &fixture.disk[target_file];
        let marker = target.markers[owner];
        let definition = definition.expect(id);
        assert_eq!(definition.document_id(), &uri(target_file), "{id}/{site}");
        assert_eq!(
            definition.range(),
            DiagnosticRange::new(position(target, marker.start), position(target, marker.end)),
            "{id}/{site}"
        );
    }
    let mut expected = doc
        .markers
        .iter()
        .filter(|(name, _)| name.as_str() == "target" || name.starts_with("use"))
        .map(|(_, marker)| {
            DiagnosticRange::new(position(doc, marker.start), position(doc, marker.end))
        })
        .collect::<Vec<_>>();
    expected.sort_by_key(|r| (r.start().line, r.start().character));
    for include in [true, false] {
        let actual = db.references(
            &uri(file),
            position(doc, doc.markers["target"].start),
            include,
        );
        assert!(actual.iter().all(|r| r.document_id() == &uri(file)), "{id}");
        let mut actual = actual.iter().map(|r| r.range()).collect::<Vec<_>>();
        actual.sort_by_key(|r| (r.start().line, r.start().character));
        let declaration = position(doc, doc.markers["target"].start);
        let expected = expected
            .iter()
            .copied()
            .filter(|r| include || r.start() != declaration)
            .collect::<Vec<_>>();
        assert_eq!(actual, expected, "{id}, include={include}");
    }
}
