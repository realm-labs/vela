use super::{TestServer, notify, request, response_value};
use crate::matrix_fixture::{Document, document_symbols as oracle, parse_markers};
use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};

#[test]
fn lsp_document_symbol_recovery_pins_whole_utf16_trees_after_damage_repair_and_repeat() {
    for crlf in [false, true] {
        let (fixture, authored) = oracle::recovery(crlf);
        let owner = super::support::unique_temp_root("中文 % symbol recovery");
        let root = owner.join("workspace");
        fixture.materialize(&root).expect("owned fixture");
        let file = authored["file"].as_str().expect("file");
        let uri = lsp_types::Url::from_file_path(root.join(file))
            .expect("URI")
            .to_string();
        let mut server = TestServer::new();
        let _ = response_value(request::<r::Initialize>(
            &mut server,
            1,
            json!({"processId":null,"rootUri":lsp_types::Url::from_file_path(root.join("scripts")).expect("root URI"),"capabilities":{"textDocument":{"documentSymbol":{"hierarchicalDocumentSymbolSupport":true}}}}),
        ));
        let _ = notify::<n::DidOpenTextDocument>(
            &mut server,
            json!({"textDocument":{"uri":uri,"languageId":"vela","version":1,"text":fixture.disk[file].text}}),
        );
        let mut version = 1;
        let mut request_id = 2;
        for case in authored["cases"].as_array().expect("cases") {
            let damaged = parse_markers(case["source"].as_str().expect("source")).expect("markers");
            for (doc, rows, parse_error) in [
                (
                    &damaged,
                    &case["symbols"],
                    case["parseError"].as_bool().expect("parse policy"),
                ),
                (&fixture.disk[file], &authored["symbols"], false),
                (
                    &damaged,
                    &case["symbols"],
                    case["parseError"].as_bool().expect("parse policy"),
                ),
            ] {
                version += 1;
                let _ = notify::<n::DidChangeTextDocument>(
                    &mut server,
                    json!({"textDocument":{"uri":uri,"version":version},"contentChanges":[{"text":doc.text}]}),
                );
                check(
                    &mut server,
                    &uri,
                    doc,
                    rows,
                    parse_error,
                    case["id"].as_str().expect("case"),
                    &mut request_id,
                );
                // A newly initialized protocol coordinator receives the same
                // disk and overlay; both are checked against authored facts.
                let mut fresh = TestServer::new();
                let _ = response_value(request::<r::Initialize>(
                    &mut fresh,
                    1,
                    json!({"processId":null,"rootUri":lsp_types::Url::from_file_path(root.join("scripts")).expect("root URI"),"capabilities":{}}),
                ));
                let _ = notify::<n::DidOpenTextDocument>(
                    &mut fresh,
                    json!({"textDocument":{"uri":uri,"languageId":"vela","version":version,"text":doc.text}}),
                );
                check(
                    &mut fresh,
                    &uri,
                    doc,
                    rows,
                    parse_error,
                    case["id"].as_str().expect("case"),
                    &mut request_id,
                );
            }
        }
        std::fs::remove_dir_all(owner).expect("owned cleanup");
    }
}

fn check(
    server: &mut TestServer,
    uri: &str,
    doc: &Document,
    rows: &Value,
    parse_error: bool,
    id: &str,
    request_id: &mut i32,
) {
    let expected = oracle::expected(doc, rows, true);
    oracle::assert_ancestry(&expected, None);
    for _ in 0..3 {
        *request_id += 1;
        let response = response_value(request::<r::DocumentSymbolRequest>(
            server,
            *request_id,
            json!({"textDocument":{"uri":uri}}),
        ));
        assert_eq!(
            response,
            json!({"jsonrpc":"2.0","id":request_id,"result":expected}),
            "complete/repeat tree {id}"
        );
    }
    assert_eq!(
        server
            .snapshot()
            .databases()
            .parse_db()
            .parse_diagnostics(&vela_language_service::DocumentId::from(uri))
            .expect("parsed")
            .iter()
            .any(|d| d.code.as_deref() == Some("E_PARSE")),
        parse_error,
        "parse policy {id}"
    );
}
