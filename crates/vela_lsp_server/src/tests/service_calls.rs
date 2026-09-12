use crate::matrix_fixture::{Edit, FixtureWorkspace, apply_edits, load};
use crate::tests::{TestServer, notify, request, response_value};
use lsp_types::{notification as n, request as r};
use serde_json::json;

#[test]
fn service_call_matrix_projects_positional_completion_edits_and_exact_signatures() {
    for (crlf, mode) in [false, true]
        .into_iter()
        .flat_map(|crlf| ["full", "missing", "ordinary"].map(|mode| (crlf, mode)))
    {
        let mut spec = load("completion-service-calls");
        if crlf {
            for text in spec.files.values_mut() {
                *text = text.replace('\n', "\r\n");
            }
        }
        if mode == "missing" {
            spec.files.remove("schema.json");
        }
        if mode == "ordinary" {
            let mut schema: serde_json::Value =
                serde_json::from_str(&spec.files["schema.json"]).expect("schema");
            schema.as_object_mut().expect("object").remove("serviceSet");
            spec.files
                .insert("schema.json".to_owned(), schema.to_string());
        }
        let fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let temp = crate::tests::support::unique_temp_root("service-calls");
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
            let source = fixture.document(file).expect("source");
            let point = source.markers["cursor"].start;
            let params = json!({"textDocument":{"uri":uri(file)},"position":{"line":point.line,"character":point.character}});
            let response = response_value(request::<r::SignatureHelpRequest>(
                &mut server,
                id,
                params.clone(),
            ));
            id += 1;
            assert!(response["error"].is_null(), "{case}: {response}");
            let help = &response["result"];
            if let Some(signature) = case["signature"].as_str().filter(|_| mode == "full") {
                assert_eq!(
                    help["signatures"].as_array().expect("signatures").len(),
                    1,
                    "{case}"
                );
                assert_eq!(help["signatures"][0]["label"], signature, "{case}");
                assert_eq!(help["activeParameter"], case["active"], "{case}");
            } else {
                assert!(help.is_null(), "{case}: {help}");
            }
            let first = response_value(request::<r::Completion>(&mut server, id, params.clone()));
            id += 1;
            let repeat = response_value(request::<r::Completion>(&mut server, id, params));
            id += 1;
            assert_eq!(first["result"], repeat["result"], "{case}");
            let items = first["result"]["items"].as_array().expect("items");
            assert_eq!(
                items
                    .iter()
                    .map(|i| i["label"].as_str().expect("label"))
                    .collect::<Vec<_>>(),
                ["chosen_value"],
                "{case}"
            );
            let edit = &items[0]["textEdit"];
            let range = source.markers["replace"];
            assert_eq!(
                edit["range"],
                json!({"start":{"line":range.start.line,"character":range.start.character},"end":{"line":range.end.line,"character":range.end.character}}),
                "{case}"
            );
            assert_eq!(edit["newText"], "chosen_value", "{case}");
            let projected: lsp_types::TextEdit =
                serde_json::from_value(edit.clone()).expect("edit");
            let actual = Edit {
                start: (
                    projected.range.start.line as usize,
                    projected.range.start.character as usize,
                ),
                end: (
                    projected.range.end.line as usize,
                    projected.range.end.character as usize,
                ),
                text: &projected.new_text,
            };
            let edited = apply_edits(&source.text, &[actual]).expect("apply");
            let expected = format!(
                "{}chosen_value{}",
                &source.text[..range.start.byte],
                &source.text[range.end.byte..]
            );
            assert_eq!(edited, expected, "{case}");
            assert!(
                vela_syntax::parse::parse_source(&format!(
                    "{edited}{}",
                    case["recoverySuffix"].as_str().unwrap_or("")
                ))
                .diagnostics()
                .is_empty(),
                "{case}: {edited}"
            );
            let _ = notify::<n::DidChangeTextDocument>(
                &mut server,
                json!({"textDocument":{"uri":uri(file),"version":2},"contentChanges":[{"text":edited}]}),
            );
            let again = response_value(request::<r::Completion>(
                &mut server,
                id,
                json!({"textDocument":{"uri":uri(file)},"position":{"line":range.start.line,"character":range.start.character + "chosen_value".len()}}),
            ));
            id += 1;
            assert_eq!(
                again["result"]["items"]
                    .as_array()
                    .expect("re-query items")
                    .iter()
                    .map(|i| i["label"].as_str().expect("label"))
                    .collect::<Vec<_>>(),
                ["chosen_value"],
                "re-query: {case}"
            );
        }
        std::fs::remove_dir_all(temp).expect("cleanup");
    }
}
