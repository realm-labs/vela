use super::{TestServer, notify, request, response_value};
use crate::matrix_fixture::document_symbols as oracle;
use lsp_types::{notification as n, request as r};
use serde_json::json;

#[test]
fn lsp_document_symbol_declarations_have_complete_utf16_ranges_and_encoded_uri_ownership() {
    for crlf in [false, true] {
        let (fixture, authored) = oracle::fixture(crlf);
        let owner = super::support::unique_temp_root("中文 % document symbols");
        let root = owner.join("workspace");
        fixture.materialize(&root).expect("owned fixture");
        let uri = |file: &str| {
            lsp_types::Url::from_file_path(root.join(file))
                .expect("URI")
                .to_string()
        };
        assert!(uri("").contains("%E4%B8%AD"));
        assert!(uri("").contains("%25"));
        assert!(uri("").contains("%20"));
        let mut server = TestServer::new();
        let _ = response_value(request::<r::Initialize>(
            &mut server,
            1,
            json!({"processId":null,"rootUri":uri("scripts"),"capabilities":{"textDocument":{"documentSymbol":{"hierarchicalDocumentSymbolSupport":true}}}}),
        ));
        let file = authored["file"].as_str().expect("file");
        let expected = oracle::expected(&fixture.disk[file], &authored["symbols"], true);
        assert_eq!(oracle::assert_ancestry(&expected, None), 33);
        let disk = response_value(request::<r::DocumentSymbolRequest>(
            &mut server,
            2,
            json!({"textDocument":{"uri":uri(file)}}),
        ));
        assert_eq!(disk, json!({"jsonrpc":"2.0","id":2,"result":expected}));
        let disk_keys = server
            .snapshot()
            .databases()
            .source_db()
            .records()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(disk_keys.len(), 3);
        let _ = notify::<n::DidOpenTextDocument>(
            &mut server,
            json!({"textDocument":{"uri":uri(file),"languageId":"vela","version":1,"text":fixture.disk[file].text}}),
        );
        assert_eq!(
            server
                .snapshot()
                .databases()
                .source_db()
                .records()
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
            disk_keys
        );
        let response = response_value(request::<r::DocumentSymbolRequest>(
            &mut server,
            3,
            json!({"textDocument":{"uri":uri(file)}}),
        ));
        assert_eq!(
            response,
            json!({"jsonrpc":"2.0","id":3,"result":expected}),
            "whole wire tree crlf={crlf}"
        );
        let empty = response_value(request::<r::DocumentSymbolRequest>(
            &mut server,
            4,
            json!({"textDocument":{"uri":uri("scripts/imports.vela")}}),
        ));
        assert_eq!(empty, json!({"jsonrpc":"2.0","id":4,"result":[]}));
        std::fs::remove_dir_all(&owner).expect("owned cleanup");
    }
}
