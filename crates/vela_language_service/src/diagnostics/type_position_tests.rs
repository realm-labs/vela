use serde_json::{Value, json};

use super::{ServiceDiagnostic, ServiceDiagnosticSeverity};
use crate::matrix_fixture::{Document, FixtureWorkspace, Spec, load, parse_markers};
use crate::{
    DocumentId, LanguageServiceDatabases, SourceFileSnapshot, Workspace, WorkspaceConfig,
    WorkspaceRoot, assemble_project_sources,
};

fn uri(file: &str) -> DocumentId {
    DocumentId::from(format!("/workspace/{file}"))
}

fn byte_range(document: &Document, marker: &str) -> Value {
    let marker = document.markers[marker];
    let point = |byte: usize, line: usize| {
        let line_start = document.text[..byte]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        json!([line, byte - line_start])
    };
    json!({"start":point(marker.start.byte, marker.start.line),
        "end":point(marker.end.byte, marker.end.line)})
}

fn expected(spec: &Spec, document: &Document) -> Value {
    let invalid = uri("scripts/invalid.vela");
    json!(
        spec.oracle["diagnostics"]
            .as_array()
            .expect("oracle diagnostics")
            .iter()
            .map(|item| {
                let span = byte_range(document, item["marker"].as_str().expect("marker"));
                json!({"code":item["code"],"message":item["message"],"severity":"error",
                "range":span,"labels":[{"uri":invalid.as_str(),"range":span,
                    "message":item["label"]}],"candidates":[],"repairHints":0})
            })
            .collect::<Vec<_>>()
    )
}

fn diagnostic(value: &ServiceDiagnostic) -> Value {
    let range = |span: super::DiagnosticRange| {
        json!({"start":[span.start().line,span.start().character],
            "end":[span.end().line,span.end().character]})
    };
    json!({"code":value.code(),"message":value.message(),
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
        "candidates":value.candidates().iter().map(|candidate| candidate.replacement()).collect::<Vec<_>>(),
        "repairHints":value.repair_hints().len()
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
            .map(diagnostic)
            .collect::<Vec<_>>()
    )
}

fn repaired(spec: &Spec, document: &Document) -> String {
    let mut edits = spec.oracle["repairs"]
        .as_object()
        .expect("repairs")
        .iter()
        .map(|(marker, replacement)| {
            let span = document.markers[marker];
            (
                span.start.byte,
                span.end.byte,
                replacement.as_str().expect("replacement"),
            )
        })
        .collect::<Vec<_>>();
    edits.sort_by_key(|edit| edit.0);
    for pair in edits.windows(2) {
        assert!(pair[0].1 <= pair[1].0, "repairs do not overlap");
    }
    let mut text = document.text.clone();
    for (start, end, replacement) in edits.into_iter().rev() {
        text.replace_range(start..end, replacement);
    }
    text
}

#[test]
fn type_hint_diagnostics_enforce_only_supported_contracts_and_clear_after_repair() {
    let spec = load("diagnostic-type-positions");
    for crlf in [false, true] {
        let mut fixture = FixtureWorkspace::new(&spec).expect("fixture");
        if crlf {
            for (file, document) in &mut fixture.disk {
                *document =
                    parse_markers(&spec.files[file].replace('\n', "\r\n")).expect("CRLF source");
            }
        }
        let mut live = LanguageServiceDatabases::new();
        update(&mut live, &fixture);
        let invalid = &fixture.disk["scripts/invalid.vela"];
        assert_eq!(
            all(&live, "scripts/invalid.vela"),
            expected(&spec, invalid),
            "CRLF={crlf}"
        );
        for file in ["scripts/defs.vela", "scripts/valid.vela"] {
            assert_eq!(all(&live, file), json!([]), "valid type positions {file}");
        }
        let repaired = repaired(&spec, invalid);
        assert!(
            vela_syntax::parse::parse_source(&repaired)
                .diagnostics()
                .is_empty()
        );
        fixture.disk.insert(
            "scripts/invalid.vela".into(),
            parse_markers(&repaired).expect("repaired source"),
        );
        update(&mut live, &fixture);
        for file in [
            "scripts/defs.vela",
            "scripts/valid.vela",
            "scripts/invalid.vela",
        ] {
            assert_eq!(all(&live, file), json!([]), "repaired {file}");
        }
        let mut fresh = LanguageServiceDatabases::new();
        update(&mut fresh, &fixture);
        for file in [
            "scripts/defs.vela",
            "scripts/valid.vela",
            "scripts/invalid.vela",
        ] {
            assert_eq!(
                live.diagnostics_for_document(&uri(file)).diagnostics(),
                fresh.diagnostics_for_document(&uri(file)).diagnostics(),
                "fresh {file}"
            );
        }
    }
}
