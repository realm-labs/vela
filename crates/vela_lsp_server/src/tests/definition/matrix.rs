use lsp_types::{notification as n, request as r};
use serde_json::json;

use crate::matrix_fixture::{FixtureWorkspace, load, schema_artifact};
use crate::tests::{TestServer, navigation_request, notify, request, response_value};

#[test]
fn navigation_recovery_matrix_preserves_neighbors_and_explicit_incomplete_nulls() {
    assert_navigation_matrix("navigation-recovery");
}

#[test]
fn navigation_type_position_matrix_projects_exact_nested_targets_and_builtin_nulls() {
    assert_navigation_matrix("navigation-type-positions");
}

#[test]
fn navigation_dynamic_matrix_rejects_guessed_members_after_known_any_returns() {
    assert_navigation_matrix("navigation-dynamic");
}

#[test]
fn navigation_declaration_matrix_projects_exact_utf16_targets_and_nulls() {
    assert_navigation_matrix("navigation-declarations");
}

#[test]
fn navigation_member_matrix_projects_exact_source_member_targets_and_nulls() {
    assert_navigation_matrix("navigation-members");
}

#[test]
fn navigation_schema_matrix_projects_exact_source_spans_and_metadata_nulls() {
    assert_navigation_matrix("navigation-schema");
}

#[test]
fn implementation_matrix_rejects_source_schema_dynamic_and_unresolved_targets_for_client_profiles()
{
    for capabilities in [
        json!({}),
        json!({"textDocument":{"implementation":{"dynamicRegistration":true,"linkSupport":true},
            "definition":{"dynamicRegistration":true,"linkSupport":true}}}),
    ] {
        for fixture in [
            "navigation-declarations",
            "navigation-members",
            "navigation-schema",
        ] {
            assert_navigation_matrix_for_client(fixture, &capabilities, true);
        }
    }
}

fn assert_navigation_matrix(fixture_id: &str) {
    assert_navigation_matrix_for_client(fixture_id, &json!({}), false);
}

