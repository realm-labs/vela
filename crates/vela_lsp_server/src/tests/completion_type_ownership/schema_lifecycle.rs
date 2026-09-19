use crate::matrix_fixture::schema_lifecycle_source;
use crate::tests::{TestServer, notification_values, notify};
use lsp_types::notification as n;
use serde_json::{Value, json};
use std::path::Path;

pub(super) fn apply_phase(
    server: &mut TestServer,
    root: &Path,
    diagnostic_file: Option<&str>,
    phase: &Value,
) {
    let path = root.join("schema.json");
    let existed = path.exists();
    let kind = match schema_lifecycle_source(phase) {
        Some(text) => {
            std::fs::write(&path, text).expect("schema state");
            if existed { 2 } else { 1 }
        }
        None => {
            std::fs::remove_file(&path).expect("delete fixture schema");
            3
        }
    };
    let uri = |file: &str| {
        lsp_types::Url::from_file_path(root.join(file))
            .expect("URI")
            .to_string()
    };
    let publications = notification_values(notify::<n::DidChangeWatchedFiles>(
        server,
        json!({"changes":[{"uri":uri("schema.json"),"type":kind}]}),
    ));
    if let Some(file) = diagnostic_file {
        let publication = publications
            .iter()
            .find(|p| {
                p["method"] == "textDocument/publishDiagnostics" && p["params"]["uri"] == uri(file)
            })
            .expect("schema diagnostic publication");
        let errors = publication["params"]["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .iter()
            .filter(|d| d["code"] == "schema::unavailable")
            .collect::<Vec<_>>();
        assert_eq!(
            publication["params"]["diagnostics"]
                .as_array()
                .expect("diagnostics")
                .len(),
            errors.len(),
            "source-only control: {phase}"
        );
        assert_eq!(
            errors.len(),
            usize::from(phase["diagnosticKind"].is_string()),
            "{phase}"
        );
        if let Some(error) = errors.first() {
            assert_eq!(error["severity"], 2);
            assert_eq!(
                error["range"],
                json!({"start":{"line":0,"character":0},"end":{"line":0,"character":0}})
            );
            let message = error["message"].as_str().expect("message");
            let kind = phase["diagnosticKind"].as_str().expect("kind");
            assert!(
                message.starts_with("host schema `")
                    && message.contains(&format!("schema.json` is {kind}"))
                    && message.ends_with("host facts degrade to Any"),
                "{message}"
            );
        }
    }
}
