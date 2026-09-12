use crate::matrix_fixture::{Edit, FixtureWorkspace, apply_edits, load};
use crate::tests::{TestServer, notify, request, response_value};
use lsp_types::{notification as n, request as r};
use serde_json::json;

#[test]
fn service_parameter_candidates_follow_schema_watch_events() {
    let spec = load("completion-service-parameters");
    let fixture = FixtureWorkspace::new(&spec).expect("fixture");
    let temp = crate::tests::support::unique_temp_root("service-parameter-refresh");
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
    let source = fixture.document("scripts/local.vela").expect("source");
    let _ = notify::<n::DidOpenTextDocument>(
        &mut server,
        json!({"textDocument":{"uri":uri("scripts/local.vela"),"languageId":"vela","version":1,"text":source.text}}),
    );
    let original = &fixture.disk["schema.json"].text;
    let mut changed: serde_json::Value = serde_json::from_str(original).expect("schema");
    changed["serviceSet"]["services"][0]["methods"][1]["parameters"][0]["typeHint"] =
        "String".into();
    let changed = changed.to_string();
    let mut ordinary: serde_json::Value = serde_json::from_str(original).expect("schema");
    ordinary
        .as_object_mut()
        .expect("schema")
        .remove("serviceSet");
    let ordinary = ordinary.to_string();
    let point = source.markers["cursor"].start;
    for (index, (bytes, detail)) in [
        (Some(original.as_str()), "api::Peer"),
        (Some(changed.as_str()), "String"),
        (Some(ordinary.as_str()), "unknown"),
        (Some("{"), "unknown"),
        (None, "unknown"),
        (Some(original.as_str()), "api::Peer"),
    ]
    .into_iter()
    .enumerate()
    {
        if let Some(bytes) = bytes {
            std::fs::write(root.join("schema.json"), bytes).expect("write schema");
        } else {
            std::fs::remove_file(root.join("schema.json")).expect("remove schema");
        }
        let _ = notify::<n::DidChangeWatchedFiles>(
            &mut server,
            json!({"changes":[{"uri":uri("schema.json"),"type":if bytes.is_none() { 3 } else if index == 5 { 1 } else { 2 }}]}),
        );
        let params = json!({"textDocument":{"uri":uri("scripts/local.vela")},"position":{"line":point.line,"character":point.character}});
        let result = response_value(request::<r::Completion>(
            &mut server,
            10 + index as i32,
            params.clone(),
        ));
        assert!(result["error"].is_null());
        let items = result["result"]["items"].as_array().expect("items");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["label"], "chosen_value");
        assert_eq!(items[0]["detail"], detail);
        let repeat = response_value(request::<r::Completion>(
            &mut server,
            20 + index as i32,
            params,
        ));
        assert_eq!(repeat["result"], result["result"]);
    }
    std::fs::remove_dir_all(temp).expect("cleanup");
}

