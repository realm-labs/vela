use super::collisions::{point, query, uri, wire_point};
use super::{initialize, open_document};
use crate::matrix_fixture::{
    Edit, FixtureWorkspace, Spec, apply_edits, references, rename_collisions as oracle,
};
use crate::tests::{TestServer, notify};
use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};

#[test]
fn local_rename_collision_matrix_projects_exact_edits_and_preserves_owners() {
    for crlf in [false, true] {
        for spec in oracle::local_cases(crlf) {
            let id = spec.oracle["id"].as_str().expect("id");
            let fixture = FixtureWorkspace::new(&spec).expect(id);
            references::assert_parsed(&fixture);
            let mut endpoint = TestServer::new();
            let _ = initialize(&mut endpoint, "file:///workspace/scripts");
            for (file, doc) in &fixture.disk {
                if file.ends_with(".vela") {
                    open_document(&mut endpoint, &uri(file), 1, &doc.text);
                }
            }
            check_owners(&mut endpoint, &fixture, &spec);
            let main = uri("scripts/main.vela");
            let doc = &fixture.disk["scripts/main.vela"];
            let mut sites = doc
                .markers
                .iter()
                .filter(|(name, _)| name.as_str() == "target" || name.starts_with("use"))
                .collect::<Vec<_>>();
            sites.sort_by_key(|(_, marker)| marker.start.byte);
            let expected_edits = sites.iter().map(|(_, marker)| json!({"range":{"start":point(marker.start),"end":point(marker.end)},"newText":spec.oracle["newName"]})).collect::<Vec<_>>();
            let mut last_edit = Value::Null;
            for (name, site) in &sites {
                let params = json!({"textDocument":{"uri":main},"position":point(site.start)});
                let prepare = query::<r::PrepareRenameRequest>(&mut endpoint, params.clone());
                assert_eq!(prepare["placeholder"], "value", "{id}/{name}");
                assert_eq!(
                    prepare["range"],
                    json!({"start":point(site.start),"end":point(site.end)})
                );
                let edit = query::<r::Rename>(
                    &mut endpoint,
                    json!({"textDocument":params["textDocument"],"position":params["position"],"newName":spec.oracle["newName"]}),
                );
                assert_eq!(
                    !edit.is_null(),
                    spec.oracle["allowed"].as_bool().expect("allowed"),
                    "{id}/{name}: {edit}"
                );
                if edit.is_null() {
                    continue;
                }
                assert_eq!(
                    edit["changes"],
                    json!({main.clone():expected_edits}),
                    "{id}/{name}"
                );
                assert_eq!(
                    edit["documentChanges"],
                    json!([{"textDocument":{"uri":main,"version":1},"edits":expected_edits}]),
                    "{id}/{name}"
                );
                last_edit = edit;
            }
            if !last_edit.is_null() {
                let edits = last_edit["changes"][&main]
                    .as_array()
                    .expect("edits")
                    .iter()
                    .map(|edit| Edit {
                        start: wire_point(&edit["range"]["start"]),
                        end: wire_point(&edit["range"]["end"]),
                        text: edit["newText"].as_str().expect("text"),
                    })
                    .collect::<Vec<_>>();
                let actual = apply_edits(&doc.text, &edits).expect("UTF-16 edits");
                let expected =
                    FixtureWorkspace::new(&oracle::renamed_local(&spec)).expect("expected");
                assert_eq!(actual, expected.disk["scripts/main.vela"].text, "{id}");
                references::assert_parsed(&expected);
                let _ = notify::<n::DidChangeTextDocument>(
                    &mut endpoint,
                    json!({"textDocument":{"uri":main,"version":2},"contentChanges":[{"text":actual}]}),
                );
                check_owners(&mut endpoint, &expected, &spec);
                let _ = notify::<n::DidChangeTextDocument>(
                    &mut endpoint,
                    json!({"textDocument":{"uri":main,"version":3},"contentChanges":[{"text":doc.text}]}),
                );
            }
            check_owners(&mut endpoint, &fixture, &spec);
        }
    }
}

fn check_owners(endpoint: &mut TestServer, fixture: &FixtureWorkspace, spec: &Spec) {
    let id = spec.oracle["id"].as_str().expect("id");
    let file = "scripts/main.vela";
    let doc = &fixture.disk[file];
    for (site, owner) in spec.oracle["probes"].as_object().expect("probes") {
        let definition = query::<r::GotoDefinition>(
            endpoint,
            json!({"textDocument":{"uri":uri(file)},"position":point(doc.markers[site].start)}),
        );
        let Some(owner) = owner.as_str() else {
            assert!(definition.is_null(), "{id}/{site}");
            continue;
        };
        let target_file = if owner == "global" {
            "scripts/helpers.vela"
        } else {
            file
        };
        let target = fixture.disk[target_file].markers[owner];
        assert_eq!(
            definition,
            json!({"uri":uri(target_file),"range":{"start":point(target.start),"end":point(target.end)}}),
            "{id}/{site}"
        );
    }
    for include in [true, false] {
        let mut expected = doc.markers.iter().filter(|(name, _)| (include && name.as_str() == "target") || name.starts_with("use")).map(|(_, marker)| json!({"uri":uri(file),"range":{"start":point(marker.start),"end":point(marker.end)}})).collect::<Vec<_>>();
        let actual = query::<r::References>(
            endpoint,
            json!({"textDocument":{"uri":uri(file)},"position":point(doc.markers["target"].start),"context":{"includeDeclaration":include}}),
        );
        let mut actual = actual.as_array().expect("references").clone();
        actual.sort_by_key(Value::to_string);
        expected.sort_by_key(Value::to_string);
        assert_eq!(actual, expected, "{id}, include={include}");
    }
}
