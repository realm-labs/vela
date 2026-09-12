use crate::matrix_fixture::{FixtureWorkspace, load};
use crate::tests::{TestServer, notify, request, response_value};
use lsp_types::{notification as n, request as r};
use serde_json::json;

#[test]
fn call_parameter_mapping_matrix_projects_named_slots_at_unicode_positions() {
    for crlf in [false, true] {
        let mut spec = load("call-parameter-mapping");
        if crlf {
            for text in spec.files.values_mut() {
                *text = text.replace('\n', "\r\n");
            }
        }
        let fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let temp = crate::tests::support::unique_temp_root("call-parameter-mapping");
        let root = temp.join("中文 % workspace");
        fixture.materialize(&root).expect("materialize");
        let uri = |file: &str| {
            lsp_types::Url::from_file_path(root.join(file))
                .expect("URI")
                .to_string()
        };
        let mut server = TestServer::new();
        let _ = request::<r::Initialize>(
            &mut server,
            1,
            json!({"processId":null,"rootUri":uri(""),"capabilities":{}}),
        );
        let mut id = 2;
        for case in spec.oracle["queries"].as_array().expect("queries") {
            let file = case["file"].as_str().expect("file");
            let source = fixture.document(file).expect("source");
            let point = source.markers["cursor"].start;
            let _ = notify::<n::DidOpenTextDocument>(
                &mut server,
                json!({"textDocument":{"uri":uri(file),"languageId":"vela","version":1,"text":source.text}}),
            );
            let params = json!({"textDocument":{"uri":uri(file)},"position":{"line":point.line,"character":point.character}});
            let help = response_value(request::<r::SignatureHelpRequest>(
                &mut server,
                id,
                params.clone(),
            ));
            id += 1;
            assert!(help["error"].is_null(), "{case}: {help}");
            assert_eq!(
                help["result"]["signatures"]
                    .as_array()
                    .expect("signatures")
                    .len(),
                1,
                "{case}"
            );
            assert_eq!(
                help["result"]["signatures"][0]["label"], case["signature"],
                "{case}"
            );
            assert_eq!(help["result"]["activeParameter"], case["active"], "{case}");
            let repeat = response_value(request::<r::SignatureHelpRequest>(
                &mut server,
                id,
                params.clone(),
            ));
            id += 1;
            assert_eq!(help["result"], repeat["result"]);
            // Exercise a real UTF-16 incremental edit before the cursor: the
            // new non-BMP prefix must move the query without changing its slot.
            let _ = notify::<n::DidChangeTextDocument>(
                &mut server,
                json!({"textDocument":{"uri":uri(file),"version":2},"contentChanges":[{"range":{"start":{"line":0,"character":0},"end":{"line":0,"character":0}},"text":"/* 雪😀 */ "}]}),
            );
            let mut moved = params;
            if point.line == 0 {
                moved["position"]["character"] =
                    json!(point.character + "/* 雪😀 */ ".encode_utf16().count());
            }
            let again = response_value(request::<r::SignatureHelpRequest>(&mut server, id, moved));
            id += 1;
            assert_eq!(again["result"], help["result"], "{case}");
        }
        std::fs::remove_dir_all(temp).expect("cleanup");
    }
}
