use super::{TestServer, notify, request, response_value};
use crate::matrix_fixture::{document_symbols as oracle, schema_artifact};
use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};

#[test]
fn lsp_document_symbol_members_and_imports_pin_whole_utf16_source_trees_with_schema_collisions() {
    for crlf in [false, true] {
        let (fixture, authored) = oracle::ownership(crlf);
        let owner = super::support::unique_temp_root("中文 % outline ownership");
        let root = owner.join("workspace");
        fixture.materialize(&root).expect("owned root");
        let uri = |file: &str| {
            lsp_types::Url::from_file_path(root.join(file))
                .expect("encoded URI")
                .to_string()
        };
        assert!(uri("").contains("%25") && uri("").contains("%20"));
        let mut server = TestServer::new();
        let _ = response_value(request::<r::Initialize>(
            &mut server,
            1,
            json!({"processId":null,"rootUri":uri(""),"capabilities":{"textDocument":{"documentSymbol":{"hierarchicalDocumentSymbolSupport":true}}}}),
        ));
        let snapshot = server.snapshot();
        assert_eq!(
            snapshot.databases().source_db().records().len(),
            8,
            "all configured source files discovered"
        );
        let artifact = schema_artifact(&authored["schema"], &fixture, |file| {
            snapshot.databases().source_db().records()
                [&vela_language_service::DocumentId::from(uri(file))]
                .source_id()
                .get()
        });
        std::fs::write(root.join("schema.json"), artifact.to_string()).expect("bound schema");
        let _ = notify::<n::DidChangeWatchedFiles>(
            &mut server,
            json!({"changes":[{"uri":uri("schema.json"),"type":2}]}),
        );
        assert!(
            server
                .snapshot()
                .databases()
                .schema_db()
                .diagnostics()
                .is_empty(),
            "valid schema"
        );
        let loaded = server.snapshot();
        let facts = loaded.databases().schema_db().facts();
        assert_eq!(facts.types().count(), 4);
        assert_eq!(facts.fields().count(), 5);
        assert!(
            facts
                .fields()
                .all(|field| field.fact.display_name() == "String"),
            "loaded string facts, not unknown primitive spellings"
        );
        assert_eq!(facts.methods().count(), 3);
        assert_eq!(facts.trait_methods().count(), 1);
        assert_eq!(facts.functions().count(), 2);
        assert_eq!(facts.variants().count(), 3);
        let locations = loaded.databases().schema_db().source_locations();
        assert!(locations.type_span("host::Box").is_some());
        assert!(locations.trait_span("host::Readable").is_some());
        assert!(locations.function_span("host::make").is_some());
        assert!(locations.field_span("host::Box", "value").is_some());
        assert!(locations.method_span("host::Box", "read").is_some());
        assert!(
            locations
                .trait_method_span("host::Readable", "read")
                .is_some()
        );
        assert!(locations.variant_span("host::Choice", "Pair").is_some());
        assert!(locations.type_span("metadata::Box").is_none());
        let mut id = 2;
        let mut nodes = 0;
        for (file, rows) in authored["trees"].as_object().expect("file trees") {
            let doc = &fixture.disk[file];
            let expected = oracle::expected(doc, rows, true);
            nodes += oracle::assert_ancestry(&expected, None);
            for open in [false, true] {
                if open {
                    let _ = notify::<n::DidOpenTextDocument>(
                        &mut server,
                        json!({"textDocument":{"uri":uri(file),"languageId":"vela","version":1,"text":doc.text}}),
                    );
                }
                for _ in 0..3 {
                    id += 1;
                    let response = response_value(request::<r::DocumentSymbolRequest>(
                        &mut server,
                        id,
                        json!({"textDocument":{"uri":uri(file)}}),
                    ));
                    assert_eq!(
                        response,
                        json!({"jsonrpc":"2.0","id":id,"result":expected}),
                        "whole owned tree {file} crlf={crlf} open={open}"
                    );
                }
            }
        }
        assert_eq!(nodes, 46);
        for file in ["scripts/missing.vela", "schema.json"] {
            id += 1;
            let response = response_value(request::<r::DocumentSymbolRequest>(
                &mut server,
                id,
                json!({"textDocument":{"uri":uri(file)}}),
            ));
            assert_eq!(
                response["result"],
                Value::Array(Vec::new()),
                "no synthetic source {file}"
            );
        }
        std::fs::remove_dir_all(owner).expect("owned cleanup");
    }
}
