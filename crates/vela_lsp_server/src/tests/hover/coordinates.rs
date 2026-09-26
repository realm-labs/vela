use crate::matrix_fixture::{FixtureWorkspace, hover_signature as oracle};
use crate::tests::{TestServer, notify, request, response_value, sync_diagnostics};
use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};
use std::{fs, path::Path};

fn uri(root: &Path, file: &str) -> String {
    lsp_types::Url::from_file_path(root.join(file))
        .expect("encoded URI")
        .to_string()
}

fn hover(server: &mut TestServer, uri: &str, position: Value) -> Value {
    let response = response_value(request::<r::HoverRequest>(
        server,
        2,
        json!({"textDocument":{"uri":uri},"position":position}),
    ));
    assert!(response.get("error").is_none(), "{response}");
    assert!(
        response.get("result").is_some(),
        "JSON null must be explicit"
    );
    response["result"].clone()
}

fn signature(server: &mut TestServer, uri: &str, position: Value) -> Value {
    let response = response_value(request::<r::SignatureHelpRequest>(
        server,
        3,
        json!({"textDocument":{"uri":uri},"position":position}),
    ));
    assert!(response.get("error").is_none(), "{response}");
    assert!(
        response.get("result").is_some(),
        "JSON null must be explicit"
    );
    response["result"].clone()
}

#[test]
fn hover_signature_coordinates_preserve_complete_unicode_results_and_repeats() {
    for crlf in [false, true] {
        let disk = oracle::spec(crlf, false);
        let fixture = FixtureWorkspace::new(&disk).expect("disk fixture");
        let parent = crate::tests::support::unique_temp_root("hover-signature-coordinates");
        let root = parent.join("中文 % hover");
        fixture.materialize(&root).expect("isolated source");
        let main_uri = uri(&root, "scripts/main.vela");
        assert!(main_uri.contains("%25"));
        assert!(main_uri.contains("%E4"));
        let mut server = TestServer::new();
        let _ = response_value(request::<r::Initialize>(
            &mut server,
            1,
            json!({"processId":null,"rootUri":uri(&root,""),"capabilities":{}}),
        ));
        let mut original = None;
        for (phase, shifted) in [false, true, false].into_iter().enumerate() {
            let spec = oracle::spec(crlf, shifted);
            let current = FixtureWorkspace::new(&spec).expect("current fixture");
            let main = &current.disk["scripts/main.vela"];
            let version = phase + 1;
            if phase == 0 {
                let _ = sync_diagnostics::<n::DidOpenTextDocument>(
                    &mut server,
                    json!({"textDocument":{"uri":main_uri,"languageId":"vela","version":version,"text":main.text}}),
                );
            } else {
                let _ = sync_diagnostics::<n::DidChangeTextDocument>(
                    &mut server,
                    json!({"textDocument":{"uri":main_uri,"version":version},"contentChanges":[{"text":main.text}]}),
                );
            }
            let mut observed = Vec::new();
            for query in spec.oracle["hover"].as_array().expect("hover queries") {
                let file = query["file"].as_str().expect("file");
                let document = &current.disk[file];
                let marker = query["marker"].as_str().expect("marker");
                let file_uri = uri(&root, file);
                for offset in 0..if query["result"].is_null() { 1 } else { 2 } {
                    let position = oracle::position(document, marker, true, offset);
                    let actual = hover(&mut server, &file_uri, position.clone());
                    assert_eq!(
                        actual,
                        oracle::hover_result(document, query, true),
                        "{file}/{marker}, CRLF={crlf}, shifted={shifted}"
                    );
                    assert_eq!(
                        hover(&mut server, &file_uri, position),
                        actual,
                        "repeat hover"
                    );
                    observed.push(actual);
                }
            }
            for query in spec.oracle["signatures"]
                .as_array()
                .expect("signature queries")
            {
                let point =
                    oracle::position(main, query["marker"].as_str().expect("marker"), true, 0);
                let actual = signature(&mut server, &main_uri, point.clone());
                assert_eq!(
                    actual,
                    oracle::signature_result(query, true),
                    "{}, CRLF={crlf}, shifted={shifted}",
                    query["marker"]
                );
                assert_eq!(
                    signature(&mut server, &main_uri, point),
                    actual,
                    "repeat signature"
                );
                observed.push(actual);
            }
            // The Chinese character occupies one UTF-16 unit; the following
            // non-BMP character occupies two. Reject its interior, not the whole
            // comment, then prove every subsequent valid query still agrees.
            let unicode = main.markers["unicode"].start;
            for bad in [
                json!({"line":unicode.line,"character":unicode.character+2}),
                json!({"line":unicode.line,"character":100000}),
            ] {
                let params = json!({"textDocument":{"uri":main_uri},"position":bad});
                for response in [
                    response_value(request::<r::HoverRequest>(&mut server, 4, params.clone())),
                    response_value(request::<r::SignatureHelpRequest>(&mut server, 5, params)),
                ] {
                    assert_eq!(response["error"]["code"], -32600, "{response}");
                    assert!(
                        response.get("result").is_none(),
                        "invalid position is an error"
                    );
                }
            }
            for query in spec.oracle["hover"].as_array().expect("hover queries") {
                let file = query["file"].as_str().expect("file");
                let document = &current.disk[file];
                assert_eq!(
                    hover(
                        &mut server,
                        &uri(&root, file),
                        oracle::position(
                            document,
                            query["marker"].as_str().expect("marker"),
                            true,
                            0
                        )
                    ),
                    oracle::hover_result(document, query, true),
                    "hover after malformed positions"
                );
            }
            for query in spec.oracle["signatures"]
                .as_array()
                .expect("signature queries")
            {
                assert_eq!(
                    signature(
                        &mut server,
                        &main_uri,
                        oracle::position(main, query["marker"].as_str().expect("marker"), true, 0)
                    ),
                    oracle::signature_result(query, true),
                    "signature after malformed positions"
                );
            }
            assert_eq!(
                fs::read_to_string(root.join("scripts/main.vela")).expect("physical disk"),
                fixture.disk["scripts/main.vela"].text
            );
            if !shifted {
                if let Some(original) = &original {
                    assert_eq!(&observed, original, "exact restoration");
                } else {
                    original = Some(observed);
                }
            }
        }
        let _ = notify::<n::DidCloseTextDocument>(
            &mut server,
            json!({"textDocument":{"uri":main_uri}}),
        );
        let main = &fixture.disk["scripts/main.vela"];
        for query in disk.oracle["hover"]
            .as_array()
            .expect("hover queries")
            .iter()
            .filter(|query| query["file"] == "scripts/main.vela")
        {
            assert_eq!(
                hover(
                    &mut server,
                    &main_uri,
                    oracle::position(main, query["marker"].as_str().expect("marker"), true, 0)
                ),
                oracle::hover_result(main, query, true),
                "closed disk result"
            );
        }
        fs::remove_dir_all(parent).expect("remove owned isolated workspace");
    }
}
