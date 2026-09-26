use crate::matrix_fixture::{FixtureWorkspace, hover_signature, signature_calls};
use crate::tests::{TestServer, request, response_value, sync_diagnostics};
use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};
use std::{collections::BTreeSet, fs, path::Path};

fn uri(root: &Path, file: &str) -> String {
    lsp_types::Url::from_file_path(root.join(file))
        .expect("encoded URI")
        .to_string()
}

fn signature(server: &mut TestServer, uri: &str, position: &Value) -> Value {
    let response = response_value(request::<r::SignatureHelpRequest>(
        server,
        2,
        json!({"textDocument":{"uri":uri},"position":position}),
    ));
    assert!(response.get("error").is_none(), "{response}");
    assert!(response.get("result").is_some(), "explicit JSON null");
    response["result"].clone()
}

#[test]
fn signature_call_matrix_preserves_full_parameters_owners_and_static_boundaries() {
    verify_fixture("signature-s5", 161);
}

#[test]
fn signature_type_matrix_preserves_complete_hints_and_unknown_boundaries() {
    verify_fixture("signature-s3", 102);
}

fn verify_fixture(name: &str, expected_count: usize) {
    for crlf in [false, true] {
        let mut total = 0;
        for spec in signature_calls::specs(name, crlf) {
            let fixture = FixtureWorkspace::new(&spec).expect("fixture");
            let parent = crate::tests::support::unique_temp_root("signature-s5");
            let root = parent.join("中文 % signatures");
            fixture.materialize(&root).expect("isolated source");
            let mut server = TestServer::new();
            let initialize = response_value(request::<r::Initialize>(
                &mut server,
                1,
                json!({"processId":null,"rootUri":uri(&root,""),"capabilities":{}}),
            ));
            assert!(initialize.get("error").is_none(), "{initialize}");
            let mut opened = BTreeSet::new();
            for case in spec.oracle["queries"].as_array().expect("queries") {
                total += 1;
                let file = case["file"].as_str().expect("file");
                let source = &fixture.disk[file];
                let target = uri(&root, file);
                if opened.insert(file) {
                    let _ = sync_diagnostics::<n::DidOpenTextDocument>(
                        &mut server,
                        json!({"textDocument":{"uri":target,"languageId":"vela","version":1,"text":source.text}}),
                    );
                }
                let position = hover_signature::position(
                    source,
                    case["marker"].as_str().expect("marker"),
                    true,
                    0,
                );
                let actual = signature(&mut server, &target, &position);
                assert_eq!(
                    actual,
                    hover_signature::signature_result(case, true),
                    "{}/{}, CRLF={crlf}",
                    spec.id,
                    case["id"]
                );
                assert_eq!(signature(&mut server, &target, &position), actual, "repeat");
                assert_eq!(
                    fs::read_to_string(root.join(file)).expect("disk source"),
                    source.text,
                    "analysis queries preserve disk bytes"
                );
            }
            fs::remove_dir_all(parent).expect("remove isolated owned workspace");
        }
        assert_eq!(total, expected_count, "every reviewed position must run");
    }
}
