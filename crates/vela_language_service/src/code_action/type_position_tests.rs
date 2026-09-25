use crate::matrix_fixture::{Document, FixtureWorkspace, Spec, load};
use crate::{
    DiagnosticRange, DocumentId, LanguageServiceDatabases, Position, SourceFileSnapshot, Workspace,
    WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

fn uri(file: &str) -> DocumentId {
    DocumentId::from(format!("/workspace/{file}"))
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

fn marker_range(document: &Document, name: &str) -> DiagnosticRange {
    let marker = document.markers[name];
    let point = |line: usize, byte: usize| {
        let line_start = document.text[..byte]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        Position::new(line, byte - line_start)
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

fn repaired(spec: &Spec, document: &Document) -> String {
    let mut edits = spec.oracle["repairs"]
        .as_object()
        .expect("repairs")
        .iter()
        .map(|(name, value)| {
            let marker = document.markers[name];
            (
                marker.start.byte,
                marker.end.byte,
                value.as_str().expect("replacement"),
            )
        })
        .collect::<Vec<_>>();
    edits.sort_by_key(|edit| edit.0);
    for pair in edits.windows(2) {
        assert!(pair[0].1 <= pair[1].0, "independent repairs");
    }
    let mut text = document.text.clone();
    for (start, end, replacement) in edits.into_iter().rev() {
        text.replace_range(start..end, replacement);
    }
    text
}

#[test]
fn type_position_actions_reject_speculative_edits_and_clear_after_manual_repair() {
    let spec = load("diagnostic-type-positions");
    for crlf in [false, true] {
        let mut fixture = FixtureWorkspace::new(&spec).expect("fixture");
        if crlf {
            for (file, document) in &mut fixture.disk {
                *document =
                    crate::matrix_fixture::parse_markers(&spec.files[file].replace('\n', "\r\n"))
                        .expect("CRLF fixture");
            }
        }
        let live = databases(&fixture);
        let valid = uri("scripts/valid.vela");
        let invalid = uri("scripts/invalid.vela");
        let valid_document = &fixture.disk["scripts/valid.vela"];
        let invalid_document = &fixture.disk["scripts/invalid.vela"];
        assert_eq!(live.diagnostics_for_document(&valid).diagnostics(), []);
        assert_eq!(
            live.code_actions(&valid, whole(valid_document)),
            [],
            "valid type positions"
        );
        let diagnostics = live.diagnostics_for_document(&invalid);
        assert_eq!(diagnostics.diagnostics().len(), 12);
        let action_spec = &spec.oracle["codeAction"];
        let action_marker = action_spec["requestMarker"]
            .as_str()
            .expect("action marker");
        for item in spec.oracle["diagnostics"].as_array().expect("diagnostics") {
            let name = item["marker"].as_str().expect("marker");
            let span = marker_range(invalid_document, name);
            assert!(
                diagnostics
                    .diagnostics()
                    .iter()
                    .any(|diagnostic| diagnostic.range() == Some(span)
                        && diagnostic.code() == item["code"].as_str()),
                "positioned diagnostic {name}"
            );
            let actions = live.code_actions(&invalid, span);
            if name == action_marker {
                let [action] = actions.as_slice() else {
                    panic!("one supported type fix")
                };
                assert_eq!(
                    action.title(),
                    action_spec["title"].as_str().expect("title")
                );
                assert_eq!(action.kind().as_lsp_kind(), "quickfix");
                let [target] = action.edit().document_edits() else {
                    panic!("one target")
                };
                let [edit] = target.edits() else {
                    panic!("one edit")
                };
                assert_eq!(target.document_id(), &invalid);
                assert_eq!(
                    edit.range(),
                    marker_range(
                        invalid_document,
                        action_spec["editMarker"].as_str().expect("edit marker")
                    )
                );
                assert_eq!(
                    edit.new_text(),
                    action_spec["replacement"].as_str().expect("replacement")
                );
            } else {
                assert_eq!(actions, [], "no guessed fix at {name}, CRLF={crlf}");
            }
        }
        let mut one_fixed = invalid_document.text.clone();
        let edit_marker =
            invalid_document.markers[action_spec["editMarker"].as_str().expect("edit marker")];
        one_fixed.replace_range(
            edit_marker.start.byte..edit_marker.end.byte,
            action_spec["replacement"].as_str().expect("replacement"),
        );
        assert_eq!(
            one_fixed,
            invalid_document.text.replace("Cell<i64>", "Cell")
        );
        let mut one_fixture = fixture.clone();
        one_fixture.disk.insert(
            "scripts/invalid.vela".into(),
            crate::matrix_fixture::parse_markers(&one_fixed).expect("one fixed source"),
        );
        let one_db = databases(&one_fixture);
        let remaining = one_db.diagnostics_for_document(&invalid);
        let expected_remaining = diagnostics
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code() != Some("syntax::generic_type_hint"))
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(expected_remaining.len(), 11);
        assert_eq!(remaining.diagnostics(), expected_remaining);
        assert_eq!(
            one_db.code_actions(&invalid, whole(&one_fixture.disk["scripts/invalid.vela"])),
            []
        );
        let repaired = repaired(&spec, invalid_document);
        assert!(
            vela_syntax::parse::parse_source(&repaired)
                .diagnostics()
                .is_empty()
        );
        fixture.disk.insert(
            "scripts/invalid.vela".into(),
            crate::matrix_fixture::parse_markers(&repaired).expect("repaired source"),
        );
        let fresh = databases(&fixture);
        assert_eq!(fresh.diagnostics_for_document(&invalid).diagnostics(), []);
        assert_eq!(
            fresh.code_actions(&invalid, whole(&fixture.disk["scripts/invalid.vela"])),
            []
        );
        assert_eq!(
            fresh.code_actions(&valid, whole(&fixture.disk["scripts/valid.vela"])),
            []
        );

        fixture.disk.insert(
            "scripts/incomplete.vela".into(),
            crate::matrix_fixture::parse_markers(
                "/* 中😀 */ fn broken(value: Cell[[open:start]]<[[open:end]]i64) {}",
            )
            .expect("incomplete source"),
        );
        let incomplete_db = databases(&fixture);
        let incomplete = uri("scripts/incomplete.vela");
        let incomplete_document = &fixture.disk["scripts/incomplete.vela"];
        let diagnostics = incomplete_db.diagnostics_for_document(&incomplete);
        assert!(
            diagnostics
                .diagnostics()
                .iter()
                .any(
                    |diagnostic| diagnostic.code() == Some("syntax::generic_type_hint")
                        && diagnostic.repair_hints().is_empty()
                )
        );
        assert_eq!(
            incomplete_db.code_actions(&incomplete, marker_range(incomplete_document, "open")),
            [],
            "unclosed type arguments have no removal edit"
        );
    }
}
