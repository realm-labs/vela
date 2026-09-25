use crate::matrix_fixture::{FixtureWorkspace, Point, load, parse_markers};
use crate::{
    DiagnosticRange, DocumentId, LanguageServiceDatabases, Position, ServiceDiagnosticSeverity,
    SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

fn position(text: &str, point: Point) -> Position {
    Position::new(
        point.line,
        point.byte
            - text[..point.byte]
                .rfind('\n')
                .map_or(0, |offset| offset + 1),
    )
}

fn update(db: &mut LanguageServiceDatabases, document: &DocumentId, text: &str) {
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(
        &config,
        &[SourceFileSnapshot::new(document.clone(), text)],
        &Workspace::new().snapshot(),
    );
    db.update(&project);
}

#[test]
fn method_typo_fix_clears_only_its_diagnostic_after_applied_edit() {
    for crlf in [false, true] {
        let mut spec = load("diagnostic-action-method-typo");
        if crlf {
            for source in spec.files.values_mut() {
                *source = source.replace('\n', "\r\n");
            }
        }
        let fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let source = &fixture.disk["scripts/game/main.vela"];
        let applied_source = spec.oracle["applied"].as_str().expect("applied source");
        let applied = parse_markers(&if crlf {
            applied_source.replace('\n', "\r\n")
        } else {
            applied_source.to_owned()
        })
        .expect("applied markers");
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let mut db = LanguageServiceDatabases::new();
        update(&mut db, &document, &source.text);

        let expected = spec.oracle["diagnostics"].as_array().expect("diagnostics");
        let actual = db.diagnostics_for_document(&document);
        assert_eq!(actual.diagnostics().len(), expected.len(), "before {crlf}");
        for (diagnostic, expected) in actual.diagnostics().iter().zip(expected) {
            let marker = expected["marker"].as_str().expect("marker");
            let range = source.markers[marker];
            assert_eq!(diagnostic.code(), expected["code"].as_str());
            assert_eq!(
                diagnostic.message(),
                expected["message"].as_str().expect("diagnostic message")
            );
            assert_eq!(diagnostic.severity(), ServiceDiagnosticSeverity::Error);
            assert_eq!(
                diagnostic.range(),
                Some(DiagnosticRange::new(
                    position(&source.text, range.start),
                    position(&source.text, range.end)
                ))
            );
        }

        let fix = source.markers["fix"];
        let range = DiagnosticRange::new(
            position(&source.text, fix.start),
            position(&source.text, fix.end),
        );
        let actions = db.code_actions(&document, range);
        let titles = actions
            .iter()
            .map(|action| action.title())
            .collect::<Vec<_>>();
        let expected_titles = spec.oracle["actionTitles"]
            .as_array()
            .expect("action titles")
            .iter()
            .map(|title| title.as_str().expect("title"))
            .collect::<Vec<_>>();
        assert_eq!(titles, expected_titles, "exact fix set {crlf}");
        let replacements = spec.oracle["actionReplacements"]
            .as_array()
            .expect("action replacements");
        assert_eq!(actions.len(), replacements.len());
        for (action, replacement) in actions.iter().zip(replacements) {
            assert_eq!(action.kind().as_lsp_kind(), "quickfix");
            let [document_edit] = action.edit().document_edits() else {
                panic!("one document edit");
            };
            assert_eq!(document_edit.document_id(), &document);
            let [edit] = document_edit.edits() else {
                panic!("one text edit");
            };
            assert_eq!(edit.range(), range);
            assert_eq!(edit.new_text(), replacement.as_str().expect("replacement"));
        }
        let action = actions
            .iter()
            .find(|action| {
                action.title()
                    == spec.oracle["action"]["title"]
                        .as_str()
                        .expect("action title")
            })
            .expect("selected fix");
        assert_eq!(
            action.title(),
            spec.oracle["action"]["title"]
                .as_str()
                .expect("action title")
        );
        assert_eq!(action.kind().as_lsp_kind(), "quickfix");
        let [document_edit] = action.edit().document_edits() else {
            panic!("one document edit");
        };
        assert_eq!(document_edit.document_id(), &document);
        let [edit] = document_edit.edits() else {
            panic!("one text edit");
        };
        assert_eq!(edit.range(), range);
        assert_eq!(edit.new_text(), "first");
        let mut actual_text = source.text.clone();
        actual_text.replace_range(fix.start.byte..fix.end.byte, edit.new_text());
        assert_eq!(actual_text, applied.text, "whole applied source {crlf}");
        assert!(
            vela_syntax::parse::parse_source(&actual_text)
                .diagnostics()
                .is_empty()
        );

        update(&mut db, &document, &actual_text);
        let diagnostics = db.diagnostics_for_document(&document);
        let [remaining] = diagnostics.diagnostics() else {
            panic!("only unrelated diagnostic remains: {diagnostics:?}");
        };
        let unrelated = applied.markers["unrelated"];
        assert_eq!(remaining.code(), expected[1]["code"].as_str());
        assert_eq!(remaining.severity(), ServiceDiagnosticSeverity::Error);
        assert_eq!(
            remaining.message(),
            expected[1]["message"].as_str().expect("diagnostic message")
        );
        assert_eq!(
            remaining.range(),
            Some(DiagnosticRange::new(
                position(&applied.text, unrelated.start),
                position(&applied.text, unrelated.end)
            ))
        );
        let fixed = applied.markers["fix"];
        assert!(
            db.code_actions(
                &document,
                DiagnosticRange::new(
                    position(&applied.text, fixed.start),
                    position(&applied.text, fixed.end)
                )
            )
            .is_empty()
        );
    }
}
