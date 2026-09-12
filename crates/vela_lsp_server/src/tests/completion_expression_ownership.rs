use crate::matrix_fixture::{Edit, FixtureWorkspace, apply_edits, load};
use crate::tests::{TestServer, notify, request, response_value};
use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};

#[test]
fn expression_ownership_matrix_projects_applied_type_identity_and_receiver_members() {
    for crlf in [false, true] {
        let mut spec = load("completion-expression-ownership");
        if crlf {
            for text in spec.files.values_mut() {
                *text = text.replace('\n', "\r\n");
            }
        }
        let fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let temp = crate::tests::support::unique_temp_root("expression-ownership");
        let root = temp.join("中文 % workspace");
        fixture.materialize(&root).expect("workspace");
        let uri = |file: &str| {
            lsp_types::Url::from_file_path(root.join(file))
                .expect("URI")
                .to_string()
        };
        let mut server = TestServer::new();
        let _ = request::<r::Initialize>(
            &mut server,
            1,
            json!({"processId":null,"rootUri":uri(""),"capabilities":{"textDocument":{"completion":{"completionItem":{"labelDetailsSupport":true,"resolveSupport":{"properties":["documentation"]}}}}}}),
        );
        let mut id = 2;
        for case in spec.oracle["queries"].as_array().expect("queries") {
            let file = string(case, "file");
            let source = fixture.document(file).expect("source");
            let point = source.markers["cursor"].start;
            let range = source.markers["replace"];
            let _ = notify::<n::DidOpenTextDocument>(
                &mut server,
                json!({"textDocument":{"uri":uri(file),"languageId":"vela","version":1,"text":source.text}}),
            );
            let params = json!({"textDocument":{"uri":uri(file)},"position":{"line":point.line,"character":point.character}});
            let response =
                response_value(request::<r::Completion>(&mut server, id, params.clone()));
            id += 1;
            let items = response["result"]["items"].as_array().expect("items");
            let expected = case["items"].as_array().expect("expected items");
            let mut actual = items
                .iter()
                .map(|i| {
                    (
                        string(i, "label"),
                        string(&i["textEdit"], "newText"),
                        i["kind"].as_u64().expect("kind"),
                    )
                })
                .collect::<Vec<_>>();
            let mut keys = expected
                .iter()
                .map(|i| {
                    (
                        string(i, "label"),
                        string(i, "insert"),
                        if i["kind"] == "Type" { 22 } else { 6 },
                    )
                })
                .collect::<Vec<_>>();
            actual.sort();
            keys.sort();
            assert_eq!(actual, keys, "{case}");
            let mut version = 1;
            for expected in expected.iter().filter(|i| i["kind"] == "Type") {
                let item = items
                    .iter()
                    .find(|i| {
                        i["label"] == expected["label"]
                            && i["textEdit"]["newText"] == expected["insert"]
                    })
                    .expect("owned type");
                let insertion = string(expected, "insert");
                assert_eq!(
                    item["textEdit"],
                    json!({"range":{"start":{"line":range.start.line,"character":range.start.character},"end":{"line":range.end.line,"character":range.end.character}},"newText":insertion})
                );
                assert_eq!(
                    item["labelDetails"]["description"],
                    expected
                        .get("description")
                        .cloned()
                        .unwrap_or_else(|| json!(
                            string(expected, "symbol")
                                .rsplit_once("::")
                                .map(|(owner, _)| owner)
                        ))
                );
                let resolved = response_value(request::<r::ResolveCompletionItem>(
                    &mut server,
                    id,
                    item.clone(),
                ));
                id += 1;
                assert_eq!(resolved["result"]["textEdit"], item["textEdit"]);
                assert_eq!(resolved["result"]["data"], item["data"]);
                let edited = apply_edits(
                    &source.text,
                    &[Edit {
                        start: (range.start.line, range.start.character),
                        end: (range.end.line, range.end.character),
                        text: insertion,
                    }],
                )
                .expect("apply");
                assert_eq!(
                    edited,
                    format!(
                        "{}{}{}",
                        &source.text[..range.start.byte],
                        insertion,
                        &source.text[range.end.byte..]
                    )
                );
                assert!(
                    vela_syntax::parse::parse_source(&edited)
                        .diagnostics()
                        .is_empty()
                );
                version += 1;
                let _ = notify::<n::DidChangeTextDocument>(
                    &mut server,
                    json!({"textDocument":{"uri":uri(file),"version":version},"contentChanges":[{"text":edited}]}),
                );
                let name_start = insertion.rfind("::").map_or(0, |i| i + 2);
                let definition = response_value(request::<r::GotoDefinition>(
                    &mut server,
                    id,
                    json!({"textDocument":{"uri":uri(file)},"position":{"line":range.start.line,"character":range.start.character+name_start+1}}),
                ));
                id += 1;
                let target = if let Some(target) = expected["target"].as_str() {
                    let marker = fixture.document(target).expect("target").markers["type"];
                    json!({"uri":uri(target),"range":{"start":{"line":marker.start.line,"character":marker.start.character},"end":{"line":marker.end.line,"character":marker.end.character}}})
                } else {
                    Value::Null
                };
                assert_eq!(definition["result"], target, "{case} {expected}");
                if case["receiver"] == true {
                    let member = source.markers["member"].start;
                    let character = member.character + insertion.encode_utf16().count()
                        - (range.end.character - range.start.character);
                    let members = response_value(request::<r::Completion>(
                        &mut server,
                        id,
                        json!({"textDocument":{"uri":uri(file)},"position":{"line":member.line,"character":character}}),
                    ));
                    id += 1;
                    let members = members["result"]["items"].as_array().expect("members");
                    assert_eq!(
                        members
                            .iter()
                            .map(|i| string(i, "label"))
                            .collect::<Vec<_>>(),
                        ["tag"],
                        "{case} {expected}"
                    );
                    assert_eq!(
                        members[0]["data"]["resolve"]["symbol"],
                        json!({"kind":expected["origin"],"name":format!("{}.tag",string(expected,"symbol"))}),
                        "{case} {expected}"
                    );
                    assert_eq!(
                        members[0]["detail"], expected["memberDetail"],
                        "{case} {expected}"
                    );
                }
                version += 1;
                let _ = notify::<n::DidChangeTextDocument>(
                    &mut server,
                    json!({"textDocument":{"uri":uri(file),"version":version},"contentChanges":[{"text":source.text}]}),
                );
                let restored =
                    response_value(request::<r::Completion>(&mut server, id, params.clone()));
                id += 1;
                assert_eq!(restored["result"], response["result"], "{case}");
            }
        }
        std::fs::remove_dir_all(temp).expect("cleanup");
    }
}
fn string<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().expect("fixture string")
}