#[test]
fn service_parameter_matrix_projects_contextual_candidates_and_utf16_edits() {
    for (crlf, mode) in [false, true]
        .into_iter()
        .flat_map(|crlf| ["full", "missing", "ordinary"].map(|mode| (crlf, mode)))
    {
        let mut spec = load("completion-service-parameters");
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
        let temp = crate::tests::support::unique_temp_root("service-parameters");
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
            if case.get("signature").is_some() {
                let help = response_value(request::<r::SignatureHelpRequest>(
                    &mut server,
                    id,
                    params.clone(),
                ));
                id += 1;
                assert!(help["error"].is_null());
                if mode == "full" {
                    assert_eq!(
                        help["result"]["signatures"]
                            .as_array()
                            .expect("signatures")
                            .len(),
                        1
                    );
                    assert_eq!(help["result"]["signatures"][0]["label"], case["signature"]);
                    assert_eq!(help["result"]["activeParameter"], 0);
                } else {
                    assert!(help["result"].is_null());
                }
            }
            let first = response_value(request::<r::Completion>(&mut server, id, params.clone()));
            id += 1;
            let repeat = response_value(request::<r::Completion>(&mut server, id, params));
            id += 1;
            assert!(first["error"].is_null(), "{case}: {first}");
            assert_eq!(first["result"], repeat["result"], "{case}");
            let items = first["result"]["items"].as_array().expect("items");
            let expected = if mode == "full" {
                case["items"].as_array().expect("items").as_slice()
            } else {
                case["withoutSchemaItems"]
                    .as_array()
                    .expect("fallback")
                    .as_slice()
            };
            let mut actual_labels = items
                .iter()
                .map(|i| i["label"].as_str().expect("label"))
                .collect::<Vec<_>>();
            actual_labels.sort_unstable();
            let mut expected_labels = expected
                .iter()
                .map(|i| i["label"].as_str().expect("label"))
                .collect::<Vec<_>>();
            expected_labels.sort_unstable();
            assert_eq!(
                actual_labels, expected_labels,
                "{mode}, CRLF={crlf}: {case}"
            );
            let mut version = 1;
            for expected in expected {
                let item = items
                    .iter()
                    .find(|i| i["label"] == expected["label"])
                    .expect("candidate");
                assert_eq!(item["kind"], expected["lspKind"]);
                assert_eq!(item["detail"], expected["detail"]);
                assert_eq!(item["filterText"], expected["label"]);
                if !expected["symbol"].is_null() {
                    assert_eq!(
                        item["data"]["resolve"],
                        json!({"kind":"documentation","symbol":{"kind":"schema","name":expected["symbol"]}})
                    );
                    let resolved = response_value(request::<r::ResolveCompletionItem>(
                        &mut server,
                        id,
                        item.clone(),
                    ));
                    id += 1;
                    assert!(resolved["error"].is_null());
                    assert_eq!(resolved["result"], *item);
                } else {
                    assert!(item["data"]["resolve"].is_null());
                }
                assert_eq!(
                    item["insertTextFormat"],
                    if expected["insert"].as_str().expect("insert").contains("$0") {
                        json!(2)
                    } else {
                        serde_json::Value::Null
                    }
                );
                let range = source.markers["replace"];
                assert_eq!(
                    item["textEdit"],
                    json!({"range":{"start":{"line":range.start.line,"character":range.start.character},"end":{"line":range.end.line,"character":range.end.character}},"newText":expected["insert"]})
                );
                let edit: lsp_types::TextEdit =
                    serde_json::from_value(item["textEdit"].clone()).expect("edit");
                let insertion = edit.new_text.replace("$0", "");
                let text = apply_edits(
                    &source.text,
                    &[Edit {
                        start: (
                            edit.range.start.line as usize,
                            edit.range.start.character as usize,
                        ),
                        end: (
                            edit.range.end.line as usize,
                            edit.range.end.character as usize,
                        ),
                        text: &insertion,
                    }],
                )
                .expect("apply");
                assert_eq!(
                    text,
                    format!(
                        "{}{}{}",
                        &source.text[..range.start.byte],
                        expected["insert"]
                            .as_str()
                            .expect("insert")
                            .replace("$0", ""),
                        &source.text[range.end.byte..]
                    )
                );
                let parsed = vela_syntax::parse::parse_source(&format!(
                    "{text}{}",
                    case["recoverySuffix"].as_str().expect("suffix")
                ));
                assert!(
                    parsed.diagnostics().is_empty(),
                    "{case}: {:?}",
                    parsed.diagnostics()
                );
                version += 1;
                let _ = notify::<n::DidChangeTextDocument>(
                    &mut server,
                    json!({"textDocument":{"uri":uri(file),"version":version},"contentChanges":[{"text":text}]}),
                );
                let again = response_value(request::<r::Completion>(
                    &mut server,
                    id,
                    json!({"textDocument":{"uri":uri(file)},"position":{"line":range.start.line,"character":range.start.character + expected["label"].as_str().expect("label").encode_utf16().count()}}),
                ));
                id += 1;
                let reapplied = again["result"]["items"]
                    .as_array()
                    .expect("re-query items")
                    .iter()
                    .find(|i| i["label"] == expected["label"])
                    .expect("re-query candidate");
                assert_eq!(reapplied["data"], item["data"]);
                assert_eq!(reapplied["detail"], item["detail"]);
                version += 1;
                let _ = notify::<n::DidChangeTextDocument>(
                    &mut server,
                    json!({"textDocument":{"uri":uri(file),"version":version},"contentChanges":[{"text":source.text}]}),
                );
            }
        }
        std::fs::remove_dir_all(temp).expect("cleanup");
    }
}
