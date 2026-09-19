use super::Layout;
use crate::LanguageServiceDatabases;
use crate::matrix_fixture::schema_lifecycle_source;
use serde_json::Value;

pub(super) fn apply_phase(db: &mut LanguageServiceDatabases, phase: &Value) {
    let path = "/workspace/schema.json";
    match schema_lifecycle_source(phase) {
        Some(text) => db.load_schema_artifact_json(path, &text),
        None => db.mark_schema_missing(path),
    }
    let diagnostics = db.schema_db().diagnostics();
    if let Some(kind) = phase["diagnosticKind"].as_str() {
        assert_eq!(diagnostics.len(), 1, "{phase}");
        let message = diagnostics[0].message();
        assert!(
            message.starts_with(&format!("host schema `{path}` is {kind}")),
            "{message}"
        );
        assert!(message.ends_with("host facts degrade to Any"), "{message}");
    } else {
        assert!(diagnostics.is_empty(), "{phase}: {diagnostics:?}");
    }
}

pub(super) fn assert_diagnostics(
    db: &LanguageServiceDatabases,
    layout: &Layout,
    file: &str,
    phase: &Value,
) {
    let result = db.diagnostics_for_document(&layout.uri(file));
    let errors = result
        .diagnostics()
        .iter()
        .filter(|d| d.code() == Some("schema::unavailable"))
        .collect::<Vec<_>>();
    assert_eq!(
        result.diagnostics().len(),
        errors.len(),
        "source-only control: {phase}"
    );
    assert_eq!(
        errors.len(),
        usize::from(phase["diagnosticKind"].is_string()),
        "{phase}"
    );
    if let Some(error) = errors.first() {
        assert_eq!(error.severity(), crate::ServiceDiagnosticSeverity::Warning);
        assert!(error.range().is_none());
        assert_eq!(error.message(), db.schema_db().diagnostics()[0].message());
    }
}
