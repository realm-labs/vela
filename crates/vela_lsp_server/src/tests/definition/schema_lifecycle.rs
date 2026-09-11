use std::{fs, path::Path};

use lsp_types::{notification as n, request as r};
use serde_json::json;
use vela_language_service::DocumentId;

use super::matrix::assert_queries;
use crate::matrix_fixture::{FixtureWorkspace, lifecycle_facts, load, schema_artifact};
use crate::tests::{TestServer, notification_values, notify, request};

#[test]
fn schema_navigation_lifecycle_clears_stale_targets_and_publishes_controlled_diagnostics() {
    for crlf in [false, true] {
        let mut spec = load("navigation-schema");
        if crlf {
            for source in spec.files.values_mut() {
                *source = source.replace('\n', "\r\n");
            }
        }
        let fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let temp = crate::tests::support::unique_temp_root("schema-navigation-lifecycle");
        let root = temp.join("中文 workspace");
        fixture.materialize(&root).expect("workspace");
        fs::create_dir(root.join("target")).expect("schema directory");
        let mut server = start_server(&root, &fixture);
        let path = root.join("target/schema.json");
        let mut id = 2;
        for step in spec.oracle["schemaLifecycle"].as_array().expect("steps") {
            let existed = path.exists();
            let change = match step["op"].as_str().expect("operation") {
                "delete" => {
                    fs::remove_file(&path).expect("delete schema");
                    3
                }
                "invalid" => {
                    fs::write(&path, "{ broken").expect("invalid schema");
                    2
                }
                "restore" | "replace" => {
                    let snapshot = server.snapshot();
                    let facts = lifecycle_facts(&spec.oracle["schema"], step);
                    let artifact = schema_artifact(&facts, &fixture, |file| {
                        snapshot.databases().source_db().records()
                            [&DocumentId::from(uri(&root, file))]
                            .source_id()
                            .get()
                    });
                    fs::write(&path, artifact.to_string()).expect("schema");
                    if existed { 2 } else { 1 }
                }
                operation => panic!("unsupported operation {operation}"),
            };
            let notifications = notification_values(notify::<n::DidChangeWatchedFiles>(
                &mut server,
                json!({"changes":[{"uri":uri(&root,"target/schema.json"),"type":change}]}),
            ));
            let publication = notifications
                .iter()
                .find(|value| {
                    value["method"] == "textDocument/publishDiagnostics"
                        && value["params"]["uri"] == uri(&root, "scripts/main.vela")
                })
                .expect("caller diagnostics publication");
            let diagnostics = publication["params"]["diagnostics"]
                .as_array()
                .expect("diagnostics");
            let schema_errors = diagnostics
                .iter()
                .filter(|value| value["code"] == "schema::unavailable")
                .collect::<Vec<_>>();
            if let Some(expected) = step["diagnostic"].as_str() {
                assert_eq!(schema_errors.len(), 1, "{}", step["id"]);
                assert!(
                    schema_errors[0]["message"]
                        .as_str()
                        .expect("message")
                        .contains(expected)
                );
            } else {
                assert!(
                    schema_errors.is_empty(),
                    "{}: {schema_errors:?}",
                    step["id"]
                );
            }
            let mut fresh = start_server(&root, &fixture);
            let mut fresh_id = 2;
            for _ in 0..2 {
                let context = format!("{} CRLF={crlf}", step["id"]);
                assert_queries(
                    &mut server,
                    &fixture,
                    &step["queries"],
                    &root,
                    &mut id,
                    &format!("incremental {context}"),
                );
                assert_queries(
                    &mut fresh,
                    &fixture,
                    &step["queries"],
                    &root,
                    &mut fresh_id,
                    &format!("fresh {context}"),
                );
            }
        }
        fs::remove_dir_all(temp).expect("cleanup");
    }
}

fn uri(root: &Path, file: &str) -> String {
    lsp_types::Url::from_file_path(root.join(file))
        .expect("URI")
        .to_string()
}

fn start_server(root: &Path, fixture: &FixtureWorkspace) -> TestServer {
    let mut server = TestServer::new();
    let _ = request::<r::Initialize>(
        &mut server,
        1,
        json!({"processId":null,"rootUri":uri(root,""),"capabilities":{}}),
    );
    for (file, document) in &fixture.disk {
        if file.ends_with(".vela") {
            let _ = notify::<n::DidOpenTextDocument>(
                &mut server,
                json!({"textDocument":{"uri":uri(root,file),"languageId":"vela","version":1,"text":document.text}}),
            );
        }
    }
    server
}
