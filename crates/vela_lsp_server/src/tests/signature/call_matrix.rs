use crate::matrix_fixture::{FixtureWorkspace, hover_signature, signature_calls};
use crate::tests::{TestServer, request, response_value, sync_diagnostics};
use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};
use std::{collections::BTreeSet, fs, path::Path};

mod capabilities;

fn uri(root: &Path, file: &str) -> String {
    lsp_types::Url::from_file_path(root.join(file))
        .expect("encoded URI")
        .to_string()
}

fn signature(server: &mut TestServer, uri: &str, position: &Value, context: &Value) -> Value {
    let mut params = json!({"textDocument":{"uri":uri},"position":position});
    if !context.is_null() {
        params["context"] = context.clone();
    }
    let response = response_value(request::<r::SignatureHelpRequest>(server, 2, params));
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

#[test]
fn signature_ownership_matrix_preserves_static_targets_and_lexical_shadowing() {
    verify_fixture("signature-s10", 75);
}

#[test]
fn signature_client_capability_matrix_preserves_triggers_labels_and_current_targets() {
    let clients = capabilities::profiles();
    assert_eq!(clients.len(), 5, "all required capability profiles");
    for client in clients {
        assert_eq!(client.contexts.len(), 5, "all required contexts");
        verify_client("signature-s10", 75, &client);
    }
}

fn verify_fixture(name: &str, expected_count: usize) {
    verify_client(name, expected_count, &capabilities::minimal());
}

fn verify_client(name: &str, expected_count: usize, client: &capabilities::Client) {
    for crlf in [false, true] {
        let mut total = 0;
        for spec in signature_calls::specs(name, crlf) {
            let fixture = FixtureWorkspace::new(&spec).expect("fixture");
            let parent = crate::tests::support::unique_temp_root(name);
            let root = parent.join("中文 % signatures");
            fixture.materialize(&root).expect("isolated source");
            let mut server = TestServer::new();
            let initialize = response_value(request::<r::Initialize>(
                &mut server,
                1,
                json!({"processId":null,"rootUri":uri(&root,""),"capabilities":client.capabilities}),
            ));
            assert!(initialize.get("error").is_none(), "{initialize}");
            assert_eq!(
                initialize["result"]["capabilities"]["signatureHelpProvider"],
                json!({"triggerCharacters":["(",","],"retriggerCharacters":[","]}),
                "{} advertised triggers",
                client.id
            );
            let mut opened = BTreeSet::new();
            for case in spec.oracle["queries"].as_array().expect("queries") {
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
                for context in &client.contexts {
                    total += 1;
                    let actual = signature(&mut server, &target, &position, context);
                    assert_eq!(
                        actual,
                        hover_signature::signature_result(case, true),
                        "{}/{}, client={}, context={context}, CRLF={crlf}",
                        spec.id,
                        case["id"],
                        client.id,
                    );
                    assert_eq!(
                        signature(&mut server, &target, &position, context),
                        actual,
                        "repeat"
                    );
                }
                assert_eq!(
                    fs::read_to_string(root.join(file)).expect("disk source"),
                    source.text,
                    "analysis queries preserve disk bytes"
                );
            }
            fs::remove_dir_all(parent).expect("remove isolated owned workspace");
        }
        assert_eq!(
            total,
            expected_count * client.contexts.len(),
            "every reviewed position and context must run"
        );
    }
}
