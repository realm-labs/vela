use crate::matrix_fixture::{Document, FixtureWorkspace, load, parse_markers};
use crate::{
    DiagnosticRange, DocumentId, LanguageServiceDatabases, Position, SourceFileSnapshot, Workspace,
    WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
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
fn member_and_constructor_actions_apply_only_the_supported_record_fix() {
    let spec = load("diagnostic-action-members-constructors");
    for crlf in [false, true] {
        let mut fixture = FixtureWorkspace::new(&spec).expect("fixture");
        if crlf {
            for (name, document) in &mut fixture.disk {
                *document =
                    parse_markers(&spec.files[name].replace('\n', "\r\n")).expect("CRLF fixture");
            }
        }
        let invalid_id = document_id("scripts/invalid.vela");
        let valid_id = document_id("scripts/valid.vela");
        let incomplete_id = document_id("scripts/incomplete.vela");
        let invalid = fixture.disk["scripts/invalid.vela"].clone();
        let valid = &fixture.disk["scripts/valid.vela"];
        let incomplete = &fixture.disk["scripts/incomplete.vela"];
        let mut db = LanguageServiceDatabases::new();
        update(&mut db, &fixture);
        assert_eq!(db.diagnostics_for_document(&valid_id).diagnostics(), []);
        assert_eq!(db.code_actions(&valid_id, whole(valid)), []);
        assert!(
            !db.diagnostics_for_document(&incomplete_id)
                .diagnostics()
                .is_empty()
        );
        assert_eq!(
            db.code_actions(&incomplete_id, range(incomplete, "incomplete")),
            [],
            "unclosed record syntax cannot supply an insertion edit"
        );

        let diagnostics = db.diagnostics_for_document(&invalid_id);
        let [diagnostic] = diagnostics.diagnostics() else {
            panic!("one missing field diagnostic, CRLF={crlf}: {diagnostics:?}");
        };
        assert_eq!(
            diagnostic.code(),
            Some("analysis::missing_constructor_field")
        );
        assert_eq!(
            diagnostic.message(),
            "missing constructor field `extra` for `Box`"
        );
        assert_eq!(diagnostic.range(), Some(range(&invalid, "missing")));
        for name in spec.oracle["negative"]
            .as_array()
            .expect("negative markers")
        {
            let name = name.as_str().expect("marker");
            assert_eq!(
                db.code_actions(&invalid_id, range(&invalid, name)),
                [],
                "no guessed source member or variant fix at {name}, CRLF={crlf}"
            );
        }

        let expected = &spec.oracle["selected"][0];
        let actions = db.code_actions(&invalid_id, range(&invalid, "missing"));
        let [action] = actions.as_slice() else {
            panic!("one missing field action, CRLF={crlf}: {actions:?}");
        };
        assert_eq!(action.title(), expected["title"].as_str().expect("title"));
        assert_eq!(action.kind().as_lsp_kind(), "quickfix");
        let [document_edit] = action.edit().document_edits() else {
            panic!("one edited document");
        };
        assert_eq!(document_edit.document_id(), &invalid_id);
        let [edit] = document_edit.edits() else {
            panic!("one text edit");
        };
        assert_eq!(edit.range(), range(&invalid, "missing-insert"));
        assert_eq!(edit.new_text(), expected["newText"].as_str().expect("edit"));
        let mut applied = invalid.text.clone();
        let insertion = invalid.markers["missing-insert"].start.byte;
        applied.insert_str(insertion, edit.new_text());
        let oracle = parse_markers(&if crlf {
            spec.oracle["applied"]
                .as_str()
                .expect("applied")
                .replace('\n', "\r\n")
        } else {
            spec.oracle["applied"].as_str().expect("applied").to_owned()
        })
        .expect("applied oracle");
        assert_eq!(applied, oracle.text, "whole applied source, CRLF={crlf}");
        assert!(
            vela_syntax::parse::parse_source(&applied)
                .diagnostics()
                .is_empty()
        );

        fixture.disk.insert("scripts/invalid.vela".into(), oracle);
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
        assert_eq!(
            fresh.code_actions(&invalid_id, whole(&fixture.disk["scripts/invalid.vela"])),
            []
        );
        fixture
            .disk
            .insert("scripts/invalid.vela".into(), invalid.clone());
        update(&mut db, &fixture);
        assert_eq!(
            db.diagnostics_for_document(&invalid_id).diagnostics(),
            diagnostics.diagnostics()
        );
        assert_eq!(
            db.code_actions(&invalid_id, range(&invalid, "missing")),
            actions
        );
    }
}
