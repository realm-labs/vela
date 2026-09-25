use serde_json::{Value, json};

use super::{DiagnosticRange, ServiceDiagnostic, ServiceDiagnosticSeverity};
use crate::matrix_fixture::{FixtureWorkspace, load, schema_artifact};
use crate::{
    DocumentId, LanguageServiceDatabases, SourceFileSnapshot, Workspace, WorkspaceConfig,
    WorkspaceRoot, assemble_project_sources,
};

fn uri(file: &str) -> DocumentId {
    DocumentId::from(format!("/workspace/{file}"))
}

fn range(span: DiagnosticRange) -> Value {
    json!({"start":[span.start().line,span.start().character],
        "end":[span.end().line,span.end().character]})
}

fn project(diagnostic: &ServiceDiagnostic) -> Value {
    json!({
        "code":diagnostic.code(),"message":diagnostic.message(),
        "severity":match diagnostic.severity() {
            ServiceDiagnosticSeverity::Error => "error",
            ServiceDiagnosticSeverity::Warning => "warning",
            ServiceDiagnosticSeverity::Note => "note",
            ServiceDiagnosticSeverity::Help => "help",
        },
        "range":diagnostic.range().map(range),
        "labels":diagnostic.labels().iter().map(|label| json!({
            "uri":label.document_id().as_str(),"range":range(label.range()),
            "message":label.message(),
        })).collect::<Vec<_>>(),
        "candidates":diagnostic.candidates().len(),
        "repairHints":diagnostic.repair_hints().len(),
    })
}

fn expected(phase: usize) -> Vec<Value> {
    let main = uri("scripts/game/main.vela");
    let reward = uri("scripts/game/reward.vela");
    let spans = [
        json!({"start":[0,14],"end":[0,45]}),
        json!({"start":[1,0],"end":[1,32]}),
        json!({"start":[2,0],"end":[2,31]}),
        json!({"start":[3,0],"end":[3,34]}),
        json!({"start":[4,0],"end":[4,34]}),
    ];
    let label = |uri: &DocumentId, span: Value, message: &str| {
        json!({
            "uri":uri.as_str(),"range":span,"message":message
        })
    };
    let diagnostic =
        |code: &str, message: &str, severity: &str, span: Value, labels: Vec<Value>| {
            json!({
                "code":code,"message":message,"severity":severity,"range":span,"labels":labels,
                "candidates":0,"repairHints":0,
            })
        };
    let module_error = |line: usize, module: &str| {
        diagnostic(
            "hir::unresolved_module",
            &format!("unresolved module `{module}`"),
            "error",
            spans[line].clone(),
            vec![label(
                &main,
                spans[line].clone(),
                "no similar modules found",
            )],
        )
    };
    if phase == 2 || phase == 3 {
        return (0..4)
            .map(|line| module_error(line, "game::reward"))
            .chain(std::iter::once(module_error(4, "game::absent")))
            .collect();
    }
    let (line, name, suggestion) = if phase == 1 {
        (0, "grant", "grnat")
    } else {
        (2, "grnat", "grant")
    };
    let mut result = vec![
        diagnostic(
            "hir::unresolved_import",
            &format!("unresolved import `{name}` in module `game::reward`"),
            "error",
            spans[line].clone(),
            vec![label(
                &main,
                spans[line].clone(),
                &format!("did you mean `{suggestion}`?"),
            )],
        ),
        diagnostic(
            "hir::private_import",
            "declaration `secret` in module `game::reward` is private",
            "error",
            spans[3].clone(),
            vec![
                label(
                    &main,
                    spans[3].clone(),
                    "private declaration cannot be imported from another module",
                ),
                label(
                    &reward,
                    json!({"start":[2,0],"end":[2,25]}),
                    "declaration is private",
                ),
            ],
        ),
        module_error(4, "game::absent"),
        diagnostic(
            "lsp::unused_import",
            "unused import `spare`",
            "warning",
            spans[1].clone(),
            vec![label(&main, spans[1].clone(), "import is never used")],
        ),
    ];
    if phase == 1 {
        result.push(diagnostic(
            "lsp::unused_import",
            "unused import `typo`",
            "warning",
            spans[2].clone(),
            vec![label(&main, spans[2].clone(), "import is never used")],
        ));
    }
    result
}

#[test]
fn source_and_schema_import_diagnostics_have_exact_byte_owners() {
    for crlf in [false, true] {
        for phase in [0, 1, 2, 3, 0] {
            let mut spec = load("diagnostic-import-partitions");
            if phase == 1 {
                spec.files.insert(
                    "scripts/game/reward.vela".into(),
                    spec.oracle["changedDependency"]
                        .as_str()
                        .expect("changed source")
                        .into(),
                );
            } else if phase == 2 || phase == 3 {
                spec.files.remove("scripts/game/reward.vela");
                if phase == 3 {
                    spec.files.insert(
                        "scripts/game/prize.vela".into(),
                        spec.oracle["changedDependency"]
                            .as_str()
                            .expect("renamed module source")
                            .into(),
                    );
                }
            }
            if crlf {
                for text in spec.files.values_mut() {
                    *text = text.replace('\n', "\r\n");
                }
            }
            let fixture = FixtureWorkspace::new(&spec).expect("fixture");
            let sources = fixture
                .disk
                .iter()
                .filter(|(file, _)| file.ends_with(".vela"))
                .map(|(file, document)| SourceFileSnapshot::new(uri(file), document.text.as_str()))
                .collect::<Vec<_>>();
            let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
            let mut db = LanguageServiceDatabases::new();
            db.update(&assemble_project_sources(
                &config,
                &sources,
                &Workspace::new().snapshot(),
            ));
            let artifact = schema_artifact(&spec.oracle["schema"], &fixture, |file| {
                db.source_db().records()[&uri(file)].source_id().get()
            });
            db.load_schema_artifact_json("/workspace/target/schema.json", &artifact.to_string());
            assert!(
                db.schema_db()
                    .facts()
                    .function_fact("host::stamp")
                    .is_some()
            );
            let actual = db.diagnostics_for_document(&uri("scripts/game/main.vela"));
            assert_eq!(
                actual.diagnostics().iter().map(project).collect::<Vec<_>>(),
                expected(phase),
                "phase {phase}, crlf {crlf}"
            );
            if phase == 3 {
                assert!(
                    db.diagnostics_for_document(&uri("scripts/game/prize.vela"))
                        .diagnostics()
                        .is_empty(),
                    "renamed module itself stays valid"
                );
            }
        }
    }
}
