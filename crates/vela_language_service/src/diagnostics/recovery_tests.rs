use crate::matrix_fixture::{Document, Marker, Point, load, parse_markers};
use crate::{
    DiagnosticRange, DocumentId, LanguageServiceDatabases, Position, ServiceDiagnosticSeverity,
    SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
    diagnostics::DiagnosticStatus,
};

const FILE: &str = "/workspace/scripts/game/main.vela";

fn position(text: &str, point: Point) -> Position {
    Position::new(
        point.line,
        point.byte
            - text[..point.byte]
                .rfind('\n')
                .map_or(0, |offset| offset + 1),
    )
}

fn range(document: &Document, marker: Marker) -> DiagnosticRange {
    DiagnosticRange::new(
        position(&document.text, marker.start),
        position(&document.text, marker.end),
    )
}

fn update(db: &mut LanguageServiceDatabases, document: &Document) {
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(
        &config,
        &[SourceFileSnapshot::new(FILE, document.text.as_str())],
        &Workspace::new().snapshot(),
    );
    db.update(&project);
}

fn check(db: &LanguageServiceDatabases, document: &Document, phase: &str, fixed: bool) {
    let result = db.diagnostics_for_document(&DocumentId::from(FILE));
    assert_eq!(result.status(), DiagnosticStatus::Partial, "{phase}");
    let mut expected = Vec::new();
    match phase {
        "type" => expected.push((
            "syntax::type_argument_arity",
            "`Array` expects 1 type argument",
            "bad",
            Vec::<&str>::new(),
        )),
        "declaration" => expected.push(("E_PARSE", "expected `)`", "bad", Vec::new())),
        "baseline" | "member" | "call" | "shifted" | "restored" => {}
        other => panic!("unknown recovery phase {other}"),
    }
    expected.push((
        "hir::unresolved_name",
        "unresolved name `missing`",
        "missing",
        Vec::new(),
    ));
    if !fixed {
        expected.push((
            "analysis::unknown_method",
            "unknown method `frist` for `Array(i64)`",
            "typo",
            vec!["first", "find", "last"],
        ));
    }
    assert_eq!(result.diagnostics().len(), expected.len(), "{phase}");
    for (actual, (code, message, marker, candidates)) in result.diagnostics().iter().zip(expected) {
        assert_eq!(actual.code(), Some(code), "{phase}");
        assert_eq!(actual.message(), message, "{phase}");
        assert_eq!(
            actual.severity(),
            ServiceDiagnosticSeverity::Error,
            "{phase}"
        );
        assert_eq!(
            actual.range(),
            Some(range(document, document.markers[marker])),
            "{phase} {marker} byte range"
        );
        assert_eq!(
            actual
                .candidates()
                .iter()
                .map(|candidate| candidate.replacement())
                .collect::<Vec<_>>(),
            candidates,
            "{phase} {marker} candidates"
        );
    }
}

#[test]
fn incomplete_neighbors_keep_precise_diagnostics_without_empty_member_guess() {
    for crlf in [false, true] {
        let spec = load("diagnostic-recovery-partitions");
        let marked = |source: &str| {
            parse_markers(&if crlf {
                source.replace('\n', "\r\n")
            } else {
                source.to_owned()
            })
            .expect("marked source")
        };
        let baseline = marked(&spec.files["scripts/game/main.vela"]);
        let mut live = LanguageServiceDatabases::new();
        update(&mut live, &baseline);
        check(&live, &baseline, "baseline", false);

        for state in spec.oracle["states"].as_array().expect("states") {
            let phase = state["id"].as_str().expect("phase");
            let document = marked(state["text"].as_str().expect("phase source"));
            let old_generation = live.generation();
            update(&mut live, &document);
            check(&live, &document, phase, false);
            let stale = live
                .diagnostics_for_document_at_generation(&DocumentId::from(FILE), old_generation);
            assert_eq!(stale.status(), DiagnosticStatus::Stale, "{phase}");
            assert!(stale.diagnostics().is_empty(), "{phase} stale facts");
            let mut fresh = LanguageServiceDatabases::new();
            update(&mut fresh, &document);
            assert_eq!(
                live.diagnostics_for_document(&DocumentId::from(FILE))
                    .diagnostics(),
                fresh
                    .diagnostics_for_document(&DocumentId::from(FILE))
                    .diagnostics(),
                "fresh diagnostics {phase}"
            );
        }

        update(&mut live, &baseline);
        check(&live, &baseline, "restored", false);
    }
}

