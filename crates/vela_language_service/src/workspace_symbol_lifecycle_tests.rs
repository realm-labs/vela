use crate::LanguageServiceDatabases;
use crate::document_symbol_lifecycle_tests::{check_schema, schema, update, uri};
use crate::matrix_fixture::{
    FixtureWorkspace, Spec, document_symbol_lifecycle as states, workspace_symbols as oracle,
};
use serde_json::Value;

fn check(db: &LanguageServiceDatabases, spec: &Spec, phase: &Value) {
    let fixture = states::fresh_fixture(spec, phase);
    let mut effective = fixture.clone();
    for (file, doc) in &fixture.open {
        effective.disk.insert(file.clone(), doc.clone());
    }
    for (file, variant) in phase["views"].as_object().expect("views") {
        if variant.is_null() {
            assert!(
                !db.source_db().records().contains_key(&uri(file)),
                "deleted source {}",
                phase["id"]
            );
        } else {
            assert_eq!(
                db.source_db().records()[&uri(file)].text(),
                effective.disk[file].text
            );
            assert!(
                db.parse_db()
                    .parse_diagnostics(&uri(file))
                    .expect("parsed")
                    .is_empty()
            );
        }
    }
    assert_eq!(
        db.source_db().records().len(),
        phase["views"]
            .as_object()
            .expect("views")
            .values()
            .filter(|v| !v.is_null())
            .count()
    );
    check_schema(db, spec, &phase["schema"]);
    let before = db.workspace_symbols("");
    let workspace = &phase["workspace"];
    for query in workspace["queries"].as_array().expect("queries") {
        let wanted = oracle::expected(&effective, workspace, &query["symbols"], false, &|file| {
            uri(file).as_str().to_owned()
        });
        for repeat in 0..3 {
            assert_eq!(
                Value::Array(
                    db.workspace_symbols(query["query"].as_str().expect("query"))
                        .iter()
                        .map(crate::workspace_symbol_ownership_tests::project)
                        .collect()
                ),
                wanted,
                "phase={} query={} repeat={repeat}",
                phase["id"],
                query["id"]
            );
        }
    }
    assert_eq!(
        db.workspace_symbols(""),
        before,
        "query cannot mutate ownership"
    );
}

#[test]
fn workspace_symbol_lifecycle_pins_current_and_frozen_whole_sets_against_fresh_databases() {
    for crlf in [false, true] {
        for shifted in [false, true] {
            let spec = oracle::lifecycle(crlf, shifted);
            let mut fixture = FixtureWorkspace::new(&spec).expect("fixture");
            let mut db = LanguageServiceDatabases::new();
            let mut frozen = Vec::new();
            for (index, phase) in spec.oracle["phases"]
                .as_array()
                .expect("phases")
                .iter()
                .enumerate()
            {
                for action in states::actions(&spec, phase) {
                    fixture.apply(&action).expect("finite source action");
                }
                states::assert_state(&fixture, &spec, phase);
                update(&mut db, &fixture, index as u64 + 1);
                if index == 0 || !phase["schemaAction"].is_null() {
                    schema(&mut db, &fixture, &spec, &phase["schema"]);
                }
                check(&db, &spec, phase);
                let fresh_fixture = states::fresh_fixture(&spec, phase);
                let mut fresh = LanguageServiceDatabases::new();
                update(&mut fresh, &fresh_fixture, index as u64 + 1);
                schema(&mut fresh, &fresh_fixture, &spec, &phase["schema"]);
                check(&fresh, &spec, phase);
                for (snapshot, old_phase) in &frozen {
                    check(snapshot, &spec, old_phase);
                }
                frozen.push((db.clone(), phase.clone()));
            }
        }
    }
}
