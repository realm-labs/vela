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

fn state_range(item: &serde_json::Value) -> DiagnosticRange {
    let line = item["line"].as_u64().expect("state line") as usize;
    DiagnosticRange::new(
        Position::new(
            line,
            item["startByte"].as_u64().expect("state start") as usize,
        ),
        Position::new(line, item["endByte"].as_u64().expect("state end") as usize),
    )
}

fn repaired_duplicates(spec: &Spec, document: &Document) -> String {
    let mut edits = spec.oracle["repairs"]
        .as_object()
        .expect("repair table")
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
    let mut text = document.text.clone();
    for (start, end, replacement) in edits.into_iter().rev() {
        text.replace_range(start..end, replacement);
    }
    text
}

#[test]
fn top_level_actions_keep_only_applicable_repairs() {
    for crlf in [false, true] {
        let mut spec = load("diagnostic-top-level-declarations");
        if crlf {
            for text in spec.files.values_mut() {
                *text = text.replace('\n', "\r\n");
            }
        }
        let mut fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let db = databases(&fixture);
        let valid = uri("scripts/valid.vela");
        let invalid = uri("scripts/invalid.vela");
        let state = uri("scripts/invalid_state.vela");
        let legacy = uri("scripts/legacy.vela");
        assert_eq!(db.diagnostics_for_document(&valid).diagnostics().len(), 0);
        assert_eq!(db.diagnostics_for_document(&invalid).diagnostics().len(), 7);
        assert_eq!(db.diagnostics_for_document(&state).diagnostics().len(), 3);
        let legacy_diagnostics = db.diagnostics_for_document(&legacy);
        let [legacy_diagnostic] = legacy_diagnostics.diagnostics() else {
            panic!("one legacy syntax diagnostic")
        };
        assert_eq!(legacy_diagnostic.code(), Some("syntax::legacy_global_decl"));
        assert_eq!(legacy_diagnostic.candidates().len(), 2);

        let valid_text = &fixture.disk["scripts/valid.vela"].text;
        let whole_valid = DiagnosticRange::new(
            Position::new(0, 0),
            Position::new(valid_text.lines().count(), 0),
        );
        assert_eq!(
            db.code_actions(&valid, whole_valid),
            [],
            "valid declarations"
        );
        let invalid_document = &fixture.disk["scripts/invalid.vela"];
        for item in spec.oracle["duplicates"].as_array().expect("duplicates") {
            let marker = item["second"].as_str().expect("duplicate marker");
            assert_eq!(
                db.code_actions(&invalid, marker_range(invalid_document, marker)),
                [],
                "duplicate {marker}, CRLF={crlf}"
            );
        }
        for item in spec.oracle["stateErrors"].as_array().expect("state errors") {
            assert_eq!(
                db.code_actions(&state, state_range(item)),
                [],
                "state error {}, CRLF={crlf}",
                item["message"]
            );
        }
        let legacy_document = &fixture.disk["scripts/legacy.vela"];
        assert_eq!(
            db.code_actions(&legacy, marker_range(legacy_document, "legacy")),
            [],
            "placeholder migration candidates must not become destructive edits"
        );

        let fixed_invalid = repaired_duplicates(&spec, invalid_document);
        let line_ending = if crlf { "\r\n" } else { "\n" };
        for (file, text) in [
            ("scripts/invalid.vela", fixed_invalid),
            (
                "scripts/invalid_state.vela",
                spec.oracle["repairedState"]
                    .as_str()
                    .expect("state repair")
                    .replace('\n', line_ending),
            ),
            (
                "scripts/legacy.vela",
                spec.oracle["repairedLegacy"]
                    .as_str()
                    .expect("legacy repair")
                    .replace('\n', line_ending),
            ),
        ] {
            fixture.disk.insert(
                file.into(),
                crate::matrix_fixture::parse_markers(&text).expect("fixed source"),
            );
        }
        let fresh = databases(&fixture);
        for file in [
            "scripts/valid.vela",
            "scripts/invalid.vela",
            "scripts/invalid_state.vela",
            "scripts/legacy.vela",
        ] {
            let document = &fixture.disk[file];
            assert_eq!(
                fresh
                    .diagnostics_for_document(&uri(file))
                    .diagnostics()
                    .len(),
                0,
                "repaired diagnostics {file}"
            );
            assert_eq!(
                fresh.code_actions(
                    &uri(file),
                    DiagnosticRange::new(
                        Position::new(0, 0),
                        Position::new(document.text.lines().count(), 0)
                    )
                ),
                [],
                "repaired actions {file}"
            );
        }
    }
}