#[test]
fn incomplete_neighbors_keep_exact_actions_and_applied_repairs() {
    for crlf in [false, true] {
        let spec = load("diagnostic-recovery-partitions");
        let marked = |source: &str| {
            parse_markers(&if crlf {
                source.replace('\n', "\r\n")
            } else {
                source.to_owned()
            })
            .expect("marked source")
        };
        let baseline = marked(&spec.files["scripts/game/main.vela"]);
        let mut live = LanguageServiceDatabases::new();
        update(&mut live, &baseline);
        let file = DocumentId::from(FILE);
        for state in spec.oracle["states"].as_array().expect("states") {
            let phase = state["id"].as_str().expect("phase");
            let document = marked(state["text"].as_str().expect("source"));
            update(&mut live, &document);
            let typo = document.markers["typo"];
            let typo_range = range(&document, typo);
            let actions = live.code_actions(&file, typo_range);
            assert_eq!(actions.len(), 3, "{phase} action count");
            for (action, replacement) in actions.iter().zip(["first", "find", "last"]) {
                assert_eq!(
                    action.title(),
                    format!("Replace with `{replacement}`"),
                    "{phase}"
                );
                assert_eq!(action.kind().as_lsp_kind(), "quickfix", "{phase}");
                let [document_edit] = action.edit().document_edits() else {
                    panic!("one document edit {phase}");
                };
                assert_eq!(document_edit.document_id(), &file, "{phase}");
                let [edit] = document_edit.edits() else {
                    panic!("one text edit {phase}");
                };
                assert_eq!(edit.range(), typo_range, "{phase}");
                assert_eq!(edit.new_text(), replacement, "{phase}");
            }
            for marker in ["missing", "bad"] {
                if let Some(site) = document.markers.get(marker) {
                    assert!(
                        live.code_actions(&file, range(&document, *site)).is_empty(),
                        "no guessed action at {phase} {marker}"
                    );
                }
            }

            let mut fresh = LanguageServiceDatabases::new();
            update(&mut fresh, &document);
            assert_eq!(
                actions,
                fresh.code_actions(&file, typo_range),
                "fresh action set {phase}"
            );
            for marker in ["missing", "bad"] {
                if let Some(site) = document.markers.get(marker) {
                    assert!(
                        fresh
                            .code_actions(&file, range(&document, *site))
                            .is_empty(),
                        "fresh negative action {phase} {marker}"
                    );
                }
            }

            let selected = &actions[0].edit().document_edits()[0].edits()[0];
            let mut applied_text = document.text.clone();
            applied_text.replace_range(typo.start.byte..typo.end.byte, selected.new_text());
            let applied_source = format!(
                "{}{}{}",
                state["appliedPrefix"].as_str().unwrap_or(""),
                spec.oracle["appliedHead"].as_str().expect("applied head"),
                state["appliedTail"].as_str().expect("applied tail")
            );
            let applied = marked(&applied_source);
            assert_eq!(applied_text, applied.text, "whole applied source {phase}");
            update(&mut fresh, &applied);
            check(&fresh, &applied, phase, true);
            for marker in ["typo", "missing", "bad"] {
                if let Some(site) = applied.markers.get(marker) {
                    assert!(
                        fresh.code_actions(&file, range(&applied, *site)).is_empty(),
                        "no stale or guessed action after repair {phase} {marker}"
                    );
                }
            }
            if phase == "shifted" {
                assert!(
                    live.code_actions(&file, range(&baseline, baseline.markers["typo"]))
                        .is_empty(),
                    "old byte range must not repair shifted text"
                );
            }
        }
        update(&mut live, &baseline);
        let restored = live.code_actions(&file, range(&baseline, baseline.markers["typo"]));
        assert_eq!(
            restored
                .iter()
                .map(|action| action.title())
                .collect::<Vec<_>>(),
            [
                "Replace with `first`",
                "Replace with `find`",
                "Replace with `last`"
            ],
            "restored actions"
        );
    }
}
