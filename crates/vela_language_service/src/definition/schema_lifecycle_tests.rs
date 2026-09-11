use crate::LanguageServiceDatabases;
use crate::matrix_fixture::{FixtureWorkspace, lifecycle_facts, load, schema_artifact};

use super::matrix_tests::{assert_queries, fixture_databases, uri};

#[test]
fn schema_navigation_lifecycle_replaces_removes_recovers_and_matches_fresh() {
    for crlf in [false, true] {
        let mut spec = load("navigation-schema");
        if crlf {
            for source in spec.files.values_mut() {
                *source = source.replace('\n', "\r\n");
            }
        }
        let fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let mut databases = fixture_databases(&fixture);
        for step in spec.oracle["schemaLifecycle"]
            .as_array()
            .expect("lifecycle")
        {
            let facts = lifecycle_facts(&spec.oracle["schema"], step);
            let mut fresh = fixture_databases(&fixture);
            for database in [&mut databases, &mut fresh] {
                apply_schema_step(database, &fixture, step, &facts);
                let diagnostics = database.schema_db().diagnostics();
                if let Some(expected) = step["diagnostic"].as_str() {
                    assert_eq!(diagnostics.len(), 1);
                    assert!(diagnostics[0].message().contains(expected));
                } else {
                    assert!(diagnostics.is_empty());
                }
                for _ in 0..2 {
                    assert_queries(
                        database,
                        &fixture,
                        &step["queries"],
                        &format!("{} CRLF={crlf}", step["id"]),
                    );
                }
            }
        }
    }
}

fn apply_schema_step(
    database: &mut LanguageServiceDatabases,
    fixture: &FixtureWorkspace,
    step: &serde_json::Value,
    facts: &serde_json::Value,
) {
    let path = "/workspace/target/schema.json";
    match step["op"].as_str().expect("operation") {
        "delete" => database.mark_schema_missing(path),
        "invalid" => database.load_schema_artifact_json(path, "{ broken"),
        "restore" | "replace" => {
            let artifact = schema_artifact(facts, fixture, |file| {
                database.source_db().records()[&uri(file)].source_id().get()
            });
            database.load_schema_artifact_json(path, &artifact.to_string());
        }
        operation => panic!("unsupported operation {operation}"),
    }
}
