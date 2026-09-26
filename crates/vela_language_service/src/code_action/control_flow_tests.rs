use vela_analysis::{registry::RegistryFacts, type_fact::TypeFact};

use crate::matrix_fixture::{Document, FixtureWorkspace, load, parse_markers};
use crate::{
    DiagnosticRange, DocumentId, LanguageServiceDatabases, Position, ServiceDiagnosticSeverity,
    SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

fn document_id(name: &str) -> DocumentId {
    DocumentId::from(format!("/workspace/{name}"))
}

fn update(db: &mut LanguageServiceDatabases, fixture: &FixtureWorkspace) {
    let files = fixture
        .disk
        .iter()
        .map(|(name, file)| SourceFileSnapshot::new(document_id(name), file.text.as_str()))
        .collect::<Vec<_>>();
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    db.update(&assemble_project_sources(
        &config,
        &files,
        &Workspace::new().snapshot(),
    ));
}

fn range(document: &Document, name: &str) -> DiagnosticRange {
    let marker = document.markers[name];
    let point = |line: usize, byte: usize| {
        let start = document.text[..byte]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        Position::new(line, byte - start)
    };
    DiagnosticRange::new(
        point(marker.start.line, marker.start.byte),
        point(marker.end.line, marker.end.byte),
    )
}

fn whole(document: &Document) -> DiagnosticRange {
    DiagnosticRange::new(
        Position::new(0, 0),
        Position::new(document.text.lines().count(), 0),
    )
}

#[test]
fn control_flow_actions_apply_all_missing_pattern_shapes_without_guessing() {
    let spec = load("diagnostic-action-control-flow");
    let expected_names = ["Option", "Result", "Option"];
    let expected_missing = ["None", "Ok", "Some"];
    for crlf in [false, true] {
        let mut fixture = FixtureWorkspace::new(&spec).expect("fixture");
        if crlf {
            for (name, document) in &mut fixture.disk {
                *document =
                    parse_markers(&spec.files[name].replace('\n', "\r\n")).expect("CRLF fixture");
            }
        }
        let valid_id = document_id("scripts/valid.vela");
        let invalid_id = document_id("scripts/invalid.vela");
        let inline_id = document_id("scripts/inline.vela");
        let incomplete_id = document_id("scripts/incomplete.vela");
        let original = fixture.disk["scripts/invalid.vela"].clone();
        let mut db = LanguageServiceDatabases::new();
        update(&mut db, &fixture);
        let valid = &fixture.disk["scripts/valid.vela"];
        assert_eq!(db.diagnostics_for_document(&valid_id).diagnostics(), []);
        assert_eq!(db.code_actions(&valid_id, whole(valid)), []);
        for (id, name) in [(&inline_id, "inline"), (&incomplete_id, "incomplete")] {
            let document = &fixture.disk[&format!("scripts/{name}.vela")];
            let diagnostics = db.diagnostics_for_document(id);
            assert!(
                diagnostics
                    .diagnostics()
                    .iter()
                    .any(|diagnostic| diagnostic.code() == Some("analysis::non_exhaustive_match")),
                "warning remains visible for {name}"
            );
            assert_eq!(
                db.code_actions(id, range(document, name)),
                [],
                "inline or incomplete match has no safe insertion, CRLF={crlf}"
            );
        }

        let initial = db.diagnostics_for_document(&invalid_id);
        assert_eq!(initial.diagnostics().len(), 3, "CRLF={crlf}");
        for (index, item) in spec.oracle["positive"]
            .as_array()
            .expect("positive actions")
            .iter()
            .enumerate()
        {
            let name = item["marker"].as_str().expect("marker");
            let diagnostic = &initial.diagnostics()[index];
            assert_eq!(diagnostic.code(), Some("analysis::non_exhaustive_match"));
            assert_eq!(diagnostic.severity(), ServiceDiagnosticSeverity::Warning);
            assert_eq!(
                diagnostic.message(),
                format!(
                    "match on `{}` does not cover all known variants",
                    expected_names[index]
                )
            );
            assert_eq!(diagnostic.range(), Some(range(&original, name)));
            assert_eq!(diagnostic.labels().len(), 1);
            assert_eq!(
                diagnostic.labels()[0].message(),
                format!("missing variants: {}", expected_missing[index])
            );
            assert!(diagnostic.candidates().is_empty());
            assert!(diagnostic.repair_hints().is_empty());
        }
        for name in spec.oracle["negative"]
            .as_array()
            .expect("negative markers")
        {
            let name = name.as_str().expect("marker");
            assert_eq!(
                db.code_actions(&invalid_id, range(&original, name)),
                [],
                "no action at {name}, CRLF={crlf}"
            );
        }

        let mut edits = Vec::new();
        for (index, item) in spec.oracle["positive"]
            .as_array()
            .expect("positive actions")
            .iter()
            .enumerate()
        {
            let name = item["marker"].as_str().expect("marker");
            let insertion = item["insert"].as_str().expect("insertion marker");
            let actions = db.code_actions(&invalid_id, range(&original, name));
            let [action] = actions.as_slice() else {
                panic!("one action at {name}, CRLF={crlf}: {actions:?}");
            };
            assert_eq!(action.title(), item["title"].as_str().expect("title"));
            assert_eq!(action.kind().as_lsp_kind(), "quickfix");
            let [document_edit] = action.edit().document_edits() else {
                panic!("one document edit");
            };
            assert_eq!(document_edit.document_id(), &invalid_id);
            let [edit] = document_edit.edits() else {
                panic!("one text edit");
            };
            assert_eq!(edit.range(), range(&original, insertion));
            let expected_edit = item["replacement"].as_str().expect("replacement");
            assert_eq!(
                edit.new_text(),
                if crlf {
                    expected_edit.replace('\n', "\r\n")
                } else {
                    expected_edit.to_owned()
                }
            );
            let offset = original.markers[insertion].start.byte;
            edits.push((offset, edit.new_text().to_owned()));

            let mut one_fixed = original.text.clone();
            one_fixed.insert_str(offset, edit.new_text());
            let expected = spec.oracle["applied"][name]
                .as_str()
                .expect("one-fix whole-source oracle");
            assert_eq!(
                one_fixed,
                if crlf {
                    expected.replace('\n', "\r\n")
                } else {
                    expected.to_owned()
                },
                "whole source after {name}, CRLF={crlf}"
            );
            fixture.disk.insert(
                "scripts/invalid.vela".into(),
                parse_markers(&one_fixed).expect("applied source"),
            );
            update(&mut db, &fixture);
            let remaining = db.diagnostics_for_document(&invalid_id);
            assert_eq!(remaining.diagnostics().len(), 2, "after {name}");
            let expected_remaining = expected_names
                .iter()
                .enumerate()
                .filter(|(other, _)| *other != index)
                .map(|(_, owner)| format!("match on `{owner}` does not cover all known variants"))
                .collect::<Vec<_>>();
            assert_eq!(
                remaining
                    .diagnostics()
                    .iter()
                    .map(|diagnostic| diagnostic.message())
                    .collect::<Vec<_>>(),
                expected_remaining,
                "unrelated warnings survive {name}"
            );
            fixture
                .disk
                .insert("scripts/invalid.vela".into(), original.clone());
            update(&mut db, &fixture);
        }

        let mut all_fixed = original.text.clone();
        edits.sort_by_key(|edit| edit.0);
        for (offset, replacement) in edits.into_iter().rev() {
            all_fixed.insert_str(offset, &replacement);
        }
        let expected = spec.oracle["applied"]["all"]
            .as_str()
            .expect("all-fixes whole-source oracle");
        assert_eq!(
            all_fixed,
            if crlf {
                expected.replace('\n', "\r\n")
            } else {
                expected.to_owned()
            }
        );
        assert!(
            vela_syntax::parse::parse_source(&all_fixed)
                .diagnostics()
                .is_empty()
        );
        fixture.disk.insert(
            "scripts/invalid.vela".into(),
            parse_markers(&all_fixed).expect("fully repaired source"),
        );
        update(&mut db, &fixture);
        assert_eq!(db.diagnostics_for_document(&invalid_id).diagnostics(), []);
        assert_eq!(
            db.code_actions(&invalid_id, whole(&fixture.disk["scripts/invalid.vela"])),
            []
        );
        let mut fresh = LanguageServiceDatabases::new();
        update(&mut fresh, &fixture);
        assert_eq!(
            fresh.diagnostics_for_document(&invalid_id).diagnostics(),
            []
        );
        fixture
            .disk
            .insert("scripts/invalid.vela".into(), original.clone());
        update(&mut db, &fixture);
        assert_eq!(
            db.diagnostics_for_document(&invalid_id).diagnostics(),
            initial.diagnostics()
        );
    }
}

#[test]
fn schema_match_arm_patterns_preserve_unit_tuple_and_record_shapes() {
    let mut schema = RegistryFacts::default();
    schema.insert_variant("State", "Idle", TypeFact::enum_type("State", Some("Idle")));
    schema.insert_variant("State", "Pair", TypeFact::enum_type("State", Some("Pair")));
    schema.insert_field("State::Pair", "0", TypeFact::I64);
    schema.insert_field("State::Pair", "1", TypeFact::STRING);
    schema.insert_variant(
        "State",
        "Named",
        TypeFact::enum_type("State", Some("Named")),
    );
    schema.insert_field("State::Named", "value", TypeFact::I64);
    assert_eq!(
        super::missing_variant_pattern("State", "Idle", &schema),
        Some("State::Idle".into())
    );
    assert_eq!(
        super::missing_variant_pattern("State", "Pair", &schema),
        Some("State::Pair(_, _)".into())
    );
    assert_eq!(
        super::missing_variant_pattern("State", "Named", &schema),
        Some("State::Named { value: _ }".into())
    );
    assert_eq!(
        super::missing_variant_pattern("State", "Missing", &schema),
        None
    );
    schema.insert_variant(
        "State",
        "Sparse",
        TypeFact::enum_type("State", Some("Sparse")),
    );
    schema.insert_field("State::Sparse", "2", TypeFact::I64);
    assert_eq!(
        super::missing_variant_pattern("State", "Sparse", &schema),
        None
    );
}

#[test]
fn schema_match_action_applies_tuple_and_record_patterns() {
    let spec = load("diagnostic-action-schema-match");
    for crlf in [false, true] {
        let mut fixture = FixtureWorkspace::new(&spec).expect("fixture");
        if crlf {
            for (name, document) in &mut fixture.disk {
                if name.ends_with(".vela") {
                    *document = parse_markers(&spec.files[name].replace('\n', "\r\n"))
                        .expect("CRLF fixture");
                }
            }
        }
        let original = fixture.disk["scripts/invalid.vela"].clone();
        let schema = fixture.disk["schema.json"].text.clone();
        let files = |fixture: &FixtureWorkspace| {
            fixture
                .disk
                .iter()
                .filter(|(name, _)| name.ends_with(".vela"))
                .map(|(name, document)| {
                    SourceFileSnapshot::new(document_id(name), document.text.as_str())
                })
                .collect::<Vec<_>>()
        };
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = |fixture: &FixtureWorkspace| {
            assemble_project_sources(&config, &files(fixture), &Workspace::new().snapshot())
        };
        let mut db = LanguageServiceDatabases::new();
        db.update(&project(&fixture));
        db.load_schema_artifact_json("/workspace/schema.json", &schema);
        let valid_id = document_id("scripts/valid.vela");
        let invalid_id = document_id("scripts/invalid.vela");
        assert_eq!(db.diagnostics_for_document(&valid_id).diagnostics(), []);
        assert_eq!(
            db.code_actions(&valid_id, whole(&fixture.disk["scripts/valid.vela"])),
            []
        );
        let diagnostics = db.diagnostics_for_document(&invalid_id);
        let [diagnostic] = diagnostics.diagnostics() else {
            panic!("one schema enum warning: {diagnostics:?}");
        };
        assert_eq!(diagnostic.code(), Some("analysis::non_exhaustive_match"));
        assert_eq!(diagnostic.severity(), ServiceDiagnosticSeverity::Warning);
        assert_eq!(
            diagnostic.message(),
            "match on `State` does not cover all known variants"
        );
        assert_eq!(diagnostic.range(), Some(range(&original, "match")));
        assert_eq!(
            diagnostic.labels()[0].message(),
            "missing variants: Named, Pair"
        );
        let actions = db.code_actions(&invalid_id, range(&original, "match"));
        let [action] = actions.as_slice() else {
            panic!("one schema match action: {actions:?}");
        };
        assert_eq!(
            action.title(),
            spec.oracle["title"].as_str().expect("title")
        );
        assert_eq!(action.kind().as_lsp_kind(), "quickfix");
        let [document_edit] = action.edit().document_edits() else {
            panic!("one edited document");
        };
        assert_eq!(document_edit.document_id(), &invalid_id);
        let [edit] = document_edit.edits() else {
            panic!("one text edit");
        };
        assert_eq!(edit.range(), range(&original, "insert"));
        let expected_edit = spec.oracle["replacement"].as_str().expect("replacement");
        assert_eq!(
            edit.new_text(),
            if crlf {
                expected_edit.replace('\n', "\r\n")
            } else {
                expected_edit.to_owned()
            }
        );
        let mut applied = original.text.clone();
        applied.insert_str(original.markers["insert"].start.byte, edit.new_text());
        let expected = spec.oracle["applied"]
            .as_str()
            .expect("whole-source oracle");
        assert_eq!(
            applied,
            if crlf {
                expected.replace('\n', "\r\n")
            } else {
                expected.to_owned()
            }
        );
        assert!(
            vela_syntax::parse::parse_source(&applied)
                .diagnostics()
                .is_empty()
        );
        fixture.disk.insert(
            "scripts/invalid.vela".into(),
            parse_markers(&applied).expect("applied source"),
        );
        db.update(&project(&fixture));
        assert_eq!(db.diagnostics_for_document(&invalid_id).diagnostics(), []);
        assert_eq!(
            db.code_actions(&invalid_id, whole(&fixture.disk["scripts/invalid.vela"])),
            []
        );
        let mut fresh = LanguageServiceDatabases::new();
        fresh.update(&project(&fixture));
        fresh.load_schema_artifact_json("/workspace/schema.json", &schema);
        assert_eq!(
            fresh.diagnostics_for_document(&invalid_id).diagnostics(),
            []
        );
        fixture
            .disk
            .insert("scripts/invalid.vela".into(), original.clone());
        db.update(&project(&fixture));
        assert_eq!(
            db.diagnostics_for_document(&invalid_id).diagnostics(),
            diagnostics.diagnostics()
        );
        assert_eq!(
            db.code_actions(&invalid_id, range(&original, "match")),
            actions
        );
    }
}
