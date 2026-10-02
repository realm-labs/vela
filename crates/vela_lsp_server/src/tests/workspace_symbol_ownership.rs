use super::{TestServer, notify, request, response_value};
use crate::matrix_fixture::{schema_artifact, workspace_symbols as oracle};
use lsp_types::{notification as n, request as r};
use serde_json::json;

#[test]
fn lsp_workspace_symbol_members_and_imports_pin_complete_utf16_source_and_metadata_query_sets() {
    for crlf in [false, true] {
        for shifted in [false, true] {
            let (fixture, authored) = oracle::ownership(crlf, shifted);
            let owner = super::support::unique_temp_root("中文 % workspace ownership");
            let root = owner.join("workspace");
            fixture.materialize(&root).expect("private owned corpus");
            let uri = |file: &str| {
                lsp_types::Url::from_file_path(root.join(file))
                    .expect("URI")
                    .to_string()
            };
            for encoding in ["%E4%B8%AD", "%25", "%20"] {
                assert!(uri("").contains(encoding));
            }
            let mut server = TestServer::new();
            let _ = response_value(request::<r::Initialize>(
                &mut server,
                1,
                json!({"processId":null,"rootUri":uri("scripts"),"capabilities":{}}),
            ));
            let _ = notify::<n::DidChangeConfiguration>(
                &mut server,
                json!({"settings":{"vela":{"host":{"schema":uri("schema.json")}}}}),
            );
            let snapshot = server.snapshot();
            assert_eq!(snapshot.databases().source_db().records().len(), 8);
            let artifact = schema_artifact(&authored["schema"], &fixture, |file| {
                snapshot.databases().source_db().records()
                    [&vela_language_service::DocumentId::from(uri(file))]
                    .source_id()
                    .get()
            });
            let schema_text = artifact.to_string();
            std::fs::write(root.join("schema.json"), &schema_text).expect("bound static schema");
            let _ = notify::<n::DidChangeWatchedFiles>(
                &mut server,
                json!({"changes":[{"uri":uri("schema.json"),"type":2}]}),
            );
            let loaded = server.snapshot();
            let db = loaded.databases();
            assert!(
                db.schema_db().diagnostics().is_empty(),
                "valid static schema"
            );
            let locations = db.schema_db().source_locations();
            assert!(locations.type_span("host::Choice").is_some());
            assert!(locations.trait_span("host::Readable").is_some());
            assert!(locations.function_span("host::make").is_some());
            assert!(locations.method_span("host::Box", "read").is_some());
            assert!(
                locations
                    .trait_method_span("host::Readable", "read")
                    .is_some()
            );
            for variant in ["Empty", "Pair", "Named"] {
                assert!(locations.variant_span("host::Choice", variant).is_some());
            }
            assert!(locations.field_span("metadata::Box", "value").is_none());
            assert!(locations.method_span("metadata::Box", "read").is_none());
            assert!(
                db.schema_db()
                    .source_locations()
                    .type_span("host::Box")
                    .is_some()
            );
            assert!(
                db.schema_db()
                    .source_locations()
                    .field_span("host::Box", "value")
                    .is_some()
            );
            assert!(
                db.schema_db()
                    .source_locations()
                    .type_span("metadata::Box")
                    .is_none()
            );
            let mut id = 2;
            for query in authored["queries"].as_array().expect("49 authored queries") {
                let wanted = oracle::expected(&fixture, &authored, &query["symbols"], true, &uri);
                for repeat in 0..3 {
                    let response = response_value(request::<r::WorkspaceSymbolRequest>(
                        &mut server,
                        id,
                        json!({"query":query["query"]}),
                    ));
                    assert_eq!(
                        response,
                        json!({"jsonrpc":"2.0","id":id,"result":wanted}),
                        "crlf={crlf},shifted={shifted},query={},repeat={repeat}",
                        query["id"]
                    );
                    id += 1;
                }
            }
            for (file, doc) in &fixture.disk {
                if file.ends_with(".vela") {
                    assert_eq!(
                        std::fs::read_to_string(root.join(file)).expect("physical source"),
                        doc.text
                    );
                }
            }
            assert_eq!(
                std::fs::read_to_string(root.join("schema.json")).expect("physical schema"),
                schema_text
            );
            std::fs::remove_dir_all(&owner).expect("owned cleanup");
        }
    }
}
