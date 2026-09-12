use crate::matrix_fixture::{FixtureWorkspace, load};
use crate::tests::{TestServer, notify, request, response_value};
use lsp_types::{notification as n, request as r};
use serde_json::json;

#[test]
fn call_argument_matrix_projects_owned_active_parameter_through_utf16_positions() {
    for crlf in [false, true] {
        let mut spec = load("call-argument-context");
        if crlf {
            for text in spec.files.values_mut() {
                *text = text.replace('\n', "\r\n");
            }
        }
        let fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let temp = crate::tests::support::unique_temp_root("call-argument-context");
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
        for (file, source) in &fixture.disk {
            if file.ends_with(".vela") {
                let _ = notify::<n::DidOpenTextDocument>(
                    &mut server,
                    json!({"textDocument":{"uri":uri(file),"languageId":"vela","version":1,"text":source.text}}),
                );
            }
        }
        let mut id = 2;
        for case in spec.oracle["queries"].as_array().expect("queries") {
            let file = case["file"].as_str().expect("file");
            let point = fixture.document(file).expect("source").markers["cursor"].start;
            let params = json!({"textDocument":{"uri":uri(file)},"position":{"line":point.line,"character":point.character}});
            let response = response_value(request::<r::SignatureHelpRequest>(
                &mut server,
                id,
                params.clone(),
            ));
            id += 1;
            let repeat = response_value(request::<r::SignatureHelpRequest>(
                &mut server,
                id,
                params.clone(),
            ));
            id += 1;
            assert_eq!(response["result"], repeat["result"], "{case}");
            if case["active"].is_null() {
                assert!(response["error"].is_null(), "{case}: {response}");
                assert!(response["result"].is_null(), "{case}: {response}");
                continue;
            }
            let completion =
                response_value(request::<r::Completion>(&mut server, id, params.clone()));
            id += 1;
            let completion_repeat =
                response_value(request::<r::Completion>(&mut server, id, params));
            id += 1;
            assert_eq!(
                completion["result"], completion_repeat["result"],
                "completion repeat: {case}"
            );
            let mut names = completion["result"]["items"]
                .as_array()
                .expect("items")
                .iter()
                .map(|item| item["label"].as_str().expect("name"))
                .collect::<Vec<_>>();
            names.sort_unstable();
            let mut expected = case["remainingParameters"]
                .as_array()
                .expect("remaining parameters")
                .iter()
                .map(|name| name.as_str().expect("name"))
                .collect::<Vec<_>>();
            expected.extend(crate::matrix_fixture::expected_expression_labels(
                &spec.oracle,
                case,
            ));
            expected.sort_unstable();
            assert_eq!(names, expected, "completion parameters: {case}");
            let result = &response["result"];
            assert_eq!(result["activeSignature"], 0, "{case}: {response}");
            assert_eq!(
                result["activeParameter"], case["active"],
                "{case}: {response}"
            );
            assert_eq!(
                result["signatures"].as_array().expect("signatures").len(),
                1,
                "{case}"
            );
            let callee = case["callee"].as_str().expect("callee");
            let parameters = if callee == "inner" {
                "first: Any, second: Any"
            } else {
                "first: Any, second: Any, third: Any"
            };
            let returns = if callee == "inner" { "i64" } else { "()" };
            assert_eq!(
                result["signatures"][0]["label"],
                format!("{callee}({parameters}) -> {returns}"),
                "{case}"
            );
            let active = case["active"].as_u64().expect("index") as usize;
            assert_eq!(
                result["signatures"][0]["parameters"][active]["label"],
                format!("{}: Any", ["first", "second", "third"][active]),
                "{case}"
            );
        }
        std::fs::remove_dir_all(temp).expect("cleanup");
    }
}
