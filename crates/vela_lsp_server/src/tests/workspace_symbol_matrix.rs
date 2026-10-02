use super::{TestServer, request, response_value};
use crate::matrix_fixture::workspace_symbols as oracle;
use lsp_types::request as r;
use serde_json::json;

#[test]
fn lsp_workspace_symbol_declarations_pin_complete_repeated_utf16_query_sets() {
    for crlf in [false, true] {
        for shifted in [false, true] {
            let (fixture, authored) = oracle::fixture(crlf, shifted);
            let owner = super::support::unique_temp_root("中文 % workspace symbols");
            let root = owner.join("workspace");
            fixture
                .materialize(&root)
                .expect("private owned source corpus");
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
            assert_eq!(server.snapshot().databases().source_db().records().len(), 4);
            let mut id = 2;
            for query in authored["queries"].as_array().expect("32 authored queries") {
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
                        "crlf={crlf}, shifted={shifted}, query={}, repeat={repeat}",
                        query["id"]
                    );
                    id += 1;
                }
            }
            for (file, doc) in &fixture.disk {
                assert_eq!(
                    std::fs::read_to_string(root.join(file)).expect("physical source"),
                    doc.text
                );
            }
            std::fs::remove_dir_all(&owner).expect("owned cleanup");
        }
    }
}