fn assert_navigation_matrix_for_client(
    fixture_id: &str,
    capabilities: &serde_json::Value,
    reject_implementation: bool,
) {
    for crlf in [false, true] {
        let mut spec = load(fixture_id);
        if crlf {
            for source in spec.files.values_mut() {
                *source = source.replace('\n', "\r\n");
            }
        }
        let fixture = FixtureWorkspace::new(&spec).expect("navigation fixture");
        let temp = crate::tests::support::unique_temp_root("navigation-matrix");
        let root = temp.join("中文 workspace");
        fixture.materialize(&root).expect("isolated workspace");
        let uri = |file: &str| {
            lsp_types::Url::from_file_path(root.join(file))
                .expect("URI")
                .to_string()
        };
        let mut server = TestServer::new();
        let workspace_root = if fixture.disk.contains_key("vela.toml") {
            ""
        } else {
            "scripts"
        };
        let initialized = response_value(request::<r::Initialize>(
            &mut server,
            1,
            json!({
                "processId":null,"rootUri":uri(workspace_root),"capabilities":capabilities
            }),
        ));
        assert!(initialized["result"]["capabilities"]["implementationProvider"].is_null());
        for (file, document) in &fixture.disk {
            if !file.ends_with(".vela") {
                continue;
            }
            let _ = notify::<n::DidOpenTextDocument>(
                &mut server,
                json!({
                    "textDocument":{"uri":uri(file),"languageId":"vela","version":1,"text":document.text}
                }),
            );
        }
        if let Some(facts) = spec.oracle.get("schema") {
            let snapshot = server.snapshot();
            let artifact = schema_artifact(facts, &fixture, |file| {
                snapshot.databases().source_db().records()
                    [&vela_language_service::DocumentId::from(uri(file))]
                    .source_id()
                    .get()
            });
            std::fs::create_dir_all(root.join("target")).expect("schema directory");
            std::fs::write(root.join("target/schema.json"), artifact.to_string())
                .expect("schema artifact");
            let _ = notify::<n::DidChangeWatchedFiles>(
                &mut server,
                json!({"changes":[{"uri":uri("target/schema.json"),"type":1}]}),
            );
        }
        let mut id = 2;
        if let Some(cases) = spec.oracle["diagnosticCandidates"].as_array() {
            for case in cases {
                let file = case["file"].as_str().expect("file");
                let messages =
                    crate::tests::notification_values(notify::<n::DidChangeTextDocument>(
                        &mut server,
                        json!({"textDocument":{"uri":uri(file),"version":2},
                        "contentChanges":[{"text":fixture.document(file).expect("source").text}]}),
                    ));
                assert!(
                    messages.iter().any(|message| {
                        message["params"]["uri"] == uri(file)
                            && message["params"]["diagnostics"].as_array().is_some_and(
                                |diagnostics| {
                                    diagnostics.iter().any(|diagnostic| {
                                        diagnostic["code"] == case["code"]
                                            && diagnostic["data"]["candidates"]
                                                .as_array()
                                                .is_some_and(|candidates| {
                                                    candidates.iter().any(|candidate| {
                                                        candidate["replacement"]
                                                            == case["replacement"]
                                                    })
                                                })
                                    })
                                },
                            )
                    }),
                    "expected a real diagnostic candidate before rejecting its navigation: {messages:?}"
                );
            }
        }
        if reject_implementation {
            for query in spec.oracle["queries"].as_array().expect("queries") {
                let file = query["file"].as_str().expect("query file");
                let point = fixture.document(file).expect("source").markers
                    [query["cursor"].as_str().expect("cursor")]
                .start;
                let response = response_value(request::<r::GotoImplementation>(
                    &mut server,
                    id,
                    json!({"textDocument":{"uri":uri(file)},
                        "position":{"line":point.line,"character":point.character}}),
                ));
                assert_eq!(
                    response,
                    json!({"jsonrpc":"2.0","id":id,"error":{
                        "code":-32601,"message":"method `textDocument/implementation` is not implemented"
                    }}),
                    "{fixture_id}: {}",
                    query["id"]
                );
                id += 1;
            }
        }
        // The same server must still answer supported requests after rejection.
        assert_queries(
            &mut server,
            &fixture,
            &spec.oracle["queries"],
            &root,
            &mut id,
            &format!("CRLF={crlf}"),
        );
        if let Some(cases) = spec.oracle["invalidSchemaSpans"].as_array() {
            let artifact: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(root.join("target/schema.json")).expect("schema"),
            )
            .expect("artifact");
            let point = fixture
                .document("scripts/main.vela")
                .expect("caller")
                .markers["box-hint"]
                .start;
            let marker = fixture
                .document("scripts/origins.vela")
                .expect("origin")
                .markers["box"];
            let healthy = json!({"uri":uri("scripts/origins.vela"),"range":{
                "start":{"line":marker.start.line,"character":marker.start.character},
                "end":{"line":marker.end.line,"character":marker.end.character}
            }});
            for case in cases {
                let mut invalid = artifact.clone();
                invalid["facts"]["types"][0]["sourceSpan"]
                    .as_object_mut()
                    .expect("span")
                    .extend(case["patch"].as_object().expect("span patch").clone());
                for (content, expected) in
                    [(&invalid, &serde_json::Value::Null), (&artifact, &healthy)]
                {
                    std::fs::write(root.join("target/schema.json"), content.to_string())
                        .expect("replace schema");
                    let _ = notify::<n::DidChangeWatchedFiles>(
                        &mut server,
                        json!({"changes":[{"uri":uri("target/schema.json"),"type":2}]}),
                    );
                    for method in [
                        "textDocument/definition",
                        "textDocument/declaration",
                        "textDocument/typeDefinition",
                    ] {
                        let response = response_value(navigation_request(
                            &mut server,
                            id,
                            method,
                            json!({"textDocument":{"uri":uri("scripts/main.vela")},"position":{"line":point.line,"character":point.character}}),
                        ));
                        id += 1;
                        assert!(response.get("error").is_none(), "{response}");
                        assert_eq!(
                            response.get("result"),
                            Some(expected),
                            "{} {method} CRLF={crlf}",
                            case["id"]
                        );
                    }
                }
            }
        }
        std::fs::remove_dir_all(temp).expect("fixture cleanup");
    }
}

pub(super) fn assert_queries(
    server: &mut TestServer,
    fixture: &FixtureWorkspace,
    queries: &serde_json::Value,
    root: &std::path::Path,
    id: &mut i32,
    context: &str,
) {
    let uri = |file: &str| {
        lsp_types::Url::from_file_path(root.join(file))
            .expect("URI")
            .to_string()
    };
    for query in queries.as_array().expect("matrix") {
        let file = query["file"].as_str().expect("file");
        let document = fixture.document(file).expect("document");
        let point = document.markers[query["cursor"].as_str().expect("cursor")].start;
        for (key, method) in [
            ("definition", "textDocument/definition"),
            ("declaration", "textDocument/declaration"),
            ("type-definition", "textDocument/typeDefinition"),
        ] {
            let response = response_value(navigation_request(
                server,
                *id,
                method,
                json!({
                    "textDocument":{"uri":uri(file)},"position":{"line":point.line,"character":point.character}
                }),
            ));
            *id += 1;
            let expected = query.get(key).expect("explicit method oracle");
            let result = if expected.is_null() {
                serde_json::Value::Null
            } else {
                let target_file = query["target-file"].as_str().unwrap_or(file);
                let target = fixture.document(target_file).expect("target").markers
                    [expected.as_str().expect("target marker or explicit null")];
                json!({"uri":uri(target_file),"range":{
                    "start":{"line":target.start.line,"character":target.start.character},
                    "end":{"line":target.end.line,"character":target.end.character}
                }})
            };
            assert!(response.get("error").is_none(), "{response}");
            assert_eq!(
                response.get("result"),
                Some(&result),
                "{}: {method}, {context}",
                query["id"]
            );
        }
    }
}
