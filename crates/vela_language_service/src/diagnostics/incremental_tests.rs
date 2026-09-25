use serde_json::{Value, json};

use super::{DiagnosticStatus, ServiceDiagnosticSeverity};
use crate::matrix_fixture::{Document, FixtureWorkspace, Marker, load, parse_markers};
use crate::{
    DocumentId, LanguageServiceDatabases, SourceFileSnapshot, Workspace, WorkspaceConfig,
    WorkspaceRoot, assemble_project_sources,
};

fn uri(file: &str) -> DocumentId {
    DocumentId::from(format!("/workspace/{file}"))
}

fn project(fixture: &FixtureWorkspace) -> crate::ProjectSources {
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let files = fixture
        .disk
        .iter()
        .filter(|(file, _)| file.ends_with(".vela"))
        .map(|(file, source)| SourceFileSnapshot::new(uri(file), source.text.as_str()))
        .collect::<Vec<_>>();
    assemble_project_sources(&config, &files, &Workspace::new().snapshot())
}

fn byte_range(document: &Document, marker: Marker) -> Value {
    let point = |byte: usize, line: usize| {
        let line_start = document.text[..byte]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        json!([line, byte - line_start])
    };
    json!({"start":point(marker.start.byte,marker.start.line),
        "end":point(marker.end.byte,marker.end.line)})
}

fn diagnostics(db: &LanguageServiceDatabases, file: &str) -> Value {
    let result = db.diagnostics_for_document(&uri(file));
    assert_ne!(result.status(), DiagnosticStatus::Stale);
    json!(result.diagnostics().iter().map(|diagnostic| {
        json!({
            "code":diagnostic.code(),"message":diagnostic.message(),
            "severity":match diagnostic.severity() {
                ServiceDiagnosticSeverity::Error => "error",
                ServiceDiagnosticSeverity::Warning => "warning",
                ServiceDiagnosticSeverity::Note => "note",
                ServiceDiagnosticSeverity::Help => "help",
            },
            "range":diagnostic.range().map(|range| json!({
                "start":[range.start().line,range.start().character],
                "end":[range.end().line,range.end().character]
            })),
            "labels":diagnostic.labels().iter().map(|label| json!({
                "uri":label.document_id().as_str(),
                "range":{"start":[label.range().start().line,label.range().start().character],
                    "end":[label.range().end().line,label.range().end().character]},
                "message":label.message()
            })).collect::<Vec<_>>(),
            "candidates":diagnostic.candidates().iter().map(|item| item.replacement()).collect::<Vec<_>>(),
            "repairHints":diagnostic.repair_hints().len()
        })
    }).collect::<Vec<_>>())
}

fn expected_main(document: &Document, unresolved: bool) -> Value {
    let main = uri("scripts/main.vela");
    let label = |span: Value, message: &str| {
        json!({
            "uri":main.as_str(),"range":span,"message":message
        })
    };
    let mut expected = Vec::new();
    if unresolved {
        let span = byte_range(document, document.markers["import"]);
        expected.push(json!({
            "code":"hir::unresolved_import",
            "message":"unresolved import `make` in module `helper`",
            "severity":"error","range":span,
            "labels":[label(span,"no similar declarations found")],
            "candidates":[],"repairHints":0
        }));
    }
    let span = byte_range(document, document.markers["typo"]);
    expected.push(json!({
        "code":"analysis::unknown_method",
        "message":"unknown method `frist` for `Array(i64)`",
        "severity":"error","range":span,
        "labels":[label(span.clone(),"unknown member access"),
            label(span.clone(),"did you mean `first`?"),
            label(span,"similar candidates: first, find, last")],
        "candidates":["first","find","last"],"repairHints":0
    }));
    json!(expected)
}

#[test]
fn incremental_diagnostics_follow_fingerprints_and_reject_stale_generations() {
    let spec = load("diagnostic-incremental");
    for crlf in [false, true] {
        let mut fixture = FixtureWorkspace::new(&spec).expect("fixture");
        if crlf {
            for (file, source) in &mut fixture.disk {
                *source =
                    parse_markers(&spec.files[file].replace('\n', "\r\n")).expect("CRLF source");
            }
        }
        let mut db = LanguageServiceDatabases::new();
        db.update(&project(&fixture));
        let files = [
            "scripts/main.vela",
            "scripts/helper.vela",
            "scripts/relay.vela",
            "scripts/unrelated.vela",
        ];
        let mut unresolved = false;
        for step in std::iter::once(None).chain(
            spec.oracle["steps"]
                .as_array()
                .expect("steps")
                .iter()
                .map(Some),
        ) {
            if let Some(step) = step {
                let file = step["file"].as_str().expect("file");
                let key = db.source_db().records()[&uri(file)].module_key().clone();
                let before = db.clone();
                let fingerprint = before
                    .parse_db()
                    .module_fingerprint(&key)
                    .expect("fingerprint");
                let old_generation = db.generation();
                let source = step["source"]
                    .as_str()
                    .expect("source")
                    .replace('\n', if crlf { "\r\n" } else { "\n" });
                fixture
                    .disk
                    .insert(file.to_owned(), parse_markers(&source).expect("source"));
                let report = db.update(&project(&fixture));
                assert_eq!(
                    report.reparsed_documents().iter().collect::<Vec<_>>(),
                    [&uri(file)]
                );
                let current = db.parse_db().module_fingerprint(&key).expect("fingerprint");
                assert_eq!(
                    current.declaration() != fingerprint.declaration(),
                    step["declarationChanged"] == true
                );
                assert_eq!(
                    current.import() != fingerprint.import(),
                    step["importChanged"] == true
                );
                let invalidated = report
                    .analysis_invalidated_modules()
                    .iter()
                    .map(|key| key.path.join())
                    .collect::<Vec<_>>();
                assert_eq!(json!(invalidated), step["invalidated"], "{}", step["id"]);
                assert_eq!(
                    report.hir_invalidated_modules(),
                    report.analysis_invalidated_modules()
                );
                assert_eq!(
                    db.project_db().rebuild_count() - before.project_db().rebuild_count(),
                    usize::from(
                        step["declarationChanged"] == true || step["importChanged"] == true
                    )
                );
                let stale = db.diagnostics_for_document_at_generation(
                    &uri("scripts/main.vela"),
                    old_generation,
                );
                assert_eq!(stale.status(), DiagnosticStatus::Stale);
                assert!(stale.diagnostics().is_empty());
                unresolved = step["id"] == "declaration_renamed";
            }
            let main = &fixture.disk["scripts/main.vela"];
            assert_eq!(
                diagnostics(&db, "scripts/main.vela"),
                expected_main(main, unresolved)
            );
            for file in &files[1..] {
                assert_eq!(diagnostics(&db, file), json!([]), "{file}");
            }
            let generation = db.generation();
            let repeat = diagnostics(&db, "scripts/main.vela");
            assert_eq!(repeat, diagnostics(&db, "scripts/main.vela"));
            assert_eq!(db.generation(), generation, "query is read-only");
            let mut fresh = LanguageServiceDatabases::new();
            fresh.update(&project(&fixture));
            for file in files {
                assert_eq!(
                    db.diagnostics_for_document(&uri(file)).diagnostics(),
                    fresh.diagnostics_for_document(&uri(file)).diagnostics(),
                    "fresh {file}"
                );
            }
        }
    }
}
