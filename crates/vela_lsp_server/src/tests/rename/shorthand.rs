use super::collisions::{point, query, uri, wire_point};
use super::{initialize, open_document};
use crate::matrix_fixture::{Document, Edit, apply_edits, rename_shorthand};
use crate::tests::{TestServer, notify};
use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};

#[test]
fn local_shorthand_rename_projects_expansions_and_preserves_owners() {
    for crlf in [false, true] {
        for (case, before, after) in rename_shorthand::cases(crlf) {
            let id = case["id"].as_str().expect("id");
            let mut endpoint = TestServer::new();
            let _ = initialize(&mut endpoint, "file:///workspace/scripts");
            let main = uri("scripts/main.vela");
            open_document(&mut endpoint, &main, 1, &before.text);
            check(&mut endpoint, &before, id);
            let mut expected=case["edits"].as_object().expect("edits").iter().map(|(site,text)| {
                let marker=before.markers[site];
                (marker.start.byte,json!({"range":{"start":point(marker.start),"end":point(marker.end)},"newText":text}))
            }).collect::<Vec<_>>();
            expected.sort_by_key(|(offset, _)| *offset);
            let expected = expected
                .into_iter()
                .map(|(_, edit)| edit)
                .collect::<Vec<_>>();
            let mut last = Value::Null;
            for site in case["edits"].as_object().expect("edits").keys() {
                let marker = before.markers[site];
                let params = json!({"textDocument":{"uri":main},"position":point(marker.start)});
                let prepared = query::<r::PrepareRenameRequest>(&mut endpoint, params.clone());
                assert_eq!(prepared["placeholder"], "value", "{id}/{site}");
                assert_eq!(
                    prepared["range"],
                    json!({"start":point(marker.start),"end":point(marker.end)})
                );
                let edit = query::<r::Rename>(
                    &mut endpoint,
                    json!({"textDocument":params["textDocument"],"position":params["position"],"newName":case["newName"]}),
                );
                assert_eq!(
                    edit["changes"],
                    json!({main.clone():expected}),
                    "{id}/{site}"
                );
                assert_eq!(
                    edit["documentChanges"],
                    json!([{"textDocument":{"uri":main,"version":1},"edits":expected}]),
                    "{id}/{site}"
                );
                last = edit;
            }
            let edits = last["changes"][&main]
                .as_array()
                .expect("edits")
                .iter()
                .map(|edit| Edit {
                    start: wire_point(&edit["range"]["start"]),
                    end: wire_point(&edit["range"]["end"]),
                    text: edit["newText"].as_str().expect("text"),
                })
                .collect::<Vec<_>>();
            let actual = apply_edits(&before.text, &edits).expect("UTF-16 edits");
            assert_eq!(actual, after.text, "{id}");
            let _ = notify::<n::DidChangeTextDocument>(
                &mut endpoint,
                json!({"textDocument":{"uri":main,"version":2},"contentChanges":[{"text":actual}]}),
            );
            check(&mut endpoint, &after, id);
            let _ = notify::<n::DidChangeTextDocument>(
                &mut endpoint,
                json!({"textDocument":{"uri":main,"version":3},"contentChanges":[{"text":before.text}]}),
            );
            check(&mut endpoint, &before, id);
        }
    }
}

fn check(endpoint: &mut TestServer, doc: &Document, id: &str) {
    assert!(
        vela_syntax::parse::parse_source(&doc.text)
            .diagnostics()
            .is_empty(),
        "{id}"
    );
    let target = doc.markers["target"];
    let definition = query::<r::GotoDefinition>(
        endpoint,
        json!({"textDocument":{"uri":uri("scripts/main.vela")},"position":point(doc.markers["read"].start)}),
    );
    assert_eq!(
        definition,
        json!({"uri":uri("scripts/main.vela"),"range":{"start":point(target.start),"end":point(target.end)}}),
        "{id}"
    );
    for include in [true, false] {
        let references = query::<r::References>(
            endpoint,
            json!({"textDocument":{"uri":uri("scripts/main.vela")},"position":point(target.start),"context":{"includeDeclaration":include}}),
        );
        let mut actual = references.as_array().expect("references").clone();
        let mut expected=doc.markers.iter().filter(|(name,_)|include||name.as_str()!="target").map(|(_,marker)|json!({"uri":uri("scripts/main.vela"),"range":{"start":point(marker.start),"end":point(marker.end)}})).collect::<Vec<_>>();
        actual.sort_by_key(Value::to_string);
        expected.sort_by_key(Value::to_string);
        assert_eq!(actual, expected, "{id}, include={include}");
    }
}
