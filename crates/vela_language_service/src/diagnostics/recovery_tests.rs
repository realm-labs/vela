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

fn check(db: &LanguageServiceDatabases, document: &Document, phase: &str) {
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
        "baseline" | "member" | "call" | "restored" => {}
        other => panic!("unknown recovery phase {other}"),
    }
    expected.push((
        "hir::unresolved_name",
        "unresolved name `missing`",
        "missing",
        Vec::new(),
    ));
    expected.push((
        "analysis::unknown_method",
        "unknown method `frist` for `Array(i64)`",
        "typo",
        vec!["first", "find", "last"],
    ));
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
        check(&live, &baseline, "baseline");

        for state in spec.oracle["states"].as_array().expect("states") {
            let phase = state["id"].as_str().expect("phase");
            let document = marked(state["text"].as_str().expect("phase source"));
            let old_generation = live.generation();
            update(&mut live, &document);
            check(&live, &document, phase);
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
        check(&live, &baseline, "restored");
    }
}
