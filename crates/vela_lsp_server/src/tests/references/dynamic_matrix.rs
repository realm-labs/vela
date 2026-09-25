use std::fs;

use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};
use vela_language_service::DocumentId;

use crate::matrix_fixture::{FixtureWorkspace, load, schema_artifact};
use crate::tests::{TestServer, notify, request, response_value};

#[test]
fn dynamic_any_return_matrix_rejects_all_reference_and_rename_targets() {
    for crlf in [false, true] {
        let mut spec = load("navigation-dynamic");
        if crlf {
            for source in spec.files.values_mut() {
                *source = source.replace('\n', "\r\n");
            }
        }
        let fixture = FixtureWorkspace::new(&spec).expect("dynamic fixture");
        let parent = crate::tests::support::unique_temp_root("reference-dynamic");
        let root = parent.join("中文 % dynamic references");
        fixture.materialize(&root).expect("isolated workspace");
        let uri = |file: &str| {
            lsp_types::Url::from_file_path(root.join(file))
                .expect("URI")
                .to_string()
        };
        let mut server = TestServer::new();
        let initialized = response_value(request::<r::Initialize>(
            &mut server,
            1,
            json!({"processId":null,"rootUri":uri(""),"capabilities":{}}),
        ));
        for provider in ["referencesProvider", "documentHighlightProvider"] {
            assert_eq!(initialized["result"]["capabilities"][provider], true);
        }
        assert_eq!(
            initialized["result"]["capabilities"]["renameProvider"]["prepareProvider"],
            true
        );
        for (file, document) in &fixture.disk {
            if file.ends_with(".vela") {
                let _ = notify::<n::DidOpenTextDocument>(
                    &mut server,
                    json!({"textDocument":{"uri":uri(file),"languageId":"vela","version":1,"text":document.text}}),
                );
            }
        }
        let snapshot = server.snapshot();
        let artifact = schema_artifact(&spec.oracle["schema"], &fixture, |file| {
            snapshot.databases().source_db().records()[&DocumentId::from(uri(file))]
                .source_id()
                .get()
        });
        fs::create_dir_all(root.join("target")).expect("schema directory");
        fs::write(root.join("target/schema.json"), artifact.to_string()).expect("schema artifact");
        let _ = notify::<n::DidChangeWatchedFiles>(
            &mut server,
            json!({"changes":[{"uri":uri("target/schema.json"),"type":1}]}),
        );

        let mut id = 2;
        let mut dynamic_count = 0;
        let mut owned_count = 0;
        let queries = spec.oracle["queries"].as_array().expect("query matrix");
        for query in queries {
            let file = query["file"].as_str().expect("file");
            let document = fixture.document(file).expect("document");
            let marker = document.markers[query["cursor"].as_str().expect("cursor")];
            let params = json!({"textDocument":{"uri":uri(file)},
                "position":{"line":marker.start.line,"character":marker.start.character}});
            if query["definition"].is_null() {
                dynamic_count += 1;
                for include in [false, true] {
                    let refs = query_result::<r::References>(
                        &mut server,
                        &mut id,
                        json!({"textDocument":params["textDocument"],"position":params["position"],
                            "context":{"includeDeclaration":include}}),
                    );
                    assert_eq!(refs, json!([]), "{} CRLF={crlf} refs", query["id"]);
                }
                assert_eq!(
                    query_result::<r::DocumentHighlightRequest>(
                        &mut server,
                        &mut id,
                        params.clone()
                    ),
                    json!([]),
                    "{} CRLF={crlf} highlights",
                    query["id"]
                );
                assert!(
                    query_result::<r::PrepareRenameRequest>(&mut server, &mut id, params.clone())
                        .is_null(),
                    "{} CRLF={crlf} prepare",
                    query["id"]
                );
                assert!(
                    query_result::<r::Rename>(
                        &mut server,
                        &mut id,
                        json!({"textDocument":params["textDocument"],"position":params["position"],
                        "newName":"renamed_item"})
                    )
                    .is_null(),
                    "{} CRLF={crlf} rename",
                    query["id"]
                );
            } else {
                owned_count += 1;
                let target = query_result::<r::GotoDefinition>(&mut server, &mut id, params);
                assert!(
                    target.is_object(),
                    "{} CRLF={crlf} known owner",
                    query["id"]
                );
                assert_eq!(
                    target["uri"],
                    uri(query["target-file"].as_str().unwrap_or(file))
                );
            }
        }
        assert_eq!((dynamic_count, owned_count), (12, 7));
        fs::remove_dir_all(parent).expect("remove isolated fixture");
    }
}

fn query_result<R: r::Request>(server: &mut TestServer, id: &mut i32, params: Value) -> Value {
    let response = response_value(request::<R>(server, *id, params));
    *id += 1;
    assert!(response.get("error").is_none(), "{response}");
    response["result"].clone()
}
