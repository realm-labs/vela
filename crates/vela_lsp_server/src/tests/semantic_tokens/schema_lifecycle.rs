use std::{collections::BTreeMap, fs};

use lsp_types::notification as n;
use serde_json::{Value, json};
use vela_language_service::DocumentId;

use super::lifecycle_support::Driver;
use crate::matrix_fixture::schema_artifact;
use crate::tests::{notification_values, notify};

#[test]
fn schema_tokens_drop_replaced_or_unavailable_facts_and_restore_exact_streams() {
    for crlf in [false, true] {
        let mut driver = Driver::new("semantic-token-schema-lifecycle", crlf);
        let states = driver.spec.oracle["states"]
            .as_array()
            .expect("states")
            .clone();
        let mut originals = BTreeMap::new();
        let mut previous = BTreeMap::new();
        for (index, state) in states.iter().enumerate() {
            eprintln!("schema token phase {} CRLF={crlf}", state["id"]);
            let messages = if state["action"]["op"] == "schema" {
                replace_schema(&mut driver, &state["action"])
            } else {
                driver.apply(state)
            };
            assert_schema_diagnostics(&driver, state, &messages);
            driver.assert_sources(state);
            previous = driver.check(&state["queries"], &previous);
            if index == 0 {
                originals.insert("missing", previous.clone());
            } else if state["id"] == "install-schema" {
                originals.insert("schema", previous.clone());
            }
            for original in originals.values() {
                for (file, base) in original {
                    driver.check_delta(file, base, &previous[file]);
                }
            }
            if let Some(baseline) = state["restores"].as_str() {
                assert_eq!(
                    previous, originals[baseline],
                    "{} restores {baseline}",
                    state["id"]
                );
            }
        }
        assert_eq!(
            previous, originals["schema"],
            "complete repair and close restore IDs"
        );
    }
}

fn replace_schema(driver: &mut Driver, action: &Value) -> Vec<Value> {
    let path = driver.root.join("schema.json");
    let existed = path.exists();
    let change = if action["missing"] == true {
        fs::remove_file(&path).expect("delete own schema");
        3
    } else {
        let content = if let Some(text) = action["text"].as_str() {
            text.to_owned()
        } else if !action["schema"].is_null() {
            action["schema"].to_string()
        } else {
            let snapshot = driver.live.snapshot();
            schema_artifact(&action["facts"], &driver.fixture, |file| {
                let source = snapshot.databases().source_db().records()
                    [&DocumentId::from(driver.uri(file))]
                    .source_id()
                    .get();
                assert!(source > 0, "real closed anchor uses a nonzero SourceId");
                source
            })
            .to_string()
        };
        fs::write(&path, content).expect("replace own schema");
        if existed { 2 } else { 1 }
    };
    notification_values(notify::<n::DidChangeWatchedFiles>(
        &mut driver.live,
        json!({"changes":[{"uri":lsp_types::Url::from_file_path(path).expect("schema URI"),"type":change}]}),
    ))
}

fn assert_schema_diagnostics(driver: &Driver, state: &Value, messages: &[Value]) {
    for message in messages {
        assert!(message.get("error").is_none(), "{}: {message}", state["id"]);
    }
    let publications: Vec<_> = messages
        .iter()
        .filter(|message| {
            message["method"] == "textDocument/publishDiagnostics"
                && message["params"]["uri"] == driver.uri("scripts/main.vela")
        })
        .collect();
    let opened = driver.fixture.open.contains_key("scripts/main.vela");
    let closing = state["action"]["op"] == "close";
    assert_eq!(
        publications.len(),
        usize::from(opened || closing),
        "{} publications: {messages:?}",
        state["id"]
    );
    if let Some(publication) = publications.first() {
        let diagnostics = publication["params"]["diagnostics"]
            .as_array()
            .expect("diagnostics");
        if closing {
            assert!(diagnostics.is_empty(), "close clears diagnostics");
        } else {
            let schema_errors: Vec<_> = diagnostics
                .iter()
                .filter(|value| value["code"] == "schema::unavailable")
                .collect();
            if let Some(message) = state["schemaDiagnostic"].as_str() {
                assert_eq!(schema_errors.len(), 1, "{}: {diagnostics:?}", state["id"]);
                assert!(
                    schema_errors[0]["message"]
                        .as_str()
                        .expect("message")
                        .contains(message),
                    "{}: {schema_errors:?}",
                    state["id"]
                );
            } else {
                assert!(
                    schema_errors.is_empty(),
                    "{}: {schema_errors:?}",
                    state["id"]
                );
            }
        }
    }
}
