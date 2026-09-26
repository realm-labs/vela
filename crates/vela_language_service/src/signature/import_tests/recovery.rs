use crate::{DocumentId, LanguageServiceDatabases};
use serde_json::Value;

pub(super) fn assert_query(db: &LanguageServiceDatabases, id: &DocumentId, case: &Value) {
    if let Some(expected) = case["parseErrors"].as_bool() {
        assert_eq!(
            !db.parse_db()
                .parse_diagnostics(id)
                .expect("parsed query")
                .is_empty(),
            expected,
            "{} parser recovery",
            case["id"]
        );
    }
    if let Some(candidate) = case["diagnosticCandidate"].as_object() {
        let diagnostics = db.diagnostics_for_document(id);
        assert!(
            diagnostics.diagnostics().iter().any(|d| {
                d.code() == candidate["code"].as_str()
                    && d.candidates()
                        .iter()
                        .any(|c| Some(c.replacement()) == candidate["replacement"].as_str())
            }),
            "{} known repair candidate: {diagnostics:?}",
            case["id"]
        );
    }
}

pub(super) fn assert_schema(db: &LanguageServiceDatabases, phase: &Value) {
    let Some(expected) = phase["schemaStatus"].as_str() else {
        return;
    };
    let diagnostics = db.schema_db().diagnostics();
    if expected == "valid" {
        assert!(diagnostics.is_empty(), "{} valid schema", phase["id"]);
    } else {
        assert_eq!(diagnostics.len(), 1, "{} unavailable schema", phase["id"]);
        let message = diagnostics[0].message();
        let category = if expected == "missing" {
            "unavailable"
        } else {
            "invalid"
        };
        assert!(message.contains(&format!(" is {category}")), "{message}");
        assert!(message.ends_with("host facts degrade to Any"), "{message}");
    }
}
