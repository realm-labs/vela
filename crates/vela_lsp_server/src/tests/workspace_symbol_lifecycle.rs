use super::document_symbol_lifecycle::{Harness, check_snapshot};
use super::{request, response_value};
use crate::matrix_fixture::{
    FixtureWorkspace, Spec, document_symbol_lifecycle as states, workspace_symbols as oracle,
};
use lsp_types::request as r;
use serde_json::{Value, json};

fn check(harness: &mut Harness, fixture: &FixtureWorkspace, spec: &Spec, phase: &Value) {
    // Keep the original whole-outline, physical disk and bound schema assertions.
    harness.check(fixture, spec, phase);
    let mut effective = fixture.clone();
    for (file, doc) in &fixture.open {
        effective.disk.insert(file.clone(), doc.clone());
    }
    let schema_path = harness.root.join("schema.json");
    let schema_before = std::fs::read(&schema_path).ok();
    if phase["schema"]["mode"] == "missing" {
        assert!(schema_before.is_none());
    } else {
        assert!(schema_before.is_some());
    }
    let before = harness
        .test_server
        .snapshot()
        .databases()
        .workspace_symbols("");
    for encoding in ["%E4%B8%AD", "%20", "%25"] {
        assert!(harness.uri("").contains(encoding));
    }
    let workspace = &phase["workspace"];
    for query in workspace["queries"].as_array().expect("queries") {
        let wanted = oracle::expected(&effective, workspace, &query["symbols"], true, &|file| {
            harness.uri(file)
        });
        for repeat in 0..3 {
            harness.id += 1;
            let response = response_value(request::<r::WorkspaceSymbolRequest>(
                &mut harness.test_server,
                harness.id,
                json!({"query":query["query"]}),
            ));
            assert_eq!(
                response,
                json!({"jsonrpc":"2.0","id":harness.id,"result":wanted}),
                "phase={} query={} repeat={repeat}",
                phase["id"],
                query["id"]
            );
        }
    }
    assert_eq!(
        harness
            .test_server
            .snapshot()
            .databases()
            .workspace_symbols(""),
        before,
        "query cannot mutate ownership"
    );
    assert_eq!(
        std::fs::read(schema_path).ok(),
        schema_before,
        "query cannot write schema"
    );
}

#[test]
fn lsp_workspace_symbol_lifecycle_pins_whole_utf16_sets_and_frozen_snapshots_with_fresh_parity() {
    for crlf in [false, true] {
        for shifted in [false, true] {
            let spec = oracle::lifecycle(crlf, shifted);
            let mut fixture = FixtureWorkspace::new(&spec).expect("fixture");
            let phases = spec.oracle["phases"].as_array().expect("phases");
            let mut current = Harness::new(&fixture, &spec, &phases[0]);
            let mut frozen = Vec::new();
            for (index, phase) in phases.iter().enumerate() {
                for action in states::actions(&spec, phase) {
                    fixture.apply(&action).expect("finite source action");
                    current.apply(&fixture, &action, index as i32 + 1);
                }
                if !phase["schemaAction"].is_null() {
                    current.schema(&fixture, &spec, &phase["schema"]);
                }
                check(&mut current, &fixture, &spec, phase);
                let fresh_fixture = states::fresh_fixture(&spec, phase);
                let mut fresh = Harness::new(&fresh_fixture, &spec, phase);
                check(&mut fresh, &fresh_fixture, &spec, phase);
                for (snapshot, old_phase, old_symbols) in &frozen {
                    check_snapshot(snapshot, &current, &spec, old_phase);
                    assert_eq!(
                        snapshot.databases().workspace_symbols(""),
                        *old_symbols,
                        "whole frozen ownership {}",
                        old_phase["id"]
                    );
                }
                let snapshot = current.test_server.snapshot();
                let symbols = snapshot.databases().workspace_symbols("");
                frozen.push((snapshot, phase.clone(), symbols));
            }
        }
    }
}
