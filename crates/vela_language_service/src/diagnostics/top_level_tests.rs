use serde_json::{Value, json};

use super::{DiagnosticRange, ServiceDiagnostic, ServiceDiagnosticSeverity};
use crate::matrix_fixture::{Document, FixtureWorkspace, Spec, load, parse_markers};
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

fn update(db: &mut LanguageServiceDatabases, fixture: &FixtureWorkspace) {
    let files = fixture
        .disk
        .iter()
        .filter(|(file, _)| file.ends_with(".vela"))
        .map(|(file, document)| SourceFileSnapshot::new(uri(file), document.text.as_str()))
        .collect::<Vec<_>>();
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    db.update(&assemble_project_sources(
        &config,
        &files,
        &Workspace::new().snapshot(),
    ));
}

fn all(db: &LanguageServiceDatabases, file: &str) -> Value {
    json!(
        db.diagnostics_for_document(&uri(file))
            .diagnostics()
            .iter()
            .map(project)
            .collect::<Vec<_>>()
    )
}

fn marker_range(document: &Document, name: &str) -> Value {
    let marker = document.markers[name];
    let point = |line: usize, byte: usize| {
        let line_start = document.text[..byte]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        json!([line, byte - line_start])
    };
    json!({"start":point(marker.start.line,marker.start.byte),
        "end":point(marker.end.line,marker.end.byte)})
}

fn expected_duplicates(spec: &Spec, document: &Document) -> Value {
    let uri = uri("scripts/invalid.vela");
    json!(
        spec.oracle["duplicates"]
            .as_array()
            .expect("valid top-level fixture")
            .iter()
            .map(|item| {
                let first = marker_range(
                    document,
                    item["first"].as_str().expect("valid top-level fixture"),
                );
                let second = marker_range(
                    document,
                    item["second"].as_str().expect("valid top-level fixture"),
                );
                let kind = item["kind"].as_str().expect("valid top-level fixture");
                json!({"code":item["code"],"message":item["message"],"severity":"error",
            "range":second,"labels":[
                {"uri":uri.as_str(),"range":first,"message":format!("previous {kind} is here")},
                {"uri":uri.as_str(),"range":second,"message":format!("duplicate {kind} is here")}
            ],"candidates":0,"repairHints":0})
            })
            .collect::<Vec<_>>()
    )
}

fn expected_state(spec: &Spec) -> Value {
    json!(
        spec.oracle["stateErrors"]
            .as_array()
            .expect("valid top-level fixture")
            .iter()
            .map(|item| {
                let line = item["line"].as_u64().expect("valid top-level fixture");
                let start = item["startByte"].as_u64().expect("valid top-level fixture");
                let end = item["endByte"].as_u64().expect("valid top-level fixture");
                json!({"code":"E_PARSE","message":item["message"],"severity":"error",
            "range":{"start":[line,start],"end":[line,end]},
            "labels":[],"candidates":0,"repairHints":0})
            })
            .collect::<Vec<_>>()
    )
}

fn repaired(spec: &Spec, document: &Document) -> String {
    let mut edits = spec.oracle["repairs"]
        .as_object()
        .expect("valid top-level fixture")
        .iter()
        .map(|(marker, value)| {
            let span = document.markers[marker];
            (
                span.start.byte,
                span.end.byte,
                value.as_str().expect("valid top-level fixture"),
            )
        })
        .collect::<Vec<_>>();
    edits.sort_by_key(|edit| edit.0);
    let mut text = document.text.clone();
    for (start, end, value) in edits.into_iter().rev() {
        text.replace_range(start..end, value);
    }
    text
}

#[test]
fn top_level_declaration_diagnostics_pin_duplicates_and_state_forms() {
    let spec = load("diagnostic-top-level-declarations");
    for crlf in [false, true] {
        let mut fixture = FixtureWorkspace::new(&spec).expect("valid top-level fixture");
        if crlf {
            for (file, document) in &mut fixture.disk {
                *document = parse_markers(&spec.files[file].replace('\n', "\r\n"))
                    .expect("valid top-level fixture");
            }
        }
        let mut live = LanguageServiceDatabases::new();
        update(&mut live, &fixture);
        assert_eq!(all(&live, "scripts/defs.vela"), json!([]));
        assert_eq!(all(&live, "scripts/valid.vela"), json!([]));
        assert_eq!(
            all(&live, "scripts/invalid.vela"),
            expected_duplicates(&spec, &fixture.disk["scripts/invalid.vela"]),
            "CRLF={crlf}"
        );
        assert_eq!(
            all(&live, "scripts/invalid_state.vela"),
            expected_state(&spec),
            "CRLF={crlf}"
        );

        let repaired_invalid = repaired(&spec, &fixture.disk["scripts/invalid.vela"]);
        let repaired_state = spec.oracle["repairedState"]
            .as_str()
            .expect("valid top-level fixture")
            .replace('\n', if crlf { "\r\n" } else { "\n" });
        for (file, text) in [
            ("scripts/invalid.vela", repaired_invalid),
            ("scripts/invalid_state.vela", repaired_state),
        ] {
            assert!(
                vela_syntax::parse::parse_source(&text)
                    .diagnostics()
                    .is_empty(),
                "{file}"
            );
            fixture.disk.insert(
                file.into(),
                parse_markers(&text).expect("valid top-level fixture"),
            );
        }
        update(&mut live, &fixture);
        let mut fresh = LanguageServiceDatabases::new();
        update(&mut fresh, &fixture);
        for file in [
            "scripts/defs.vela",
            "scripts/valid.vela",
            "scripts/invalid.vela",
            "scripts/invalid_state.vela",
        ] {
            assert_eq!(all(&live, file), json!([]), "repaired {file}");
            assert_eq!(all(&live, file), all(&fresh, file), "fresh {file}");
        }
    }
}
