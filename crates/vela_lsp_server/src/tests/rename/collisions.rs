use super::{initialize, open_document};
use crate::matrix_fixture::{
    Edit, FixtureWorkspace, Point, apply_edits, references, rename_collisions,
};
use crate::tests::{TestServer, notify, request, response_value};
use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};

#[test]
fn declaration_rename_collision_matrix_rejects_capture_and_applies_safe_edits() {
    for crlf in [false, true] {
        for (id, allowed, spec) in rename_collisions::cases(crlf) {
            let fixture = FixtureWorkspace::new(&spec).expect("fixture");
            references::assert_parsed(&fixture);
            let mut endpoint = TestServer::new();
            let _ = initialize(&mut endpoint, "file:///workspace/scripts");
            for (file, doc) in &fixture.disk {
                if file.ends_with(".vela") {
                    open_document(&mut endpoint, &uri(file), 1, &doc.text);
                }
            }
            assert_owner(&mut endpoint, &fixture, &id);
            let expected =
                FixtureWorkspace::new(&rename_collisions::renamed(&spec)).expect("expected");
            let mut last_edit = Value::Null;
            for (file, doc) in &fixture.disk {
                for (marker, site) in &doc.markers {
                    if marker == "alias" {
                        continue;
                    }
                    let params =
                        json!({"textDocument":{"uri":uri(file)}, "position":point(site.start)});
                    let prepare = query::<r::PrepareRenameRequest>(&mut endpoint, params.clone());
                    assert_eq!(prepare["placeholder"], "grant", "{id}/{marker}");
                    assert_eq!(
                        prepare["range"],
                        json!({"start":point(site.start), "end":point(site.end)})
                    );
                    let edit = query::<r::Rename>(
                        &mut endpoint,
                        json!({"textDocument":params["textDocument"], "position":params["position"], "newName":"award"}),
                    );
                    assert_eq!(!edit.is_null(), allowed, "{id}/{marker}: {edit}");
                    if !allowed {
                        continue;
                    }
                    let mut changes = serde_json::Map::new();
                    for (file, source) in &fixture.disk {
                        let mut markers = source
                            .markers
                            .iter()
                            .filter(|(name, _)| name.as_str() != "alias")
                            .collect::<Vec<_>>();
                        markers.sort_by_key(|(_, marker)| marker.start.byte);
                        if markers.is_empty() {
                            continue;
                        }
                        let edits = markers.iter().map(|(_, marker)| json!({"range":{"start":point(marker.start),"end":point(marker.end)},"newText":"award"})).collect::<Vec<_>>();
                        changes.insert(uri(file), edits.into());
                    }
                    assert_eq!(edit["changes"], Value::Object(changes), "{id}/{marker}");
                    last_edit = edit;
                }
            }
            if allowed {
                for (file, doc) in &fixture.disk {
                    if !file.ends_with(".vela") {
                        continue;
                    }
                    let edits = last_edit["changes"][uri(file)]
                        .as_array()
                        .expect("edits")
                        .iter()
                        .map(|edit| Edit {
                            start: wire_point(&edit["range"]["start"]),
                            end: wire_point(&edit["range"]["end"]),
                            text: edit["newText"].as_str().expect("text"),
                        })
                        .collect::<Vec<_>>();
                    let applied = apply_edits(&doc.text, &edits).expect("UTF-16 edits");
                    assert_eq!(applied, expected.disk[file].text, "{id}/{file}");
                    let _ = notify::<n::DidChangeTextDocument>(
                        &mut endpoint,
                        json!({"textDocument":{"uri":uri(file),"version":2},"contentChanges":[{"text":applied}]}),
                    );
                }
                references::assert_parsed(&expected);
                assert_owner(&mut endpoint, &expected, &id);
            } else {
                assert_owner(&mut endpoint, &fixture, &id);
            }
        }
    }
}

fn assert_owner(endpoint: &mut TestServer, fixture: &FixtureWorkspace, id: &str) {
    let doc = &fixture.disk["scripts/main.vela"];
    let site = doc
        .markers
        .get("use")
        .or_else(|| doc.markers.get("alias"))
        .expect("use");
    let definition = query::<r::GotoDefinition>(
        endpoint,
        json!({"textDocument":{"uri":uri("scripts/main.vela")},"position":point(site.start)}),
    );
    let target = fixture.disk["scripts/helpers.vela"].markers["declaration"];
    assert_eq!(
        definition,
        json!({"uri":uri("scripts/helpers.vela"),"range":{"start":point(target.start),"end":point(target.end)}}),
        "{id}"
    );
}
pub(super) fn query<R: r::Request>(endpoint: &mut TestServer, params: Value) -> Value {
    let response = response_value(request::<R>(endpoint, 50, params));
    assert_eq!(response["id"], 50);
    if let Some(error) = response.get("error") {
        assert_eq!(R::METHOD, "textDocument/rename", "{response}");
        assert_eq!(error["code"], -32600, "{response}");
        assert!(
            error["message"]
                .as_str()
                .is_some_and(|message| message.contains("was rejected")),
            "{response}"
        );
        return Value::Null;
    }
    assert!(response.get("error").is_none(), "{response}");
    response["result"].clone()
}
pub(super) fn uri(file: &str) -> String {
    format!("file:///workspace/{file}")
}
pub(super) fn point(point: Point) -> Value {
    json!({"line":point.line,"character":point.character})
}
pub(super) fn wire_point(value: &Value) -> (usize, usize) {
    (
        value["line"].as_u64().expect("line") as usize,
        value["character"].as_u64().expect("character") as usize,
    )
}
