use serde_json::{Value, json};

use super::{ServiceDiagnostic, ServiceDiagnosticSeverity};
use crate::matrix_fixture::{load, parse_markers};
use crate::{
    DocumentId, LanguageServiceDatabases, SourceFileSnapshot, SourceVersion, Workspace,
    WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

fn diagnostic(value: &ServiceDiagnostic) -> Value {
    let range = |span: super::DiagnosticRange| {
        json!({"start":[span.start().line, span.start().character],
            "end":[span.end().line, span.end().character]})
    };
    json!({
        "code":value.code(),"message":value.message(),
        "severity":match value.severity() {
            ServiceDiagnosticSeverity::Error => "error",
            ServiceDiagnosticSeverity::Warning => "warning",
            ServiceDiagnosticSeverity::Note => "note",
            ServiceDiagnosticSeverity::Help => "help",
        },
        "range":value.range().map(range),
        "labels":value.labels().iter().map(|label| json!({
            "uri":label.document_id().as_str(),"range":range(label.range()),
            "message":label.message()
        })).collect::<Vec<_>>(),
        "candidates":value.candidates().iter().map(|item| item.replacement()).collect::<Vec<_>>(),
        "repairHints":value.repair_hints().len(),
    })
}

fn current(
    config: &WorkspaceConfig,
    files: &[SourceFileSnapshot],
    workspace: &Workspace,
    document: &DocumentId,
) -> Value {
    let mut db = LanguageServiceDatabases::new();
    db.update(&assemble_project_sources(
        config,
        files,
        &workspace.snapshot(),
    ));
    json!(
        db.diagnostics_for_document(document)
            .diagnostics()
            .iter()
            .map(diagnostic)
            .collect::<Vec<_>>()
    )
}

fn expected_typo(document: &DocumentId, marker: crate::matrix_fixture::Marker) -> Value {
    let span = json!({"start":[marker.start.line,marker.start.byte],
        "end":[marker.end.line,marker.end.byte]});
    let label = |message: &str| {
        json!({
            "uri":document.as_str(),"range":span,"message":message
        })
    };
    json!([{
        "code":"analysis::unknown_method",
        "message":"unknown method `frist` for `Array(i64)`",
        "severity":"error","range":span,
        "labels":[label("unknown member access"),label("did you mean `first`?"),
            label("similar candidates: first, find, last")],
        "candidates":["first","find","last"],"repairHints":0
    }])
}

#[test]
fn workspace_overlay_and_scratch_diagnostics_follow_exact_source_state() {
    let spec = load("diagnostic-source-state");
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let helper = DocumentId::from("/workspace/scripts/game/helper.vela");
    let scratch = DocumentId::from("/outside/独立 scratch.vela");
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let disk = spec.files["scripts/game/main.vela"].as_str();
    let helper_text = spec.files["scripts/game/helper.vela"].as_str();

    for crlf in [false, true] {
        let line_ending = if crlf { "\r\n" } else { "\n" };
        let disk = disk.replace('\n', line_ending);
        let helper_text = helper_text.replace('\n', line_ending);
        let overlay = parse_markers(
            &spec.oracle["overlay"]
                .as_str()
                .expect("overlay")
                .replace('\n', line_ending),
        )
        .expect("marked overlay");
        let scratch_source = parse_markers(
            &spec.oracle["scratch"]
                .as_str()
                .expect("scratch")
                .replace('\n', line_ending),
        )
        .expect("marked scratch");
        let files = vec![
            SourceFileSnapshot::new(main.clone(), disk.as_str()),
            SourceFileSnapshot::new(helper.clone(), helper_text.as_str()),
        ];
        let mut workspace = Workspace::new();
        assert_eq!(current(&config, &files, &workspace, &main), json!([]));
        assert_eq!(current(&config, &files, &workspace, &helper), json!([]));

        workspace.open_document(main.clone(), overlay.text.as_str(), SourceVersion::new(2));
        let expected = expected_typo(&main, overlay.markers["typo"]);
        assert_eq!(
            current(&config, &files, &workspace, &main),
            expected,
            "overlay {crlf}"
        );
        assert_eq!(current(&config, &files, &workspace, &helper), json!([]));

        workspace.close_document(&main);
        assert_eq!(
            current(&config, &files, &workspace, &main),
            json!([]),
            "disk restored"
        );
        workspace.open_document(
            scratch.clone(),
            scratch_source.text.as_str(),
            SourceVersion::new(3),
        );
        let expected = expected_typo(&scratch, scratch_source.markers["typo"]);
        assert_eq!(
            current(
                &WorkspaceConfig::scratch(scratch.clone()),
                &[],
                &workspace,
                &scratch
            ),
            expected
        );
        assert_eq!(
            current(&config, &files, &workspace, &scratch),
            expected,
            "configured workspace retains outside scratch"
        );
        assert_eq!(
            current(&config, &files, &workspace, &main),
            json!([]),
            "outside root does not pollute workspace"
        );
        workspace.close_document(&scratch);
        assert_eq!(
            current(&config, &files, &workspace, &scratch),
            json!([]),
            "scratch clears on close"
        );
    }
}

#[test]
fn schema_absence_replacement_invalidity_and_restore_keep_source_diagnostics() {
    let spec = load("diagnostic-action-schema-lifecycle");
    let document_id = DocumentId::from("/workspace/scripts/main.vela");
    let schema = "/workspace/schema.json";
    for crlf in [false, true] {
        let source = parse_markers(
            &spec.files["scripts/main.vela"].replace('\n', if crlf { "\r\n" } else { "\n" }),
        )
        .expect("schema lifecycle source");
        let files = [SourceFileSnapshot::new(
            document_id.clone(),
            source.text.as_str(),
        )];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut db = LanguageServiceDatabases::new();
        db.update(&project);
        for state in spec.oracle["states"].as_array().expect("schema states") {
            match state["operation"].as_str().expect("operation") {
                "level" => {
                    db.load_schema_artifact_json(schema, &spec.oracle["levelSchema"].to_string())
                }
                "rank" => {
                    db.load_schema_artifact_json(schema, &spec.oracle["rankSchema"].to_string())
                }
                "delete" => db.mark_schema_missing(schema),
                "invalid" => db.load_schema_artifact_json(schema, "{ broken"),
                operation => panic!("unknown schema operation {operation}"),
            }
            let actual = db.diagnostics_for_document(&document_id);
            let actual = actual.diagnostics().iter().map(|diagnostic| {
                let span = diagnostic.range().map(|range| json!({
                    "start":[range.start().line,range.start().character],
                    "end":[range.end().line,range.end().character]
                }));
                json!({
                    "code":diagnostic.code(),"message":diagnostic.message(),
                    "severity":match diagnostic.severity() {
                        ServiceDiagnosticSeverity::Error => "error",
                        ServiceDiagnosticSeverity::Warning => "warning",
                        ServiceDiagnosticSeverity::Note => "note",
                        ServiceDiagnosticSeverity::Help => "help",
                    },
                    "range":span,
                    "candidates":diagnostic.candidates().iter().map(|candidate| candidate.replacement()).collect::<Vec<_>>()
                })
            }).collect::<Vec<_>>();
            let span = |marker: &str| {
                let marker = source.markers[marker];
                let line_start = source.text[..marker.start.byte]
                    .rfind('\n')
                    .map_or(0, |index| index + 1);
                json!({"start":[marker.start.line,marker.start.byte - line_start],
                    "end":[marker.end.line,marker.end.byte - line_start]})
            };
            let mut expected = vec![json!({
                "code":"analysis::unknown_method",
                "message":"unknown method `frist` for `Array(i64)`",
                "severity":"error","range":span("control"),
                "candidates":["first","find","last"]
            })];
            if let Some(field) = state["field"].as_str() {
                for (marker, typo) in [("old", "levle"), ("new", "rnak")] {
                    expected.push(json!({
                        "code":"analysis::unknown_field",
                        "message":format!("unknown field `{typo}` for `Player`"),
                        "severity":"error","range":span(marker),"candidates":[field]
                    }));
                }
            } else {
                expected.push(json!({
                    "code":"schema::unavailable",
                    "message":format!("host schema `{schema}` {}", state["warning"].as_str().expect("warning")),
                    "severity":"warning","range":null,"candidates":[]
                }));
            }
            assert_eq!(actual, expected, "{} CRLF={crlf}", state["id"]);
        }
    }
}
