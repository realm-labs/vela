use super::{TestServer, n, oracle, sync_diagnostics, uri};
use crate::matrix_fixture::Document;
use serde_json::{Value, json};

pub(super) fn assert_query(
    server: &mut TestServer,
    layout: &oracle::Layout,
    case: &Value,
    source: &Document,
) {
    let snapshot = server.snapshot();
    let id =
        vela_language_service::DocumentId::from(uri(layout, case["file"].as_str().expect("file")));
    if let Some(expected) = case["parseErrors"].as_bool() {
        assert_eq!(
            !snapshot
                .databases()
                .parse_db()
                .parse_diagnostics(&id)
                .expect("parsed query")
                .is_empty(),
            expected,
            "{} parser recovery",
            case["id"]
        );
    }
    if let Some(candidate) = case["diagnosticCandidate"].as_object() {
        // This query file is closed in the authored workspace. Open it only to
        // inspect its real publication, then close back to the same disk facts.
        let publication = sync_diagnostics::<n::DidOpenTextDocument>(
            server,
            json!({"textDocument":{
                "uri":id.as_str(),"languageId":"vela","version":1,"text":source.text
            }}),
        );
        assert!(
            publication["params"]["diagnostics"]
                .as_array()
                .expect("diagnostics")
                .iter()
                .any(|d| {
                    d["code"] == candidate["code"]
                        && d["data"]["candidates"]
                            .as_array()
                            .expect("candidates")
                            .iter()
                            .any(|c| c["replacement"] == candidate["replacement"])
                }),
            "{} known repair candidate: {publication}",
            case["id"]
        );
        let _ = sync_diagnostics::<n::DidCloseTextDocument>(
            server,
            json!({"textDocument":{"uri":id.as_str()}}),
        );
    }
}

pub(super) fn assert_schema(server: &TestServer, phase: &Value) {
    let Some(expected) = phase["schemaStatus"].as_str() else {
        return;
    };
    let snapshot = server.snapshot();
    let diagnostics = snapshot.databases().schema_db().diagnostics();
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
